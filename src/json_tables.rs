//! Turns a JSON or NDJSON file into an Exasol table family.
//!
//! `json_tables_core` owns every decision about which tables exist, which
//! columns they carry, and which DDL describes them. This module owns what that
//! crate deliberately leaves out: reading the file, naming the tables the way
//! exapump names them for every other format, converting the buffered rows to
//! Arrow, and running the statements over a connection.

use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use arrow::array::{ArrayRef, BooleanArray, Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use json_tables_core::buffer::{ColumnBuffers, ColumnValues};
use json_tables_core::contract::{
    column_sql_type, sanitize_ident, table_sql_name, ColumnPlan, PlannedTable,
};
use json_tables_core::ddl::build_sql_schema;
use json_tables_core::infer::{build_all_schema_plans, StatsCollector};
use json_tables_core::read::{detect_format, for_each_document};
use json_tables_core::sink::write_document;

/// Columns `json_tables_core` generates itself. A table holding nothing else
/// carries no data that came out of the documents.
const GENERATED_KEY_COLUMNS: [&str; 3] = ["_id", "_parent", "_pos"];

/// A planned table family derived from one JSON or NDJSON file.
///
/// The family is ordered: `plans`, `ddl`, and every row count this module
/// returns share the order `build_all_schema_plans` produced, which is the
/// table path in lexical order. Two runs over the same input therefore create,
/// load, and report the same tables in the same sequence.
pub struct TableFamily {
    plans: Vec<PlannedTable>,
    stem: String,
    schema: Option<String>,
    ddl: Vec<String>,
}

/// Scans `path` and derives the table family for the target `table`.
///
/// Performs no database I/O. `table` is split on its first `.`; both parts are
/// uppercased, so one `--table` value names the same root table for JSON, CSV,
/// and Parquet input. The uppercased table part is the stem every subtable name
/// is built from.
pub fn plan_family(path: &Path, table: &str) -> anyhow::Result<TableFamily> {
    let (schema, stem) = match table.split_once('.') {
        Some((schema, name)) => (Some(schema.to_uppercase()), name.to_uppercase()),
        None => (None, table.to_uppercase()),
    };

    let mut stats = StatsCollector::new();
    read_documents(path, |_, document| {
        stats.record_document(document);
        Ok(())
    })?;
    let plans = build_all_schema_plans(&stats.finish());

    reject_unusable_plans(&plans, path)?;
    let ddl = build_ddl(&plans, &stem, schema.as_deref())?;

    Ok(TableFamily {
        plans,
        stem,
        schema,
        ddl,
    })
}

impl TableFamily {
    /// Table names, their columns, and the planned CREATE statements, for `--dry-run`.
    pub fn describe(&self) -> String {
        let mut out = String::new();
        for (plan, create) in self.plans.iter().zip(&self.ddl) {
            out.push_str(&format!(
                "Table {}\n",
                qualified_name(plan, &self.stem, self.schema.as_deref())
            ));
            out.push_str("Columns:\n");
            for (column, sql_type) in physical_columns(plan) {
                out.push_str(&format!(
                    "  {}: {}\n",
                    sanitize_ident(&column.name),
                    sql_type
                ));
            }
            out.push_str(&format!("\n{create}\n\n"));
        }
        out
    }

    /// The one schema every statement of this family names.
    ///
    /// A schema part on `--table` wins, so the target never depends on session
    /// state; otherwise the connection's default schema supplies it. Resolved
    /// before any statement runs, so the family is never half-created in a
    /// schema exapump did not choose.
    fn target_schema(&self, conn: &exarrow_rs::Connection) -> anyhow::Result<String> {
        if let Some(schema) = &self.schema {
            return Ok(schema.clone());
        }
        match &conn.params().schema {
            Some(schema) => Ok(schema.to_uppercase()),
            None => anyhow::bail!(
                "no target schema could be resolved: --table carries no schema part and the \
                 connection has no default schema. Pass --table <schema>.<table>, or name a \
                 default schema in the DSN."
            ),
        }
    }
}

/// The table's name, qualified with the schema `--table` carried.
///
/// `build_sql_schema` emits unqualified names, and a dry run has no connection
/// to resolve a default schema from, so an unqualified `--table` stays
/// unqualified here. `load` qualifies its own statements instead.
fn qualified_name(plan: &PlannedTable, stem: &str, schema: Option<&str>) -> String {
    let table = table_sql_name(&plan.path, stem);
    match schema {
        Some(schema) => format!("{}.{}", sanitize_ident(schema), table),
        None => table,
    }
}

/// Restates the `build_sql_schema` CREATE statements as exapump's upload
/// contract requires them: idempotent, and naming the schema the family
/// resolved. The constraint statements are discarded; they are emitted
/// `DISABLE` upstream and enforce nothing.
fn build_ddl(
    plans: &[PlannedTable],
    stem: &str,
    schema: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    let (creates, _constraints) = build_sql_schema(plans, stem);

    plans
        .iter()
        .zip(creates)
        .map(|(plan, create)| {
            rewrite_create(
                &create,
                &table_sql_name(&plan.path, stem),
                &qualified_name(plan, stem, schema),
            )
        })
        .collect()
}

/// Restates one upstream CREATE statement as `CREATE TABLE IF NOT EXISTS
/// {qualified}`, keeping its column list verbatim.
///
/// The rewrite recognises the statement by the head `build_sql_schema` is
/// contracted to emit. A head it does not recognise is an upstream change
/// exapump cannot reason about, so it fails here rather than splicing a body
/// onto a name it did not verify.
fn rewrite_create(create: &str, emitted: &str, qualified: &str) -> anyhow::Result<String> {
    let head = format!("CREATE TABLE {emitted} (");
    let body = create.strip_prefix(&head).with_context(|| {
        format!("json_tables_core emitted an unrecognised CREATE statement for {emitted}")
    })?;
    Ok(format!("CREATE TABLE IF NOT EXISTS {qualified} ({body}"))
}

/// The planned columns that become real table columns, paired with their Exasol
/// type, in the order the CREATE statement lists them.
fn physical_columns(plan: &PlannedTable) -> impl Iterator<Item = (&ColumnPlan, &'static str)> {
    plan.columns
        .iter()
        .filter_map(|column| column_sql_type(column.ty).map(|sql_type| (column, sql_type)))
}

/// Rejects a family that would create tables holding no document data.
fn reject_unusable_plans(plans: &[PlannedTable], path: &Path) -> anyhow::Result<()> {
    if plans.is_empty() {
        anyhow::bail!("{} contains no documents", path.display());
    }

    let only_keys = plans.iter().all(|plan| {
        plan.columns
            .iter()
            .all(|column| GENERATED_KEY_COLUMNS.contains(&column.name.as_str()))
    });
    if only_keys {
        anyhow::bail!(
            "no column could be derived from the documents in {}: every planned table carries only the generated key columns {}",
            path.display(),
            GENERATED_KEY_COLUMNS.join(", ")
        );
    }

    Ok(())
}

/// Reads `path` once, framing it by its first non-whitespace byte, and hands
/// every document to `visit`.
///
/// Both passes over the input go through here, so framing detection stays one
/// decision rather than two. A failure raised by `visit` comes back unchanged:
/// only opening, framing, and parsing the file are reported as read failures,
/// so the message never names an operation that did not fail.
fn read_documents<F>(path: &Path, mut visit: F) -> anyhow::Result<()>
where
    F: FnMut(usize, &serde_json::Map<String, serde_json::Value>) -> anyhow::Result<()>,
{
    let file =
        std::fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    let format =
        detect_format(&mut reader).with_context(|| format!("failed to read {}", path.display()))?;

    let mut visit_failure: Option<anyhow::Error> = None;
    let framing = for_each_document(reader, format, |index, document| {
        visit(index, document).map_err(|error| {
            visit_failure = Some(error);
            json_tables_core::CoreError::msg("document rejected")
        })
    });

    if let Some(error) = visit_failure {
        return Err(error);
    }
    framing.with_context(|| format!("failed to read {}", path.display()))
}

/// Creates the family's tables and loads every document from `path`.
///
/// The returned vector holds the row count loaded per table, in family order.
/// On a failure it holds every table loaded before the error. On success it
/// holds every table of the family. The returned option holds the failure.
///
/// The import is not atomic across the family: a failure partway through leaves
/// the tables already loaded in place, which is why the counts come back beside
/// the error rather than instead of it. Nothing here writes to stdout; the
/// caller owns the report.
pub async fn load(
    family: &TableFamily,
    path: &Path,
    conn: &mut exarrow_rs::Connection,
) -> (Vec<(String, u64)>, Option<anyhow::Error>) {
    let mut loaded = Vec::new();

    let schema = match family.target_schema(conn) {
        Ok(schema) => schema,
        Err(error) => return (loaded, Some(error)),
    };

    if let Err(error) = create_tables(conn, family, &schema).await {
        return (loaded, Some(error));
    }

    let buffers = match collect_rows(family, path) {
        Ok(buffers) => buffers,
        Err(error) => return (loaded, Some(error)),
    };

    for plan in &family.plans {
        let table = qualified_name(plan, &family.stem, Some(&schema));
        let batch = match build_record_batch(plan, &buffers)
            .with_context(|| format!("failed to build the rows for {table}"))
        {
            Ok(batch) => batch,
            Err(error) => return (loaded, Some(error)),
        };
        match import_batch(conn, &table, batch).await {
            Ok(rows) => loaded.push((table, rows)),
            Err(error) => return (loaded, Some(error)),
        }
    }

    (loaded, None)
}

/// Opens the resolved schema, then creates every table of the family in family
/// order.
///
/// `build_sql_schema` emits unqualified names for an unqualified `--table`, so
/// the open schema is what places those tables in the resolved schema.
async fn create_tables(
    conn: &mut exarrow_rs::Connection,
    family: &TableFamily,
    schema: &str,
) -> anyhow::Result<()> {
    let quoted_schema = sanitize_ident(schema);
    conn.execute(&format!("OPEN SCHEMA {quoted_schema}"))
        .await
        .with_context(|| format!("failed to open schema {quoted_schema}"))?;

    for (plan, create) in family.plans.iter().zip(&family.ddl) {
        conn.execute(create).await.with_context(|| {
            format!(
                "failed to create {}",
                qualified_name(plan, &family.stem, Some(schema))
            )
        })?;
    }

    Ok(())
}

/// Buffers every document of `path` against the family's plans.
fn collect_rows(family: &TableFamily, path: &Path) -> anyhow::Result<ColumnBuffers> {
    let mut buffers = ColumnBuffers::new(&family.plans);
    read_documents(path, |_, document| {
        write_document(&mut buffers, document)
            .with_context(|| format!("failed to buffer the documents of {}", path.display()))
    })?;
    Ok(buffers)
}

/// The buffered rows of one planned table, as the batch the import takes.
///
/// Field order is the CREATE statement's column order, because the import maps
/// CSV fields onto table columns by position rather than by name. The buffer is
/// resolved by table path: `ColumnBuffers` holds its tables in a `HashMap`, so
/// iterating it would yield a per-process random order.
fn build_record_batch(plan: &PlannedTable, buffers: &ColumnBuffers) -> anyhow::Result<RecordBatch> {
    let buffer = buffers
        .table(&plan.path)
        .with_context(|| format!("no buffered rows for table path {}", plan.path))?;

    let mut fields = Vec::new();
    let mut arrays: Vec<ArrayRef> = Vec::new();

    for (column, _) in physical_columns(plan) {
        let values = buffer.column(column)?;
        let (data_type, array): (DataType, ArrayRef) = match values {
            ColumnValues::Bool(v) => (
                DataType::Boolean,
                Arc::new(BooleanArray::from_iter(v.iter().copied())),
            ),
            ColumnValues::BoolMask(v) => {
                (DataType::Boolean, Arc::new(BooleanArray::from(v.clone())))
            }
            ColumnValues::Int(v) => (
                DataType::Int64,
                Arc::new(Int64Array::from_iter(v.iter().copied())),
            ),
            ColumnValues::Double(v) => (
                DataType::Float64,
                Arc::new(Float64Array::from_iter(v.iter().copied())),
            ),
            ColumnValues::Str(v) => (
                DataType::Utf8,
                Arc::new(StringArray::from_iter(v.iter().map(|s| s.as_deref()))),
            ),
        };
        let holds_nulls = !matches!(values, ColumnValues::BoolMask(_));
        fields.push(Field::new(&column.name, data_type, holds_nulls));
        arrays.push(array);
    }

    RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays)
        .with_context(|| format!("failed to assemble the rows of table path {}", plan.path))
}

/// Imports one batch into one explicitly qualified table.
async fn import_batch(
    conn: &mut exarrow_rs::Connection,
    table: &str,
    batch: RecordBatch,
) -> anyhow::Result<u64> {
    conn.import_from_record_batches(table, [batch], exarrow_rs::ArrowImportOptions::new())
        .await
        .with_context(|| format!("failed to import into {table}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use json_tables_core::contract::ColumnKind;

    #[test]
    fn read_documents_returns_the_visitors_error_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("orders.json");
        std::fs::write(&path, r#"[{"id": 1}]"#).unwrap();

        let error = read_documents(&path, |_, _| {
            anyhow::bail!("failed to buffer the documents of orders.json")
        })
        .unwrap_err();

        assert_eq!(
            format!("{error}"),
            "failed to buffer the documents of orders.json",
            "a visitor failure must not be restated as a read failure"
        );
    }

    #[test]
    fn build_ddl_rejects_an_unrecognised_create_statement() {
        let error = rewrite_create(
            "CREATE TABLE \"ORDERS\" AS SELECT 1;",
            "\"ORDERS\"",
            "\"SALES\".\"ORDERS\"",
        )
        .unwrap_err();

        assert!(
            format!("{error}").contains("unrecognised CREATE statement"),
            "a statement head exapump cannot rewrite must be reported, not spliced: {error}"
        );
    }

    /// A family covering every table kind and every column kind the contract
    /// has: a root object table, a nested object subtable, a nested array
    /// subtable, a `Primary` column per property, an `Alternate` for `code`
    /// (string twice, integer once), and a `NullBitmask` for `note` (an
    /// explicit null once).
    fn mixed_family() -> Vec<serde_json::Map<String, serde_json::Value>> {
        [
            serde_json::json!({
                "id": 1,
                "code": "A1",
                "note": null,
                "customer": {"tier": "gold"},
                "items": [{"sku": "S1"}]
            }),
            serde_json::json!({
                "id": 2,
                "code": "B2",
                "note": "kept",
                "customer": {"tier": "silver"},
                "items": [{"sku": "S2"}, {"sku": "S3"}]
            }),
            serde_json::json!({
                "id": 3,
                "code": 42,
                "customer": {"tier": "bronze"},
                "items": []
            }),
        ]
        .into_iter()
        .map(|value| match value {
            serde_json::Value::Object(document) => document,
            other => panic!("fixture document is not an object: {other}"),
        })
        .collect()
    }

    /// The quoted column identifiers a CREATE statement declares, in order.
    fn declared_columns(create: &str) -> Vec<String> {
        let (_, body) = create
            .split_once('(')
            .expect("a CREATE statement opens a column list");
        body.lines()
            .map(str::trim)
            .take_while(|line| *line != ");")
            .filter(|line| !line.is_empty())
            .map(leading_quoted_ident)
            .collect()
    }

    /// The leading double-quoted identifier of a column declaration, its
    /// doubled-quote escapes included.
    fn leading_quoted_ident(declaration: &str) -> String {
        let mut chars = declaration.chars().peekable();
        assert_eq!(
            chars.next(),
            Some('"'),
            "column declaration does not open with a quoted identifier: {declaration}"
        );
        let mut ident = String::from('"');
        while let Some(character) = chars.next() {
            ident.push(character);
            if character == '"' {
                if chars.peek() == Some(&'"') {
                    ident.push(chars.next().expect("peeked character"));
                } else {
                    return ident;
                }
            }
        }
        panic!("unterminated quoted identifier: {declaration}");
    }

    #[test]
    fn ddl_column_order_matches_record_batch_field_order() {
        let documents = mixed_family();
        let plans = StatsCollector::plan_from_documents(&documents);
        let mut buffers = ColumnBuffers::new(&plans);
        for document in &documents {
            write_document(&mut buffers, document).unwrap();
        }
        let ddl = build_ddl(&plans, "ORDERS", Some("SALES")).unwrap();

        let kinds: Vec<&ColumnKind> = plans
            .iter()
            .flat_map(|plan| plan.columns.iter().map(|column| &column.kind))
            .collect();
        assert_eq!(plans.len(), 3, "expected a root table and two subtables");
        assert!(
            kinds
                .iter()
                .any(|kind| matches!(kind, ColumnKind::Alternate { .. })),
            "the fixture must produce an alternate column, or a reorder across one goes unseen"
        );
        assert!(
            kinds
                .iter()
                .any(|kind| matches!(kind, ColumnKind::NullBitmask { .. })),
            "the fixture must produce a null-mask column, or a reorder across one goes unseen"
        );

        for (plan, create) in plans.iter().zip(&ddl) {
            let batch = build_record_batch(plan, &buffers).unwrap();
            let fields: Vec<String> = batch
                .schema()
                .fields()
                .iter()
                .map(|field| sanitize_ident(field.name()))
                .collect();
            let declared = declared_columns(create);

            assert!(
                !declared.is_empty(),
                "no column parsed out of the CREATE statement for {}: {create}",
                plan.path
            );
            assert_eq!(
                declared, fields,
                "table {} declares {declared:?} but its batch carries {fields:?}; the import \
                 maps CSV fields onto columns by position, so any difference loads wrong data",
                plan.path
            );
        }
    }
}
