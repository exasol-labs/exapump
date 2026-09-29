//! Turns a JSON or NDJSON file into an Exasol table family.
//!
//! `json_tables_core` owns which tables exist and which columns each table
//! carries. This module owns every SQL statement exapump runs, and what that
//! crate deliberately leaves out: reading the file, naming the tables the way
//! exapump names them for every other format, mapping a contract type to an
//! Exasol type, converting the buffered rows to Arrow, and running the
//! statements over a connection.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use arrow::array::{ArrayRef, BooleanArray, Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use json_tables_core::buffer::{ColumnBuffers, ColumnValues};
use json_tables_core::contract::{
    sanitize_ident, table_raw_name, table_sql_name, ColumnPlan, PlannedTable, SimpleType, TablePath,
};
use json_tables_core::infer::{build_all_schema_plans, StatsCollector};
use json_tables_core::read::{detect_format, for_each_document, InputFormat};
use json_tables_core::sink::write_document;

/// The fixed names `json_tables_core` generates on every table it plans.
///
/// These are the only generated names known ahead of any document.
/// `reject_unusable_plans` adds the `<name>|object` and `<name>|array` link
/// columns to them, whose names depend on the documents and so are read from
/// `plan.properties` instead.
const GENERATED_KEY_COLUMNS: [&str; 3] = ["_id", "_parent", "_pos"];

/// A planned table family derived from one JSON or NDJSON file.
///
/// The family is ordered: `plans` and every row count this module returns share
/// the order `build_all_schema_plans` produced, which is the table path in
/// lexical order. Two runs over the same input therefore create, load, and
/// report the same tables in the same sequence.
pub struct TableFamily {
    plans: Vec<PlannedTable>,
    stem: String,
    schema: Option<String>,
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
    reject_duplicate_table_names(&plans, &stem, path)?;

    Ok(TableFamily {
        plans,
        stem,
        schema,
    })
}

impl TableFamily {
    /// Table names, their columns, and the planned CREATE statements, for `--dry-run`.
    pub fn describe(&self) -> String {
        let mut out = String::new();
        for plan in &self.plans {
            let schema = self.schema.as_deref();
            out.push_str(&format!(
                "Table {}\n",
                qualified_name(plan, &self.stem, schema)
            ));
            out.push_str("Columns:\n");
            for (column, sql_type) in physical_columns(plan) {
                out.push_str(&format!(
                    "  {}: {}\n",
                    sanitize_ident(&column.name),
                    sql_type
                ));
            }
            out.push_str(&format!(
                "\n{}\n\n",
                create_table_ddl(plan, &self.stem, schema)
            ));
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

/// The table's name, qualified with `schema` when one is given.
///
/// A dry run has no connection to resolve a default schema from, so it passes
/// `None` and an unqualified `--table` previews unqualified names. `load`
/// resolves the schema first and passes it here, so every statement it runs
/// names the schema explicitly.
fn qualified_name(plan: &PlannedTable, stem: &str, schema: Option<&str>) -> String {
    let table = table_sql_name(&plan.path, stem);
    match schema {
        Some(schema) => format!("{}.{}", sanitize_ident(schema), table),
        None => table,
    }
}

/// The Exasol column type for a contract scalar type, or `None` for a type that
/// never becomes a physical column.
///
/// exapump owns this mapping rather than reusing `json_tables_core`'s
/// `column_sql_type`, which maps an integer to `DECIMAL(18,0)`. An integer
/// column holds an `i64`, and `DECIMAL(18,0)` stops at 18 digits, so the
/// upstream type cannot represent the whole 64-bit signed range. `DECIMAL(19,0)`
/// can, so every value the contract classifies as an integer survives the load.
fn exasol_type(ty: SimpleType) -> Option<&'static str> {
    match ty {
        SimpleType::Bool => Some("BOOLEAN"),
        SimpleType::Integer => Some("DECIMAL(19,0)"),
        SimpleType::Number => Some("DOUBLE"),
        SimpleType::String => Some("VARCHAR(2000000)"),
        SimpleType::Null | SimpleType::Object | SimpleType::Array => None,
    }
}

/// The `CREATE TABLE` statement for one planned table, naming `schema`.
///
/// exapump renders the statement from the column plan rather than rewriting one
/// `json_tables_core` emitted, so the declared column order is the
/// `physical_columns` order by construction and the qualified name is never
/// spliced onto a statement head exapump did not produce. `IF NOT EXISTS` makes
/// a repeated run load into the family it already created. A column is
/// `NOT NULL` when the plan marks it required or makes it a null mask, matching
/// the rule upstream's own `build_sql_schema` applies.
fn create_table_ddl(plan: &PlannedTable, stem: &str, schema: Option<&str>) -> String {
    let columns: Vec<String> = physical_columns(plan)
        .map(|(column, sql_type)| {
            let not_null = if column.is_required || column.is_null_mask {
                " NOT NULL"
            } else {
                ""
            };
            format!("  {} {sql_type}{not_null}", sanitize_ident(&column.name))
        })
        .collect();

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        qualified_name(plan, stem, schema),
        columns.join(",\n")
    )
}

/// The planned columns that become real table columns, paired with their Exasol
/// type, in the order the CREATE statement lists them.
fn physical_columns(plan: &PlannedTable) -> impl Iterator<Item = (&ColumnPlan, &'static str)> {
    plan.columns
        .iter()
        .filter_map(|column| exasol_type(column.ty).map(|sql_type| (column, sql_type)))
}

/// Rejects a family that would create tables holding no document data.
fn reject_unusable_plans(plans: &[PlannedTable], path: &Path) -> anyhow::Result<()> {
    if plans.is_empty() {
        anyhow::bail!("{} contains no documents", path.display());
    }

    let carries_document_data = plans.iter().any(|plan| {
        let generated = generated_column_names(plan);
        plan.columns
            .iter()
            .any(|column| !generated.contains(column.name.as_str()))
    });
    if !carries_document_data {
        anyhow::bail!(
            "no column could be derived from the documents in {}: every planned table carries \
             only generated columns, which are the key columns {} and the \"<name>|object\" and \
             \"<name>|array\" columns that link a parent row to a nested table",
            path.display(),
            GENERATED_KEY_COLUMNS.join(", ")
        );
    }

    Ok(())
}

/// The column names in `plan` that `json_tables_core` generated rather than
/// derived from a document property.
///
/// The link column names come from the plan's own property map rather than from
/// matching the `|object` and `|array` suffixes on the column name, so a
/// document property literally named `x|object` still counts as document data.
fn generated_column_names(plan: &PlannedTable) -> HashSet<&str> {
    let mut names: HashSet<&str> = GENERATED_KEY_COLUMNS.iter().copied().collect();
    for property in plan.properties.values() {
        names.extend(property.object_fk.as_deref());
        names.extend(property.array_count.as_deref());
    }
    names
}

/// Rejects a family whose planned table names are not unique.
///
/// Two JSON paths can sanitize to one table name, and the load would then write
/// two different document shapes into that one table. The check runs while the
/// family is still only a plan, so a `--dry-run` reports it too and no table
/// exists when the run fails.
fn reject_duplicate_table_names(
    plans: &[PlannedTable],
    stem: &str,
    path: &Path,
) -> anyhow::Result<()> {
    let mut claimed: HashMap<String, &TablePath> = HashMap::new();
    for plan in plans {
        let name = table_raw_name(&plan.path, stem);
        if let Some(first) = claimed.insert(name.clone(), &plan.path) {
            anyhow::bail!(
                "two JSON paths in {} produce the table name {}: {} and {}. Rename one of the \
                 properties, or import the two paths under different --table values.",
                path.display(),
                name,
                first,
                plan.path
            );
        }
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
///
/// The multi-line-object diagnosis is attached only when line framing produced
/// no document at all, because a single object spread over several lines fails
/// on its first line. A file that yielded at least one document is real NDJSON
/// with a malformed line, and its message keeps the line number that names the
/// defect instead of blaming the framing.
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
    let mut documents_seen = 0usize;
    let framing = for_each_document(reader, format, |index, document| {
        documents_seen += 1;
        visit(index, document).map_err(|error| {
            visit_failure = Some(error);
            json_tables_core::CoreError::msg("document rejected")
        })
    });

    if let Some(error) = visit_failure {
        return Err(error);
    }
    let framed_nothing = matches!(format, InputFormat::Lines) && documents_seen == 0;
    framing
        .map_err(|error| {
            let error = anyhow::Error::new(error);
            if framed_nothing {
                error.context(
                    "the file is neither a JSON array of objects nor one JSON object per line. A \
                     single JSON object spread over several lines matches neither framing",
                )
            } else {
                error
            }
        })
        .with_context(|| format!("failed to read {}", path.display()))
}

/// Creates the family's tables and loads every document from `path`.
///
/// `loaded` accumulates the row count loaded per table, in family order, and is
/// appended to as each table finishes. It therefore holds every table of the
/// family on success, and every table loaded before the error on a failure.
///
/// The import is not atomic across the family: a failure partway through leaves
/// the tables already loaded in place, which is why the counts accumulate into a
/// caller-owned vector rather than being returned only on success. Nothing here
/// writes to stdout; the caller owns the report.
pub async fn load(
    family: &TableFamily,
    path: &Path,
    conn: &mut exarrow_rs::Connection,
    loaded: &mut Vec<(String, u64)>,
) -> anyhow::Result<()> {
    let schema = family.target_schema(conn)?;

    create_tables(conn, family, &schema).await?;

    let buffers = collect_rows(family, path)?;

    for plan in &family.plans {
        let table = qualified_name(plan, &family.stem, Some(&schema));
        let batch = build_record_batch(plan, &buffers)
            .with_context(|| format!("failed to build the rows for {table}"))?;
        let rows = import_batch(conn, &table, batch).await?;
        loaded.push((table, rows));
    }

    Ok(())
}

/// Creates every table of the family in family order, in the resolved schema.
///
/// Every statement names `schema`, so which schema the family lands in never
/// depends on the session state an earlier statement left behind. A schema
/// Exasol does not hold therefore fails here, on the family's first table,
/// rather than at a separate preparatory statement.
async fn create_tables(
    conn: &mut exarrow_rs::Connection,
    family: &TableFamily,
    schema: &str,
) -> anyhow::Result<()> {
    for plan in &family.plans {
        let create = create_table_ddl(plan, &family.stem, Some(schema));
        conn.execute(&create).await.with_context(|| {
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
/// The import names its columns, so field order no longer decides which column
/// a value lands in. Field order still follows `physical_columns`, so the batch
/// and the CREATE statement describe the table the same way. The buffer is
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

/// Imports one batch into one explicitly qualified table, naming the columns it
/// carries.
///
/// The statement lists exactly the batch's own columns and nothing else, so a
/// value lands in the column of its own name whatever order the file listed the
/// properties in, a column the table holds but this batch never names loads
/// NULL, and a column the batch names but the table does not hold fails the
/// import by name instead of landing in a neighbouring column of the same type.
/// `exarrow_rs` writes the list into the statement verbatim, so each name is
/// quoted here exactly once.
async fn import_batch(
    conn: &mut exarrow_rs::Connection,
    table: &str,
    batch: RecordBatch,
) -> anyhow::Result<u64> {
    let schema = batch.schema();
    let columns: Vec<String> = schema
        .fields()
        .iter()
        .map(|field| sanitize_ident(field.name()))
        .collect();
    let options = exarrow_rs::ArrowImportOptions::new().columns(columns.clone());

    conn.import_from_record_batches(table, [batch], options)
        .await
        .with_context(|| {
            format!(
                "failed to import into {table}, naming the columns {}",
                columns.join(", ")
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn exasol_type_maps_every_contract_type() {
        assert_eq!(exasol_type(SimpleType::Bool), Some("BOOLEAN"));
        assert_eq!(
            exasol_type(SimpleType::Integer),
            Some("DECIMAL(19,0)"),
            "an integer column holds an i64, which DECIMAL(18,0) cannot represent in full"
        );
        assert_eq!(exasol_type(SimpleType::Number), Some("DOUBLE"));
        assert_eq!(exasol_type(SimpleType::String), Some("VARCHAR(2000000)"));
        assert_eq!(exasol_type(SimpleType::Null), None);
        assert_eq!(exasol_type(SimpleType::Object), None);
        assert_eq!(exasol_type(SimpleType::Array), None);
    }

    #[test]
    fn create_table_ddl_marks_required_and_null_mask_columns_not_null() {
        let documents: Vec<serde_json::Map<String, serde_json::Value>> = [
            serde_json::json!({"id": 1, "note": null}),
            serde_json::json!({"id": 2, "extra": "x"}),
        ]
        .into_iter()
        .map(|value| match value {
            serde_json::Value::Object(object) => object,
            other => panic!("the fixture must be a JSON object, got {other}"),
        })
        .collect();

        let plans = StatsCollector::plan_from_documents(&documents);

        assert_eq!(
            create_table_ddl(&plans[0], "ORDERS", Some("SALES")),
            "CREATE TABLE IF NOT EXISTS \"SALES\".\"ORDERS\" (\n  \"_id\" DECIMAL(19,0) NOT NULL,\n  \
             \"id\" DECIMAL(19,0),\n  \"note|n\" BOOLEAN NOT NULL,\n  \"extra\" \
             VARCHAR(2000000)\n);",
            "a required column and a null-mask column each carry NOT NULL, while an ordinary \
             document column does not"
        );
    }
}
