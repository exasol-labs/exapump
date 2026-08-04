use std::io::Write;

use arrow::array::Array;
use arrow::record_batch::RecordBatch;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use super::sql::{
    error_hint, execute_one, split_statements, total_rows, write_csv, write_json, StatementOutcome,
};

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

#[derive(Debug, PartialEq)]
enum LineKind {
    Dot(DotCommand),
    Sql,
}

/// Classify one line of REPL input against whether the statement buffer is
/// currently empty. A leading `.` starts a dot-command only at a statement
/// boundary; the same text mid-buffer is ordinary SQL, since multi-line SQL
/// text may legitimately contain a line starting with `.`.
fn classify_line(line: &str, buffer_is_empty: bool) -> LineKind {
    if buffer_is_empty && line.trim().starts_with('.') {
        LineKind::Dot(parse_dot_command(line))
    } else {
        LineKind::Sql
    }
}

/// Build the line editor and load its persisted history file, creating the
/// history file's parent directory first if it is missing. Returns the
/// editor together with the path `run` later saves history back to.
fn init_editor() -> anyhow::Result<(DefaultEditor, std::path::PathBuf)> {
    let mut rl = DefaultEditor::new()?;
    let history_path = dirs::home_dir()
        .map(|h| h.join(".exapump").join("history"))
        .unwrap_or_else(|| std::path::PathBuf::from(".exapump/history"));
    if let Some(parent) = history_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = rl.load_history(&history_path);
    Ok((rl, history_path))
}

fn print_banner() {
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "exapump v{} \u{2014} Interactive SQL session\nType .help for commands, or enter SQL terminated with ;",
        version
    );
}

/// The REPL's mutable state across loop iterations: the database connection,
/// the line editor, the in-progress statement buffer, and the current output
/// format. Grouping them lets [`ReplSession::dispatch_line`] take a single
/// receiver instead of four independent `&mut` parameters.
struct ReplSession {
    conn: exarrow_rs::Connection,
    rl: DefaultEditor,
    buffer: String,
    format: InteractiveFormat,
}

impl ReplSession {
    /// Handle one line read from the REPL: a dot-command dispatches immediately,
    /// while SQL text is appended to the buffer and executed once it ends in `;`.
    /// Returns whether the REPL loop should continue reading or exit.
    async fn dispatch_line(&mut self, line: &str) -> ControlFlow {
        match classify_line(line, self.buffer.is_empty()) {
            LineKind::Dot(cmd) => handle_dot_command(cmd, &mut self.format),
            LineKind::Sql => {
                let ready = process_line(line, &mut self.buffer);
                if ready {
                    let _ = self.rl.add_history_entry(self.buffer.as_str());
                    let statements = split_statements(&self.buffer);
                    self.buffer.clear();

                    for stmt in &statements {
                        execute_statement(&mut self.conn, stmt, self.format).await;
                    }
                }
                ControlFlow::Continue
            }
        }
    }
}

pub async fn run(args: crate::cli::InteractiveArgs) -> anyhow::Result<()> {
    let conn = args.conn.connect().await?;
    let (rl, history_path) = init_editor()?;
    print_banner();

    let mut session = ReplSession {
        conn,
        rl,
        buffer: String::new(),
        format: InteractiveFormat::Table,
    };

    loop {
        let prompt = if session.buffer.is_empty() {
            PRIMARY_PROMPT
        } else {
            CONTINUATION_PROMPT
        };

        match session.rl.readline(prompt) {
            Ok(line) => match session.dispatch_line(&line).await {
                ControlFlow::Continue => continue,
                ControlFlow::Exit => break,
            },
            Err(ReadlineError::Interrupted) => {
                if session.buffer.is_empty() {
                    println!("Bye!");
                    break;
                } else {
                    session.buffer.clear();
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

    let _ = session.rl.save_history(&history_path);
    Ok(())
}

/// Render `batches` in `format` to `writer`. Table output is emitted as a
/// single `writeln!` so its bytes match the REPL's original `println!`.
fn render_batches(
    batches: &[RecordBatch],
    format: InteractiveFormat,
    writer: &mut impl Write,
) -> anyhow::Result<()> {
    match format {
        InteractiveFormat::Table => writeln!(writer, "{}", format_table(batches))?,
        InteractiveFormat::Csv => write_csv(batches, writer)?,
        InteractiveFormat::Json => write_json(batches, writer)?,
    }
    Ok(())
}

/// The REPL's trailing count line for a query result; singular for one row.
fn row_count_line(rows: usize) -> String {
    if rows == 1 {
        "1 row".to_string()
    } else {
        format!("{} rows", rows)
    }
}

/// The REPL's trailing count line for a DML statement; singular for one row.
fn rows_affected_line(rows: i64) -> String {
    if rows == 1 {
        "1 row affected".to_string()
    } else {
        format!("{} rows affected", rows)
    }
}

/// Print one query result to stdout: the rendered batches, then the row count.
/// A rendering failure is reported without suppressing the count line, which is
/// how the REPL has always treated a half-written CSV or JSON result.
fn print_result(batches: &[RecordBatch], format: InteractiveFormat) {
    if let Err(e) = render_batches(batches, format, &mut std::io::stdout()) {
        eprintln!("Error: {}", e);
    }
    println!("{}", row_count_line(total_rows(batches)));
}

/// Execute one statement against `conn` and print its outcome in `format`.
/// How a statement kind is run belongs to [`execute_one`]; this function owns
/// only how the REPL reports the result, which is the one thing that differs
/// from `exapump sql`. Query errors are returned rather than reported here so
/// that every statement kind shares a single error-reporting site in
/// [`execute_statement`].
async fn execute_and_report(
    conn: &mut exarrow_rs::Connection,
    stmt: &str,
    format: InteractiveFormat,
) -> Result<(), exarrow_rs::QueryError> {
    match execute_one(conn, stmt).await? {
        StatementOutcome::Rows(batches) => print_result(&batches, format),
        StatementOutcome::RowsAffected(rows) => println!("{}", rows_affected_line(rows)),
        StatementOutcome::Ok => println!("OK"),
    }
    Ok(())
}

async fn execute_statement(
    conn: &mut exarrow_rs::Connection,
    stmt: &str,
    format: InteractiveFormat,
) {
    if let Err(e) = execute_and_report(conn, stmt, format).await {
        print_error(&e);
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
    use super::super::sql::StatementType;
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

    // --- classify_line tests ---

    #[test]
    fn classify_line_recognizes_a_dot_command_at_a_statement_boundary() {
        assert_eq!(
            classify_line(".help", true),
            LineKind::Dot(DotCommand::Help)
        );
    }

    #[test]
    fn classify_line_recognizes_a_dot_command_with_leading_whitespace() {
        assert_eq!(
            classify_line("  .exit", true),
            LineKind::Dot(DotCommand::Exit)
        );
    }

    #[test]
    fn classify_line_recognizes_the_dot_command_variant_and_its_argument() {
        assert_eq!(
            classify_line(".format csv", true),
            LineKind::Dot(DotCommand::Format(Some("csv".to_string())))
        );
    }

    #[test]
    fn classify_line_recognizes_an_unknown_dot_command() {
        assert_eq!(
            classify_line(".bogus", true),
            LineKind::Dot(DotCommand::Unknown(".bogus".to_string()))
        );
    }

    #[test]
    fn classify_line_treats_a_leading_dot_mid_statement_as_sql() {
        assert_eq!(classify_line(".help", false), LineKind::Sql);
    }

    #[test]
    fn classify_line_treats_ordinary_sql_as_sql() {
        assert_eq!(classify_line("SELECT 1;", true), LineKind::Sql);
    }

    #[test]
    fn classify_line_treats_an_empty_line_as_sql() {
        assert_eq!(classify_line("", true), LineKind::Sql);
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

    // --- render_batches tests ---

    #[test]
    fn render_batches_table_format_writes_the_formatted_table_and_one_trailing_newline() {
        let batch = make_batch(vec!["Alice"], vec![Some(30)]);
        let mut buf: Vec<u8> = Vec::new();
        render_batches(
            std::slice::from_ref(&batch),
            InteractiveFormat::Table,
            &mut buf,
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            format!("{}\n", format_table(std::slice::from_ref(&batch)))
        );
    }

    #[test]
    fn render_batches_table_format_writes_the_empty_table_and_one_newline_for_no_batches() {
        let mut buf: Vec<u8> = Vec::new();
        render_batches(&[], InteractiveFormat::Table, &mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            format!("{}\n", format_table(&[]))
        );
    }

    #[test]
    fn render_batches_csv_format_writes_a_header_and_every_row() {
        let batch = make_batch(vec!["Alice", "Bob"], vec![Some(30), Some(25)]);
        let mut buf: Vec<u8> = Vec::new();
        render_batches(&[batch], InteractiveFormat::Csv, &mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "name,age\nAlice,30\nBob,25\n"
        );
    }

    #[test]
    fn render_batches_csv_format_writes_one_header_across_multiple_batches() {
        let first = make_batch(vec!["Alice"], vec![Some(30)]);
        let second = make_batch(vec!["Bob"], vec![Some(25)]);
        let mut buf: Vec<u8> = Vec::new();
        render_batches(&[first, second], InteractiveFormat::Csv, &mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "name,age\nAlice,30\nBob,25\n"
        );
    }

    #[test]
    fn render_batches_csv_format_writes_nothing_for_no_batches() {
        let mut buf: Vec<u8> = Vec::new();
        render_batches(&[], InteractiveFormat::Csv, &mut buf).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "");
    }

    #[test]
    fn render_batches_json_format_writes_a_json_array_of_rows() {
        let batch = make_batch(vec!["Alice"], vec![Some(30)]);
        let mut buf: Vec<u8> = Vec::new();
        render_batches(&[batch], InteractiveFormat::Json, &mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            r#"[{"name":"Alice","age":30}]"#
        );
    }

    #[test]
    fn render_batches_json_format_writes_an_empty_array_for_no_batches() {
        let mut buf: Vec<u8> = Vec::new();
        render_batches(&[], InteractiveFormat::Json, &mut buf).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "[]");
    }

    // --- row_count_line / rows_affected_line tests ---

    #[test]
    fn row_count_line_is_singular_for_exactly_one_row() {
        assert_eq!(row_count_line(1), "1 row");
    }

    #[test]
    fn row_count_line_is_plural_for_no_rows() {
        assert_eq!(row_count_line(0), "0 rows");
    }

    #[test]
    fn row_count_line_is_plural_for_many_rows() {
        assert_eq!(row_count_line(42), "42 rows");
    }

    #[test]
    fn rows_affected_line_is_singular_for_exactly_one_row() {
        assert_eq!(rows_affected_line(1), "1 row affected");
    }

    #[test]
    fn rows_affected_line_is_plural_for_no_rows() {
        assert_eq!(rows_affected_line(0), "0 rows affected");
    }

    #[test]
    fn rows_affected_line_is_plural_for_many_rows() {
        assert_eq!(rows_affected_line(7), "7 rows affected");
    }

    #[test]
    fn repl_classify_block_comment_hint_prefix_is_query() {
        assert_eq!(
            StatementType::from_sql("/*snapshot execution*/ SELECT 1"),
            StatementType::Query
        );
    }

    #[test]
    fn repl_classify_line_comment_prefix_is_query() {
        assert_eq!(
            StatementType::from_sql("-- a comment\nSELECT 1"),
            StatementType::Query
        );
    }
}
