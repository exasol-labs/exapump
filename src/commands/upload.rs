use crate::cli::UploadArgs;
use crate::format::FileFormat;

/// Executes the upload command: validates input, then either previews schema or imports data.
pub async fn run(args: UploadArgs) -> anyhow::Result<()> {
    let file = &args.files[0];
    let path = std::path::Path::new(file);

    if !path.exists() {
        anyhow::bail!("file not found: {}", path.display());
    }

    let format = crate::format::detect_from_path(path)?;

    match (format, args.dry_run) {
        (FileFormat::Parquet, true) => parquet_dry_run(path, &args.table),
        (FileFormat::Parquet, false) => parquet_import(path, &args).await,
        (FileFormat::Csv, true) => csv_dry_run(path, &args),
        (FileFormat::Csv, false) => csv_import(path, &args).await,
    }
}

fn build_csv_inference_options(args: &UploadArgs) -> exarrow_rs::types::CsvInferenceOptions {
    exarrow_rs::types::CsvInferenceOptions::new()
        .with_delimiter(args.delimiter as u8)
        .with_has_header(!args.no_header)
        .with_quote(Some(args.quote as u8))
        .with_escape(args.escape.map(|c| c as u8))
        .with_null_regex(Some(format!("^{}$", regex::escape(&args.null_value))))
        .with_column_name_mode(exarrow_rs::types::ColumnNameMode::Quoted)
}

fn print_schema(schema: &exarrow_rs::types::InferredTableSchema, table: &str) {
    println!("Columns:");
    for col in &schema.columns {
        println!("  {}: {}", col.ddl_name, col.exasol_type.to_ddl_type());
    }

    let (schema_name, table_name) = super::parse_table_name(table);
    println!();
    println!("{}", schema.to_ddl(table_name, schema_name));
}

fn parquet_dry_run(path: &std::path::Path, table: &str) -> anyhow::Result<()> {
    let schema = exarrow_rs::types::infer_schema_from_parquet(
        path,
        exarrow_rs::types::ColumnNameMode::Quoted,
    )?;

    print_schema(&schema, table);

    Ok(())
}

fn csv_dry_run(path: &std::path::Path, args: &UploadArgs) -> anyhow::Result<()> {
    let options = build_csv_inference_options(args);
    let schema = exarrow_rs::types::infer_schema_from_csv(path, &options)?;

    print_schema(&schema, &args.table);

    Ok(())
}

async fn parquet_import(path: &std::path::Path, args: &UploadArgs) -> anyhow::Result<()> {
    let schema = exarrow_rs::types::infer_schema_from_parquet(
        path,
        exarrow_rs::types::ColumnNameMode::Quoted,
    )?;

    let mut conn = args.conn.connect().await?;

    let (schema_name, table_name) = super::parse_table_name(&args.table);
    let ddl = schema.to_ddl(table_name, schema_name).replacen(
        "CREATE TABLE",
        "CREATE TABLE IF NOT EXISTS",
        1,
    );
    conn.execute(&ddl).await?;

    let options = exarrow_rs::ParquetImportOptions::new()
        .with_column_name_mode(exarrow_rs::types::ColumnNameMode::Quoted)
        .with_native_parquet(Some(false));

    let rows = conn.import_from_parquet(&args.table, path, options).await?;

    println!("Imported {rows} rows");

    Ok(())
}

/// Pick the `ROW SEPARATOR` for the IMPORT statement from the file's own line
/// endings.
///
/// Exasol applies one separator to the whole file, and naming the wrong one is
/// silent: `LF` against a CRLF file appends a `\r` to the last column of every
/// row, and `CRLF` against an LF file yields zero imported rows. A file that
/// mixes both styles has no correct answer, so it is refused rather than
/// half-loaded.
fn resolve_row_separator(
    path: &std::path::Path,
    args: &UploadArgs,
) -> anyhow::Result<exarrow_rs::ImportRowSeparator> {
    let endings =
        crate::csv_dialect::detect_row_endings(path, args.quote as u8, args.delimiter as u8)?;

    match endings {
        crate::csv_dialect::RowEndings::Lf => Ok(exarrow_rs::ImportRowSeparator::LF),
        crate::csv_dialect::RowEndings::Crlf => Ok(exarrow_rs::ImportRowSeparator::CRLF),
        crate::csv_dialect::RowEndings::Mixed { crlf, lf } => anyhow::bail!(
            "{}: mixed line endings — {crlf} rows end with CRLF and {lf} with LF. \
             Exasol imports a file under a single row separator, so part of the data \
             would be misread. Convert the file to one style first, for example \
             `dos2unix {}` or `sed -i 's/\\r$//' {}`, then upload it again.",
            path.display(),
            path.display(),
            path.display(),
        ),
    }
}

async fn csv_import(path: &std::path::Path, args: &UploadArgs) -> anyhow::Result<()> {
    let inference_options = build_csv_inference_options(args);
    let schema = exarrow_rs::types::infer_schema_from_csv(path, &inference_options)?;
    let row_separator = resolve_row_separator(path, args)?;

    let mut conn = args.conn.connect().await?;

    let (schema_name, table_name) = super::parse_table_name(&args.table);
    let ddl = schema.to_ddl(table_name, schema_name).replacen(
        "CREATE TABLE",
        "CREATE TABLE IF NOT EXISTS",
        1,
    );
    conn.execute(&ddl).await?;

    let mut import_options = exarrow_rs::CsvImportOptions::new()
        .column_separator(args.delimiter)
        .column_delimiter(args.quote)
        .row_separator(row_separator)
        .skip_rows(if args.no_header { 0 } else { 1 });

    if !args.null_value.is_empty() {
        import_options = import_options.null_value(&args.null_value);
    }

    let rows = conn
        .import_csv_from_file(&args.table, path, import_options)
        .await?;

    println!("Imported {rows} rows");

    Ok(())
}
