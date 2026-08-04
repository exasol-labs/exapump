use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::datatypes::SchemaRef;
use arrow::record_batch::RecordBatch;
use exarrow_rs::{
    ArrowExportOptions, CsvExportOptions, ExportSource, ParquetCompression, ParquetExportOptions,
};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression as ParquetCodec;
use parquet::file::properties::WriterProperties;

use crate::cli::{Compression, ExportArgs, ExportFormat};

/// Build a SQL query that returns zero rows but carries the column schema
/// for a given export source.
fn schema_query(source: &ExportSource) -> String {
    match source {
        ExportSource::Table {
            schema,
            name,
            columns,
        } => {
            let cols = if columns.is_empty() {
                "*".to_string()
            } else {
                columns.join(", ")
            };
            let table_ref = if let Some(s) = schema {
                format!("{s}.{name}")
            } else {
                name.to_string()
            };
            format!("SELECT {cols} FROM {table_ref} WHERE FALSE")
        }
        ExportSource::Query { sql } => {
            format!("SELECT * FROM ({sql}) sub WHERE FALSE")
        }
    }
}

/// Maps the CLI `Compression` enum to the exarrow-rs `ParquetCompression` enum.
///
/// If `None` is provided, defaults to `Snappy`.
pub(crate) fn map_compression(comp: Option<&Compression>) -> ParquetCompression {
    match comp {
        None | Some(Compression::Snappy) => ParquetCompression::Snappy,
        Some(Compression::Gzip) => ParquetCompression::Gzip,
        Some(Compression::Lz4) => ParquetCompression::Lz4,
        Some(Compression::Zstd) => ParquetCompression::Zstd,
        Some(Compression::None) => ParquetCompression::None,
    }
}

/// Maps the CLI `Compression` enum to the `parquet` crate's `Compression` codec,
/// used when writing via `ArrowWriter` in the split-file path.
fn map_compression_to_codec(comp: Option<&Compression>) -> ParquetCodec {
    match comp {
        None | Some(Compression::Snappy) => ParquetCodec::SNAPPY,
        Some(Compression::Gzip) => ParquetCodec::GZIP(Default::default()),
        Some(Compression::Lz4) => ParquetCodec::LZ4,
        Some(Compression::Zstd) => ParquetCodec::ZSTD(Default::default()),
        Some(Compression::None) => ParquetCodec::UNCOMPRESSED,
    }
}

fn resolve_export_source(args: &ExportArgs) -> anyhow::Result<ExportSource> {
    if let Some(ref table) = args.table {
        let (schema, name) = super::parse_table_name(table);
        Ok(ExportSource::Table {
            schema: schema.map(String::from),
            name: name.to_string(),
            columns: vec![],
        })
    } else if let Some(ref sql) = args.query {
        Ok(ExportSource::Query { sql: sql.clone() })
    } else {
        anyhow::bail!("either --table or --query must be provided")
    }
}

/// Rejects `--compression` combined with `--format csv`, before any other
/// validation runs. `run` calls this first so the compression/format mismatch
/// is always reported ahead of errors about the export source, keeping the
/// error a user sees independent of which validation happens to run first.
fn reject_compression_for_csv(args: &ExportArgs) -> anyhow::Result<()> {
    if matches!(args.format, ExportFormat::Csv) && args.compression.is_some() {
        anyhow::bail!("--compression is only supported for Parquet format");
    }
    Ok(())
}

fn build_csv_options(args: &ExportArgs) -> anyhow::Result<CsvExportOptions> {
    let mut options = CsvExportOptions::new()
        .column_separator(args.delimiter)
        .column_delimiter(args.quote)
        .with_column_names(!args.no_header);

    if !args.null_value.is_empty() {
        options = options.null_value(&args.null_value);
    }

    Ok(options)
}

/// Row/byte thresholds for splitting export output across multiple files.
/// No threshold set means the export writes a single file.
#[derive(Clone, Copy)]
struct SplitLimits {
    max_rows: Option<u64>,
    max_bytes: Option<u64>,
}

impl SplitLimits {
    fn is_split(&self) -> bool {
        self.max_rows.is_some() || self.max_bytes.is_some()
    }
}

fn resolve_split_limits(args: &ExportArgs) -> anyhow::Result<SplitLimits> {
    let max_bytes = args
        .max_file_size
        .as_deref()
        .map(crate::size::parse_size)
        .transpose()?;

    Ok(SplitLimits {
        max_rows: args.max_rows_per_file,
        max_bytes,
    })
}

async fn export_csv(
    conn: &mut exarrow_rs::Connection,
    source: ExportSource,
    options: CsvExportOptions,
    base_path: &Path,
    limits: SplitLimits,
) -> anyhow::Result<()> {
    let with_header = options.with_column_names;

    if limits.is_split() {
        let mut split_writer = crate::split::SplitCsvWriter::new(
            base_path.to_path_buf(),
            limits.max_rows,
            limits.max_bytes,
            with_header,
        );

        conn.export_csv_to_stream(source, &mut split_writer, options)
            .await?;

        let (total_rows, num_files) = split_writer.finish()?;

        if num_files == 1 {
            crate::split::rename_single_split(base_path)?;
        }

        eprintln!("Exported {total_rows} rows to {num_files} file(s)");
    } else {
        let rows = conn.export_csv_to_file(source, base_path, options).await?;
        eprintln!("Exported {rows} rows");
    }

    Ok(())
}

/// Where and how a Parquet export writes its output: the destination path,
/// the row/byte thresholds that decide whether it splits across files, and
/// the compression codec. Grouping these three keeps `export_parquet`,
/// `export_parquet_split` and `write_parquet_batches` at three parameters or
/// fewer despite each needing the full trio.
#[derive(Clone, Copy)]
struct ParquetTarget<'a> {
    base_path: &'a Path,
    limits: SplitLimits,
    compression: Option<&'a Compression>,
}

fn write_parquet_batches(
    batches: &[RecordBatch],
    schema: SchemaRef,
    target: ParquetTarget<'_>,
) -> anyhow::Result<(u64, u32)> {
    let codec = map_compression_to_codec(target.compression);
    let props = WriterProperties::builder().set_compression(codec).build();

    let mut file_index: u32 = 0;
    let mut total_rows: u64 = 0;
    let mut current_file_rows: u64 = 0;

    let current_path = crate::split::split_path(target.base_path, file_index);
    let file = File::create(&current_path)?;
    let mut writer = ArrowWriter::try_new(file, Arc::clone(&schema), Some(props.clone()))?;

    for batch in batches {
        let batch_rows = batch.num_rows() as u64;

        let row_limit_hit = target
            .limits
            .max_rows
            .is_some_and(|max| current_file_rows > 0 && current_file_rows + batch_rows > max);
        let size_limit_hit = target
            .limits
            .max_bytes
            .is_some_and(|max| current_file_rows > 0 && writer.bytes_written() as u64 >= max);

        if row_limit_hit || size_limit_hit {
            writer.close()?;
            file_index += 1;

            let next_path = crate::split::split_path(target.base_path, file_index);
            let file = File::create(&next_path)?;
            writer = ArrowWriter::try_new(file, Arc::clone(&schema), Some(props.clone()))?;
            current_file_rows = 0;
        }

        writer.write(batch)?;
        current_file_rows += batch_rows;
        total_rows += batch_rows;
    }

    writer.close()?;

    Ok((total_rows, file_index))
}

/// Exports `source` to Parquet as multiple row/byte-bounded files.
///
/// The Arrow schema is obtained by running a zero-row query against `source`
/// rather than from the first batch, because a batch may never arrive (an
/// empty result still writes one output file, below) and the writer needs
/// the schema before it sees any data. `batch_size` is aligned to
/// `target.limits.max_rows` so each fetched `RecordBatch` lines up with the
/// desired per-file row limit, keeping file rotation exact instead of
/// splitting a batch across files.
async fn export_parquet_split(
    conn: &mut exarrow_rs::Connection,
    source: ExportSource,
    target: ParquetTarget<'_>,
) -> anyhow::Result<()> {
    let schema_sql = schema_query(&source);
    let rs = conn.execute(schema_sql).await?;
    let arrow_schema = rs
        .metadata()
        .map(|m| Arc::clone(&m.schema))
        .ok_or_else(|| anyhow::anyhow!("could not determine schema for split export"))?;

    let mut arrow_opts = ArrowExportOptions::new().with_schema(arrow_schema);
    if let Some(mr) = target.limits.max_rows {
        arrow_opts = arrow_opts.with_batch_size(mr as usize);
    }

    let batches = conn.export_to_record_batches(source, arrow_opts).await?;

    if batches.is_empty() {
        eprintln!("Exported 0 rows to 1 file(s)");
        let options =
            ParquetExportOptions::new().with_compression(map_compression(target.compression));
        conn.export_to_parquet(
            ExportSource::Query {
                sql: "SELECT 1 WHERE FALSE".to_string(),
            },
            target.base_path,
            options,
        )
        .await?;
        return Ok(());
    }

    let schema = batches[0].schema();
    let (total_rows, file_index) = write_parquet_batches(&batches, Arc::clone(&schema), target)?;

    let num_files = file_index + 1;
    if file_index == 0 {
        crate::split::rename_single_split(target.base_path)?;
    }

    eprintln!("Exported {total_rows} rows to {num_files} file(s)");
    Ok(())
}

async fn export_parquet(
    conn: &mut exarrow_rs::Connection,
    source: ExportSource,
    target: ParquetTarget<'_>,
) -> anyhow::Result<()> {
    if target.limits.is_split() {
        export_parquet_split(conn, source, target).await
    } else {
        let options =
            ParquetExportOptions::new().with_compression(map_compression(target.compression));
        let rows = conn
            .export_to_parquet(source, target.base_path, options)
            .await?;
        eprintln!("Exported {rows} rows");
        Ok(())
    }
}

/// Executes the export command: exports a table or query result to a file.
pub async fn run(args: ExportArgs) -> anyhow::Result<()> {
    reject_compression_for_csv(&args)?;

    let source = resolve_export_source(&args)?;
    let base_path = Path::new(&args.output).to_path_buf();
    let limits = resolve_split_limits(&args)?;

    match args.format {
        ExportFormat::Csv => {
            let options = build_csv_options(&args)?;
            let mut conn = args.conn.connect().await?;
            export_csv(&mut conn, source, options, &base_path, limits).await?;
        }
        ExportFormat::Parquet => {
            let mut conn = args.conn.connect().await?;
            let target = ParquetTarget {
                base_path: &base_path,
                limits,
                compression: args.compression.as_ref(),
            };
            export_parquet(&mut conn, source, target).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{ConnectionArgs, Transport};

    fn base_args() -> ExportArgs {
        ExportArgs {
            table: None,
            query: None,
            output: "out.csv".to_string(),
            format: ExportFormat::Csv,
            conn: ConnectionArgs {
                dsn: None,
                profile: None,
                certificate_fingerprint: None,
                transport: Transport::Native,
            },
            delimiter: ',',
            quote: '"',
            no_header: false,
            null_value: String::new(),
            compression: None,
            max_rows_per_file: None,
            max_file_size: None,
        }
    }

    #[test]
    fn resolve_export_source_from_table() {
        let mut args = base_args();
        args.table = Some("myschema.mytable".to_string());

        let source = resolve_export_source(&args).unwrap();

        match source {
            ExportSource::Table {
                schema,
                name,
                columns,
            } => {
                assert_eq!(schema, Some("myschema".to_string()));
                assert_eq!(name, "mytable");
                assert!(columns.is_empty());
            }
            ExportSource::Query { .. } => panic!("expected Table source"),
        }
    }

    #[test]
    fn resolve_export_source_from_table_without_schema() {
        let mut args = base_args();
        args.table = Some("mytable".to_string());

        let source = resolve_export_source(&args).unwrap();

        match source {
            ExportSource::Table { schema, name, .. } => {
                assert_eq!(schema, None);
                assert_eq!(name, "mytable");
            }
            ExportSource::Query { .. } => panic!("expected Table source"),
        }
    }

    #[test]
    fn resolve_export_source_from_query() {
        let mut args = base_args();
        args.query = Some("SELECT 1".to_string());

        let source = resolve_export_source(&args).unwrap();

        match source {
            ExportSource::Query { sql } => assert_eq!(sql, "SELECT 1"),
            ExportSource::Table { .. } => panic!("expected Query source"),
        }
    }

    #[test]
    fn resolve_export_source_errors_when_neither_provided() {
        let args = base_args();

        let err = resolve_export_source(&args).unwrap_err();

        assert!(err.to_string().contains("either --table or --query"));
    }

    #[test]
    fn reject_compression_for_csv_rejects_compression_with_csv_format() {
        let mut args = base_args();
        args.compression = Some(Compression::Snappy);

        let err = reject_compression_for_csv(&args).unwrap_err();

        assert!(err
            .to_string()
            .contains("--compression is only supported for Parquet format"));
    }

    #[test]
    fn reject_compression_for_csv_allows_compression_with_parquet_format() {
        let mut args = base_args();
        args.format = ExportFormat::Parquet;
        args.compression = Some(Compression::Snappy);

        assert!(reject_compression_for_csv(&args).is_ok());
    }

    #[test]
    fn compression_with_csv_is_rejected_before_the_missing_source_error() {
        let mut args = base_args();
        args.compression = Some(Compression::Snappy);
        args.format = ExportFormat::Csv;
        args.table = None;
        args.query = None;

        let err = reject_compression_for_csv(&args).unwrap_err();

        assert!(err
            .to_string()
            .contains("--compression is only supported for Parquet format"));
    }

    #[test]
    fn build_csv_options_succeeds_without_compression() {
        let args = base_args();

        let options = build_csv_options(&args).unwrap();

        assert_eq!(options.null_value, None);
        assert_eq!(options.column_separator, ',');
        assert_eq!(options.column_delimiter, '"');
        assert!(options.with_column_names);
    }

    #[test]
    fn build_csv_options_applies_null_value_when_non_empty() {
        let mut args = base_args();
        args.null_value = "N/A".to_string();

        let options = build_csv_options(&args).unwrap();

        assert_eq!(options.null_value, Some("N/A".to_string()));
    }

    #[test]
    fn resolve_split_limits_defaults_to_no_splitting() {
        let args = base_args();

        let limits = resolve_split_limits(&args).unwrap();

        assert!(!limits.is_split());
        assert_eq!(limits.max_rows, None);
        assert_eq!(limits.max_bytes, None);
    }

    #[test]
    fn resolve_split_limits_splits_on_max_rows_per_file() {
        let mut args = base_args();
        args.max_rows_per_file = Some(100);

        let limits = resolve_split_limits(&args).unwrap();

        assert!(limits.is_split());
        assert_eq!(limits.max_rows, Some(100));
        assert_eq!(limits.max_bytes, None);
    }

    #[test]
    fn resolve_split_limits_splits_on_max_file_size() {
        let mut args = base_args();
        args.max_file_size = Some("1MB".to_string());

        let limits = resolve_split_limits(&args).unwrap();

        assert!(limits.is_split());
        assert_eq!(limits.max_rows, None);
        assert_eq!(limits.max_bytes, Some(1_000_000));
    }

    #[test]
    fn resolve_split_limits_splits_on_both_thresholds() {
        let mut args = base_args();
        args.max_rows_per_file = Some(50);
        args.max_file_size = Some("500KB".to_string());

        let limits = resolve_split_limits(&args).unwrap();

        assert!(limits.is_split());
        assert_eq!(limits.max_rows, Some(50));
        assert_eq!(limits.max_bytes, Some(500_000));
    }

    #[test]
    fn resolve_split_limits_propagates_invalid_size_error() {
        let mut args = base_args();
        args.max_file_size = Some("not-a-size".to_string());

        assert!(resolve_split_limits(&args).is_err());
    }
}
