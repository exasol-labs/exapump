use arrow::array::Array;
use arrow::record_batch::RecordBatch;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use super::sql::{error_hint, split_statements, write_csv, write_json, StatementType};

const PRIMARY_PROMPT: &str = "exapump> ";
const CONTINUATION_PROMPT: &str = "     > ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveFormat {
    Table,
    Csv,
    Json,
}

impl std::fmt::Display for InteractiveFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Table => write!(f, "table"),
            Self::Csv => write!(f, "csv"),
            Self::Json => write!(f, "json"),
        }
    }
}

#[derive(Debug, PartialEq)]
enum DotCommand {
    Format(Option<String>),
    Help,
    Exit,
    Unknown(String),
}

fn parse_dot_command(line: &str) -> DotCommand {
    let trimmed = line.trim();
    let mut parts = trimmed.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next().map(|s| s.to_string());

    match cmd {
        ".format" => DotCommand::Format(arg),
        ".help" => DotCommand::Help,
        ".exit" => DotCommand::Exit,
        other => DotCommand::Unknown(other.to_string()),
    }
}

fn handle_dot_command(cmd: DotCommand, format: &mut InteractiveFormat) -> ControlFlow {
    match cmd {
        DotCommand::Format(None) => {
            println!("Output format: {}", format);
            ControlFlow::Continue
        }
        DotCommand::Format(Some(arg)) => match arg.as_str() {
            "table" => {
                *format = InteractiveFormat::Table;
                println!("Output format: table");
                ControlFlow::Continue
            }
            "csv" => {
                *format = InteractiveFormat::Csv;
                println!("Output format: csv");
                ControlFlow::Continue
            }
            "json" => {
                *format = InteractiveFormat::Json;
                println!("Output format: json");
                ControlFlow::Continue
            }
            other => {
                println!(
                    "Unknown format '{}'. Valid formats: table, csv, json",
                    other
                );
                ControlFlow::Continue
            }
        },
        DotCommand::Help => {
            println!(".format [table|csv|json]  Set or show output format");
            println!(".help                     Show this help");
            println!(".exit                     Exit the REPL");
            ControlFlow::Continue
        }
        DotCommand::Exit => {
            println!("Bye!");
            ControlFlow::Exit
        }
        DotCommand::Unknown(name) => {
            println!(
                "Unknown command: {}. Type .help for available commands.",
                name
            );
            ControlFlow::Continue
        }
    }
}

enum ControlFlow {
    Continue,
    Exit,
}

/// Append a line to the buffer. Returns `true` when the buffer is ready
/// to execute (i.e. ends with a semicolon).
fn process_line(line: &str, buffer: &mut String) -> bool {
    if !buffer.is_empty() {
        buffer.push('\n');
    }
    buffer.push_str(line);
    buffer.trim().ends_with(';')
}

fn cell_value(col: &dyn Array, row: usize) -> String {
    if col.is_null(row) {
        "NULL".to_string()
    } else {
        arrow::util::display::array_value_to_string(col, row).unwrap_or_else(|_| "?".to_string())
    }
}

fn format_table(batches: &[RecordBatch]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth);

    if let Some(batch) = batches.first() {
        let schema = batch.schema();
        let headers: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
        table.set_header(headers);
    }

    for batch in batches {
        let num_cols = batch.num_columns();
        for row in 0..batch.num_rows() {
            let cells: Vec<String> = (0..num_cols)
                .map(|col| cell_value(batch.column(col).as_ref(), row))
                .collect();
            table.add_row(cells);
        }
    }

    table.to_string()
}

fn row_count(batches: &[RecordBatch]) -> usize {
    batches.iter().map(|b| b.num_rows()).sum()
}

pub async fn run(args: crate::cli::InteractiveArgs) -> anyhow::Result<()> {
    let mut conn = args.conn.connect().await?;

    let mut rl = DefaultEditor::new()?;
    let history_path = std::env::var("HOME")
        .map(|h| format!("{}/.exapump_history", h))
        .unwrap_or_else(|_| ".exapump_history".to_string());
    let _ = rl.load_history(&history_path);

    let version = env!("CARGO_PKG_VERSION");
    println!(
        "exapump v{} \u{2014} Interactive SQL session\nType .help for commands, or enter SQL terminated with ;",
        version
    );

    let mut buffer = String::new();
    let mut format = InteractiveFormat::Table;

    loop {
        let prompt = if buffer.is_empty() {
            PRIMARY_PROMPT
        } else {
            CONTINUATION_PROMPT
        };

        match rl.readline(prompt) {
            Ok(line) => {
                if buffer.is_empty() && line.trim().starts_with('.') {
                    let cmd = parse_dot_command(&line);
                    match handle_dot_command(cmd, &mut format) {
                        ControlFlow::Continue => continue,
                        ControlFlow::Exit => break,
                    }
                }

                let ready = process_line(&line, &mut buffer);

                if ready {
                    let _ = rl.add_history_entry(buffer.as_str());
                    let statements = split_statements(&buffer);
                    buffer.clear();

                    for stmt in &statements {
                        execute_statement(&mut conn, stmt, format).await;
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                if buffer.is_empty() {
                    println!("Bye!");
                    break;
                } else {
                    buffer.clear();
                    println!();
                }
            }
            Err(ReadlineError::Eof) => {
                println!("Bye!");
                break;
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }

    let _ = rl.save_history(&history_path);
    Ok(())
}

async fn execute_statement(
    conn: &mut exarrow_rs::Connection,
    stmt: &str,
    format: InteractiveFormat,
) {
    let stmt_type = StatementType::from_sql(stmt);

    match stmt_type {
        StatementType::Query => match conn.execute(stmt).await {
            Ok(result_set) => match result_set.fetch_all().await {
                Ok(batches) => {
                    let n = row_count(&batches);
                    match format {
                        InteractiveFormat::Table => {
                            let tbl = format_table(&batches);
                            println!("{}", tbl);
                        }
                        InteractiveFormat::Csv => {
                            if let Err(e) = write_csv(&batches) {
                                eprintln!("Error: {}", e);
                            }
                        }
                        InteractiveFormat::Json => {
                            if let Err(e) = write_json(&batches) {
                                eprintln!("Error: {}", e);
                            }
                        }
                    }
                    if n == 1 {
                        println!("1 row");
                    } else {
                        println!("{} rows", n);
                    }
                }
                Err(e) => print_error(&e),
            },
            Err(e) => print_error(&e),
        },
        StatementType::Dml => match conn.execute_update(stmt).await {
            Ok(n) => {
                if n == 1 {
                    println!("1 row affected");
                } else {
                    println!("{} rows affected", n);
                }
            }
            Err(e) => print_error(&e),
        },
        StatementType::Ddl => match conn.execute_update(stmt).await {
            Ok(_) => {
                println!("OK");
            }
            Err(e) => print_error(&e),
        },
    }
}

fn print_error(error: &exarrow_rs::QueryError) {
    eprintln!("Error: {}", error);
    let msg = error.to_string();
    if let Some(hint) = error_hint(&msg) {
        eprintln!("Hint: {}", hint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Int64Array, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use std::sync::Arc;

    // --- process_line tests ---

    #[test]
    fn process_line_single_statement() {
        let mut buf = String::new();
        assert!(process_line("SELECT 1;", &mut buf));
        assert_eq!(buf, "SELECT 1;");
    }

    #[test]
    fn process_line_multi_line() {
        let mut buf = String::new();
        assert!(!process_line("SELECT", &mut buf));
        assert!(!process_line("  1", &mut buf));
        assert!(process_line("  ;", &mut buf));
        assert_eq!(buf, "SELECT\n  1\n  ;");
    }

    #[test]
    fn process_line_no_semicolon() {
        let mut buf = String::new();
        assert!(!process_line("SELECT 1", &mut buf));
    }

    #[test]
    fn process_line_semicolon_with_trailing_whitespace() {
        let mut buf = String::new();
        assert!(process_line("SELECT 1;  ", &mut buf));
    }

    #[test]
    fn process_line_multiple_statements_on_one_line() {
        let mut buf = String::new();
        assert!(process_line("SELECT 1; SELECT 2;", &mut buf));
        let stmts = split_statements(&buf);
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT 1");
        assert_eq!(stmts[1], "SELECT 2");
    }

    // --- dot-command parsing tests ---

    #[test]
    fn parse_format_no_arg() {
        assert_eq!(parse_dot_command(".format"), DotCommand::Format(None));
    }

    #[test]
    fn parse_format_with_arg() {
        assert_eq!(
            parse_dot_command(".format csv"),
            DotCommand::Format(Some("csv".to_string()))
        );
    }

    #[test]
    fn parse_help() {
        assert_eq!(parse_dot_command(".help"), DotCommand::Help);
    }

    #[test]
    fn parse_exit() {
        assert_eq!(parse_dot_command(".exit"), DotCommand::Exit);
    }

    #[test]
    fn parse_unknown() {
        assert_eq!(
            parse_dot_command(".foo"),
            DotCommand::Unknown(".foo".to_string())
        );
    }

    #[test]
    fn parse_dot_command_with_leading_whitespace() {
        assert_eq!(parse_dot_command("  .help"), DotCommand::Help);
    }

    // --- handle_dot_command tests ---

    #[test]
    fn handle_format_show() {
        let mut fmt = InteractiveFormat::Table;
        let result = handle_dot_command(DotCommand::Format(None), &mut fmt);
        assert!(matches!(result, ControlFlow::Continue));
        assert_eq!(fmt, InteractiveFormat::Table);
    }

    #[test]
    fn handle_format_set_csv() {
        let mut fmt = InteractiveFormat::Table;
        handle_dot_command(DotCommand::Format(Some("csv".to_string())), &mut fmt);
        assert_eq!(fmt, InteractiveFormat::Csv);
    }

    #[test]
    fn handle_format_set_json() {
        let mut fmt = InteractiveFormat::Table;
        handle_dot_command(DotCommand::Format(Some("json".to_string())), &mut fmt);
        assert_eq!(fmt, InteractiveFormat::Json);
    }

    #[test]
    fn handle_format_set_table() {
        let mut fmt = InteractiveFormat::Csv;
        handle_dot_command(DotCommand::Format(Some("table".to_string())), &mut fmt);
        assert_eq!(fmt, InteractiveFormat::Table);
    }

    #[test]
    fn handle_format_invalid() {
        let mut fmt = InteractiveFormat::Table;
        handle_dot_command(DotCommand::Format(Some("xml".to_string())), &mut fmt);
        assert_eq!(fmt, InteractiveFormat::Table);
    }

    #[test]
    fn handle_exit_returns_exit() {
        let mut fmt = InteractiveFormat::Table;
        let result = handle_dot_command(DotCommand::Exit, &mut fmt);
        assert!(matches!(result, ControlFlow::Exit));
    }

    #[test]
    fn handle_help_returns_continue() {
        let mut fmt = InteractiveFormat::Table;
        let result = handle_dot_command(DotCommand::Help, &mut fmt);
        assert!(matches!(result, ControlFlow::Continue));
    }

    #[test]
    fn handle_unknown_returns_continue() {
        let mut fmt = InteractiveFormat::Table;
        let result = handle_dot_command(DotCommand::Unknown(".bogus".to_string()), &mut fmt);
        assert!(matches!(result, ControlFlow::Continue));
    }

    // --- InteractiveFormat Display ---

    #[test]
    fn format_display_table() {
        assert_eq!(InteractiveFormat::Table.to_string(), "table");
    }

    #[test]
    fn format_display_csv() {
        assert_eq!(InteractiveFormat::Csv.to_string(), "csv");
    }

    #[test]
    fn format_display_json() {
        assert_eq!(InteractiveFormat::Json.to_string(), "json");
    }

    // --- format_table tests ---

    fn make_batch(names: Vec<&str>, ages: Vec<Option<i64>>) -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new("age", DataType::Int64, true),
        ]));
        let name_array = Arc::new(StringArray::from(names)) as _;
        let age_array = Arc::new(Int64Array::from(ages)) as _;
        RecordBatch::try_new(schema, vec![name_array, age_array]).unwrap()
    }

    #[test]
    fn format_table_with_data() {
        let batch = make_batch(vec!["Alice", "Bob"], vec![Some(30), Some(25)]);
        let output = format_table(&[batch]);
        assert!(output.contains("name"));
        assert!(output.contains("age"));
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
        assert!(output.contains("30"));
        assert!(output.contains("25"));
    }

    #[test]
    fn format_table_with_nulls() {
        let batch = make_batch(vec!["Alice"], vec![None]);
        let output = format_table(&[batch]);
        assert!(output.contains("NULL"));
    }

    #[test]
    fn format_table_empty_batches() {
        let output = format_table(&[]);
        assert!(output.is_empty() || !output.contains("name"));
    }

    #[test]
    fn format_table_multiple_batches() {
        let batch1 = make_batch(vec!["Alice"], vec![Some(30)]);
        let batch2 = make_batch(vec!["Bob"], vec![Some(25)]);
        let output = format_table(&[batch1, batch2]);
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
    }

    #[test]
    fn row_count_across_batches() {
        let batch1 = make_batch(vec!["Alice"], vec![Some(30)]);
        let batch2 = make_batch(vec!["Bob", "Charlie"], vec![Some(25), Some(35)]);
        assert_eq!(row_count(&[batch1, batch2]), 3);
    }

    #[test]
    fn row_count_empty() {
        assert_eq!(row_count(&[]), 0);
    }
}
