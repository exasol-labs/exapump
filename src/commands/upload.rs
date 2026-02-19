use crate::cli::UploadArgs;
use crate::format::FileFormat;

/// Splits "schema.table" into (Some("schema"), "table") or (None, "table").
fn parse_table_name(table: &str) -> (Option<&str>, &str) {
    if let Some((schema, name)) = table.split_once('.') {
        (Some(schema), name)
    } else {
        (None, table)
    }
}

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

    let (schema_name, table_name) = parse_table_name(table);
    println!();
    println!("{}", schema.to_ddl(table_name, schema_name));
}

fn parquet_dry_run(path: &std::path::Path, table: &str) -> anyhow::Result<()> {
    let schema = exarrow_rs::types::infer_schema_from_parquet(
        path,
        exarrow_rs::types::ColumnNameMode::Sanitize,
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
    let driver = exarrow_rs::Driver::new();
    let db = driver.open(&args.dsn)?;
    let mut conn = db.connect().await?;

    let options = exarrow_rs::ParquetImportOptions::new()
        .with_create_table_if_not_exists(true)
        .with_column_name_mode(exarrow_rs::types::ColumnNameMode::Sanitize);

    let rows = conn.import_from_parquet(&args.table, path, options).await?;

    println!("Imported {rows} rows");

    Ok(())
}

async fn csv_import(path: &std::path::Path, args: &UploadArgs) -> anyhow::Result<()> {
    let inference_options = build_csv_inference_options(args);
    let schema = exarrow_rs::types::infer_schema_from_csv(path, &inference_options)?;

    let driver = exarrow_rs::Driver::new();
    let db = driver.open(&args.dsn)?;
    let mut conn = db.connect().await?;

    let (schema_name, table_name) = parse_table_name(&args.table);
    let ddl = schema.to_ddl(table_name, schema_name).replacen(
        "CREATE TABLE",
        "CREATE TABLE IF NOT EXISTS",
        1,
    );
    conn.execute(&ddl).await?;

    let mut import_options = exarrow_rs::CsvImportOptions::new()
        .column_separator(args.delimiter)
        .column_delimiter(args.quote)
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
