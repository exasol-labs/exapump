use std::path::Path;

use exarrow_rs::{CsvExportOptions, ExportSource};

use crate::cli::{ExportArgs, ExportFormat};

/// Executes the export command: exports a table or query result to a CSV file.
pub async fn run(args: ExportArgs) -> anyhow::Result<()> {
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

            let path = Path::new(&args.output);
            let mut conn = args.conn.connect().await?;
            let rows = conn.export_csv_to_file(source, path, options).await?;

            eprintln!("Exported {rows} rows");
        }
    }

    Ok(())
}
