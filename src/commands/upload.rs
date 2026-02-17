use crate::cli::UploadArgs;

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

    crate::format::detect_from_path(path)?;

    if args.dry_run {
        dry_run(path, &args.table)
    } else {
        import(path, &args).await
    }
}

fn dry_run(path: &std::path::Path, table: &str) -> anyhow::Result<()> {
    let schema = exarrow_rs::types::infer_schema_from_parquet(
        path,
        exarrow_rs::types::ColumnNameMode::Sanitize,
    )?;

    println!("Columns:");
    for col in &schema.columns {
        println!("  {}: {}", col.ddl_name, col.exasol_type.to_ddl_type());
    }

    let (schema_name, table_name) = parse_table_name(table);
    println!();
    println!("{}", schema.to_ddl(table_name, schema_name));

    Ok(())
}

async fn import(path: &std::path::Path, args: &UploadArgs) -> anyhow::Result<()> {
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
