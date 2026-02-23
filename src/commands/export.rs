use std::fs::File;
use std::path::Path;
use std::sync::Arc;

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

/// Executes the export command: exports a table or query result to a file.
pub async fn run(args: ExportArgs) -> anyhow::Result<()> {
    if args.compression.is_some() && matches!(args.format, ExportFormat::Csv) {
        anyhow::bail!("--compression is only supported for Parquet format");
    }

    let source = if let Some(ref table) = args.table {
        let (schema, name) = super::parse_table_name(table);
        ExportSource::Table {
            schema: schema.map(String::from),
            name: name.to_string(),
            columns: vec![],
        }
    } else if let Some(ref sql) = args.query {
        ExportSource::Query { sql: sql.clone() }
    } else {
        anyhow::bail!("either --table or --query must be provided");
    };

    match args.format {
        ExportFormat::Csv => {
            let mut options = CsvExportOptions::new()
                .column_separator(args.delimiter)
                .column_delimiter(args.quote)
                .with_column_names(!args.no_header);

            if !args.null_value.is_empty() {
                options = options.null_value(&args.null_value);
            }

            let base_path = Path::new(&args.output);
            let splitting = args.max_rows_per_file.is_some() || args.max_file_size.is_some();

            if splitting {
                let max_rows = args.max_rows_per_file;
                let max_bytes = args
                    .max_file_size
                    .as_deref()
                    .map(crate::size::parse_size)
                    .transpose()?;

                let mut split_writer = crate::split::SplitCsvWriter::new(
                    base_path.to_path_buf(),
                    max_rows,
                    max_bytes,
                    !args.no_header,
                );

                let mut conn = args.conn.connect().await?;
                conn.export_csv_to_stream(source, &mut split_writer, options)
                    .await?;

                let (total_rows, num_files) = split_writer.finish()?;

                if num_files == 1 {
                    crate::split::rename_single_split(base_path)?;
                }

                eprintln!("Exported {total_rows} rows to {num_files} file(s)");
            } else {
                let mut conn = args.conn.connect().await?;
                let rows = conn.export_csv_to_file(source, base_path, options).await?;

                eprintln!("Exported {rows} rows");
            }
        }
        ExportFormat::Parquet => {
            let base_path = Path::new(&args.output);
            let splitting = args.max_rows_per_file.is_some() || args.max_file_size.is_some();

            if splitting {
                let max_rows = args.max_rows_per_file;
                let max_bytes = args
                    .max_file_size
                    .as_deref()
                    .map(crate::size::parse_size)
                    .transpose()?;

                let mut conn = args.conn.connect().await?;

                // Obtain the Arrow schema by running a zero-row query so that
                // export_to_record_batches can parse CSV data correctly.
                let schema_sql = schema_query(&source);
                let rs = conn.execute(schema_sql).await?;
                let arrow_schema =
                    rs.metadata()
                        .map(|m| Arc::clone(&m.schema))
                        .ok_or_else(|| {
                            anyhow::anyhow!("could not determine schema for split export")
                        })?;

                let mut arrow_opts = ArrowExportOptions::new().with_schema(arrow_schema);

                // Set batch_size to match max_rows so that each RecordBatch
                // aligns with the desired per-file row limit, enabling
                // correct file rotation.
                if let Some(mr) = max_rows {
                    arrow_opts = arrow_opts.with_batch_size(mr as usize);
                }

                let batches = conn.export_to_record_batches(source, arrow_opts).await?;

                if batches.is_empty() {
                    eprintln!("Exported 0 rows to 1 file(s)");
                    // Ensure the output path exists even when the query returns no data.
                    let compression = map_compression(args.compression.as_ref());
                    let options = ParquetExportOptions::new().with_compression(compression);
                    conn.export_to_parquet(
                        ExportSource::Query {
                            sql: "SELECT 1 WHERE FALSE".to_string(),
                        },
                        base_path,
                        options,
                    )
                    .await?;
                    return Ok(());
                }

                let schema = batches[0].schema();
                let codec = map_compression_to_codec(args.compression.as_ref());
                let props = WriterProperties::builder().set_compression(codec).build();

                let mut file_index: u32 = 0;
                let mut total_rows: u64 = 0;
                let mut current_file_rows: u64 = 0;

                let current_path = crate::split::split_path(base_path, file_index);
                let file = File::create(&current_path)?;
                let mut writer =
                    ArrowWriter::try_new(file, Arc::clone(&schema), Some(props.clone()))?;

                for batch in &batches {
                    let batch_rows = batch.num_rows() as u64;

                    let row_limit_hit = max_rows.is_some_and(|max| {
                        current_file_rows > 0 && current_file_rows + batch_rows > max
                    });
                    let size_limit_hit = max_bytes.is_some_and(|max| {
                        current_file_rows > 0 && writer.bytes_written() as u64 >= max
                    });

                    if row_limit_hit || size_limit_hit {
                        writer.close()?;
                        file_index += 1;

                        let next_path = crate::split::split_path(base_path, file_index);
                        let file = File::create(&next_path)?;
                        writer =
                            ArrowWriter::try_new(file, Arc::clone(&schema), Some(props.clone()))?;
                        current_file_rows = 0;
                    }

                    writer.write(batch)?;
                    current_file_rows += batch_rows;
                    total_rows += batch_rows;
                }

                writer.close()?;

                let num_files = file_index + 1;
                if file_index == 0 {
                    crate::split::rename_single_split(base_path)?;
                }

                eprintln!("Exported {total_rows} rows to {num_files} file(s)");
            } else {
                let compression = map_compression(args.compression.as_ref());
                let options = ParquetExportOptions::new().with_compression(compression);

                let mut conn = args.conn.connect().await?;
                let rows = conn.export_to_parquet(source, base_path, options).await?;

                eprintln!("Exported {rows} rows");
            }
        }
    }

    Ok(())
}
