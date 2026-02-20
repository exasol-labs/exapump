use std::io::{IsTerminal, Read, Write};

use arrow::array::RecordBatch;

use crate::cli::{OutputFormat, SqlArgs};

const STATUS_LINE_MAX_SQL_LEN: usize = 60;

/// Split SQL input on semicolons, respecting single-quoted and double-quoted strings.
/// Trailing semicolons produce no empty statements. Whitespace-only statements are skipped.
fn split_statements(input: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    for ch in input.chars() {
        match ch {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                current.push(ch);
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                current.push(ch);
            }
            ';' if !in_single_quote && !in_double_quote => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    statements.push(trimmed);
                }
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        statements.push(trimmed);
    }

    statements
}

/// Classifies a SQL statement as a query (returns rows), DML (returns row count), or DDL (returns OK).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatementType {
    Query,
    Dml,
    Ddl,
}

impl StatementType {
    /// Classify a SQL statement by its first keyword.
    fn from_sql(sql: &str) -> Self {
        let first_word = sql.split_whitespace().next().unwrap_or("").to_uppercase();

        match first_word.as_str() {
            "SELECT" | "WITH" | "DESCRIBE" | "EXPLAIN" | "SHOW" => StatementType::Query,
            "INSERT" | "UPDATE" | "DELETE" | "MERGE" => StatementType::Dml,
            _ => StatementType::Ddl,
        }
    }
}

/// Truncate SQL for status line display. If longer than `max_len`, truncate and append "...".
/// Always trims whitespace first. Safe for multi-byte UTF-8.
fn truncate_sql(sql: &str, max_len: usize) -> String {
    let trimmed = sql.trim();
    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else {
        let boundary = trimmed
            .char_indices()
            .nth(max_len - 3)
            .map(|(i, _)| i)
            .unwrap_or(trimmed.len());
        format!("{}...", &trimmed[..boundary])
    }
}

/// Pattern-match on error message (case-insensitive) to provide actionable hints.
fn error_hint(message: &str) -> Option<&'static str> {
    let lower = message.to_lowercase();

    if lower.contains("not found") && lower.contains("object") {
        Some("Check that the table exists and the schema is correct.")
    } else if lower.contains("insufficient privileges") || lower.contains("not allowed") {
        Some("The user may not have the required permissions.")
    } else if lower.contains("syntax error") {
        Some("Check your SQL syntax near the marked position.")
    } else if lower.contains("connection") || lower.contains("connect") {
        Some("Check the connection string and ensure the database is reachable.")
    } else {
        None
    }
}

/// Format a query error to stderr with contextual information.
fn format_error(stmt_num: usize, sql: &str, error: &exarrow_rs::QueryError) {
    let stderr = std::io::stderr();
    let mut err = stderr.lock();

    match error {
        exarrow_rs::QueryError::SyntaxError { position, message } => {
            let _ = writeln!(err, "Error in statement {}:", stmt_num);
            let _ = writeln!(err, "  {}", sql.trim());

            let pointer_offset = 2 + position;
            let _ = writeln!(
                err,
                "{}^ syntax error at position {}",
                " ".repeat(pointer_offset),
                position
            );
            let _ = writeln!(err);

            if let Some(hint) = error_hint(message) {
                let _ = writeln!(err, "  Hint: {}", hint);
            } else {
                let _ = writeln!(
                    err,
                    "  Hint: Check your SQL syntax near the marked position."
                );
            }
        }
        exarrow_rs::QueryError::ExecutionFailed(message) => {
            let _ = writeln!(err, "Error in statement {}:", stmt_num);
            let _ = writeln!(err, "  {}", sql.trim());
            let _ = writeln!(err);
            let _ = writeln!(err, "  Query execution failed: {}", message);
            let _ = writeln!(err);

            if let Some(hint) = error_hint(message) {
                let _ = writeln!(err, "  Hint: {}", hint);
            }
        }
        other => {
            let _ = writeln!(err, "Error in statement {}:", stmt_num);
            let _ = writeln!(err, "  {}", sql.trim());
            let _ = writeln!(err);
            let _ = writeln!(err, "  {}", other);

            let msg = other.to_string();
            if let Some(hint) = error_hint(&msg) {
                let _ = writeln!(err);
                let _ = writeln!(err, "  Hint: {}", hint);
            }
        }
    }
}

/// Write record batches as CSV to stdout (with header).
fn write_csv(batches: &[RecordBatch]) -> anyhow::Result<()> {
    let mut writer = arrow_csv::WriterBuilder::new()
        .with_header(true)
        .build(std::io::stdout());

    for batch in batches {
        writer.write(batch)?;
    }

    Ok(())
}

/// Write record batches as JSON array to stdout.
fn write_json(batches: &[RecordBatch]) -> anyhow::Result<()> {
    let total_rows: usize = batches.iter().map(|b: &RecordBatch| b.num_rows()).sum();

    if total_rows == 0 {
        print!("[]");
        return Ok(());
    }

    let mut writer = arrow_json::ArrayWriter::new(std::io::stdout());
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;

    Ok(())
}

/// Execute the `sql` subcommand.
pub async fn run(args: SqlArgs) -> anyhow::Result<()> {
    let sql_input = resolve_sql_input(&args)?;

    let statements = split_statements(&sql_input);
    if statements.is_empty() {
        anyhow::bail!("No SQL statements to execute");
    }

    let mut conn = args.conn.connect().await?;

    let total = statements.len();
    let mut executed = 0;
    let mut failed = 0;
    let mut first_select = true;
    let mut exec_error: Option<anyhow::Error> = None;

    for (i, stmt) in statements.iter().enumerate() {
        let stmt_num = i + 1;
        let display_sql = truncate_sql(stmt, STATUS_LINE_MAX_SQL_LEN);
        let stmt_type = StatementType::from_sql(stmt);

        eprint!("[{}/{}] {}", stmt_num, total, display_sql);

        match stmt_type {
            StatementType::Query => match conn.execute(stmt.as_str()).await {
                Ok(result_set) => match result_set.fetch_all().await {
                    Ok(batches) => {
                        let row_count: usize = batches.iter().map(|b| b.num_rows()).sum();
                        eprintln!(" {} rows", row_count);
                        executed += 1;

                        if !first_select {
                            println!();
                        }
                        first_select = false;

                        match args.format {
                            OutputFormat::Csv => write_csv(&batches)?,
                            OutputFormat::Json => write_json(&batches)?,
                        }
                    }
                    Err(e) => {
                        eprintln!();
                        failed += 1;
                        format_error(stmt_num, stmt, &e);
                        exec_error = Some(e.into());
                        break;
                    }
                },
                Err(e) => {
                    eprintln!();
                    failed += 1;
                    format_error(stmt_num, stmt, &e);
                    exec_error = Some(e.into());
                    break;
                }
            },
            StatementType::Dml => match conn.execute_update(stmt.as_str()).await {
                Ok(row_count) => {
                    executed += 1;
                    eprintln!(" {} rows affected", row_count);
                }
                Err(e) => {
                    eprintln!();
                    failed += 1;
                    format_error(stmt_num, stmt, &e);
                    exec_error = Some(e.into());
                    break;
                }
            },
            StatementType::Ddl => match conn.execute_update(stmt.as_str()).await {
                Ok(_) => {
                    executed += 1;
                    eprintln!(" OK");
                }
                Err(e) => {
                    eprintln!();
                    failed += 1;
                    format_error(stmt_num, stmt, &e);
                    exec_error = Some(e.into());
                    break;
                }
            },
        }
    }

    print_summary(executed, failed);

    if let Some(e) = exec_error {
        Err(e)
    } else {
        Ok(())
    }
}

/// Resolve SQL input from positional argument or stdin.
fn resolve_sql_input(args: &SqlArgs) -> anyhow::Result<String> {
    match &args.sql {
        Some(sql) if sql == "-" => {
            // Explicit stdin via "-"
            read_stdin()
        }
        Some(sql) => {
            // SQL provided as positional argument
            Ok(sql.clone())
        }
        None => {
            // No argument: check if stdin is piped
            let stdin = std::io::stdin();
            if stdin.is_terminal() {
                anyhow::bail!("SQL argument is required")
            } else {
                read_stdin()
            }
        }
    }
}

/// Read all of stdin into a string.
fn read_stdin() -> anyhow::Result<String> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    Ok(input)
}

/// Print the final summary line to stderr.
fn print_summary(executed: usize, failed: usize) {
    let total = executed + failed;
    let noun = if total == 1 {
        "statement"
    } else {
        "statements"
    };
    eprintln!("{} {} executed, {} failed", total, noun, failed);
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- split_statements tests ---

    #[test]
    fn split_single_statement() {
        let result = split_statements("SELECT 1");
        assert_eq!(result, vec!["SELECT 1"]);
    }

    #[test]
    fn split_multiple_statements() {
        let result = split_statements("SELECT 1; SELECT 2; SELECT 3");
        assert_eq!(result, vec!["SELECT 1", "SELECT 2", "SELECT 3"]);
    }

    #[test]
    fn split_trailing_semicolon() {
        let result = split_statements("SELECT 1;");
        assert_eq!(result, vec!["SELECT 1"]);
    }

    #[test]
    fn split_multiple_trailing_semicolons() {
        let result = split_statements("SELECT 1;;;");
        assert_eq!(result, vec!["SELECT 1"]);
    }

    #[test]
    fn split_empty_input() {
        let result = split_statements("");
        assert!(result.is_empty());
    }

    #[test]
    fn split_whitespace_only() {
        let result = split_statements("   \n\t  ");
        assert!(result.is_empty());
    }

    #[test]
    fn split_respects_single_quotes() {
        let result = split_statements("SELECT 'hello; world' AS val");
        assert_eq!(result, vec!["SELECT 'hello; world' AS val"]);
    }

    #[test]
    fn split_respects_double_quotes() {
        let result = split_statements("SELECT \"col;name\" FROM t");
        assert_eq!(result, vec!["SELECT \"col;name\" FROM t"]);
    }

    #[test]
    fn split_mixed_quotes_and_semicolons() {
        let result = split_statements("INSERT INTO t VALUES ('a;b'); SELECT \"x;y\" FROM t");
        assert_eq!(
            result,
            vec!["INSERT INTO t VALUES ('a;b')", "SELECT \"x;y\" FROM t"]
        );
    }

    #[test]
    fn split_whitespace_between_statements() {
        let result = split_statements("  SELECT 1  ;  SELECT 2  ");
        assert_eq!(result, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn split_semicolons_only() {
        let result = split_statements(";;;");
        assert!(result.is_empty());
    }

    #[test]
    fn split_multiline_statements() {
        let input = "CREATE TABLE t (\n  id INT\n);\nINSERT INTO t VALUES (1)";
        let result = split_statements(input);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "CREATE TABLE t (\n  id INT\n)");
        assert_eq!(result[1], "INSERT INTO t VALUES (1)");
    }

    // --- StatementType tests ---

    #[test]
    fn statement_type_select() {
        assert_eq!(StatementType::from_sql("SELECT 1"), StatementType::Query);
    }

    #[test]
    fn statement_type_with() {
        assert_eq!(
            StatementType::from_sql("WITH cte AS (SELECT 1) SELECT * FROM cte"),
            StatementType::Query
        );
    }

    #[test]
    fn statement_type_describe() {
        assert_eq!(
            StatementType::from_sql("DESCRIBE my_table"),
            StatementType::Query
        );
    }

    #[test]
    fn statement_type_explain() {
        assert_eq!(
            StatementType::from_sql("EXPLAIN VIRTUAL SELECT 1"),
            StatementType::Query
        );
    }

    #[test]
    fn statement_type_show() {
        assert_eq!(StatementType::from_sql("SHOW TABLES"), StatementType::Query);
    }

    #[test]
    fn statement_type_create() {
        assert_eq!(
            StatementType::from_sql("CREATE TABLE t (id INT)"),
            StatementType::Ddl
        );
    }

    #[test]
    fn statement_type_insert() {
        assert_eq!(
            StatementType::from_sql("INSERT INTO t VALUES (1)"),
            StatementType::Dml
        );
    }

    #[test]
    fn statement_type_update() {
        assert_eq!(
            StatementType::from_sql("UPDATE t SET col = 1"),
            StatementType::Dml
        );
    }

    #[test]
    fn statement_type_delete() {
        assert_eq!(
            StatementType::from_sql("DELETE FROM t WHERE id = 1"),
            StatementType::Dml
        );
    }

    #[test]
    fn statement_type_drop() {
        assert_eq!(StatementType::from_sql("DROP TABLE t"), StatementType::Ddl);
    }

    #[test]
    fn statement_type_grant() {
        assert_eq!(
            StatementType::from_sql("GRANT SELECT ON t TO user1"),
            StatementType::Ddl
        );
    }

    #[test]
    fn statement_type_alter() {
        assert_eq!(
            StatementType::from_sql("ALTER TABLE t ADD COLUMN c INT"),
            StatementType::Ddl
        );
    }

    #[test]
    fn statement_type_case_insensitive() {
        assert_eq!(StatementType::from_sql("select 1"), StatementType::Query);
        assert_eq!(StatementType::from_sql("Select 1"), StatementType::Query);
    }

    #[test]
    fn statement_type_leading_whitespace() {
        assert_eq!(StatementType::from_sql("  SELECT 1"), StatementType::Query);
    }

    #[test]
    fn statement_type_empty_string() {
        assert_eq!(StatementType::from_sql(""), StatementType::Ddl);
    }

    #[test]
    fn statement_type_merge() {
        assert_eq!(
            StatementType::from_sql("MERGE INTO t USING s ON t.id = s.id"),
            StatementType::Dml
        );
    }

    // --- truncate_sql tests ---

    #[test]
    fn truncate_short_sql() {
        assert_eq!(truncate_sql("SELECT 1", 60), "SELECT 1");
    }

    #[test]
    fn truncate_exact_length() {
        let sql = "a".repeat(60);
        assert_eq!(truncate_sql(&sql, 60), sql);
    }

    #[test]
    fn truncate_long_sql() {
        let sql = "a".repeat(80);
        let result = truncate_sql(&sql, 60);
        assert_eq!(result.len(), 60);
        assert!(result.ends_with("..."));
        assert_eq!(&result[..57], &"a".repeat(57));
    }

    #[test]
    fn truncate_trims_whitespace() {
        assert_eq!(truncate_sql("  SELECT 1  ", 60), "SELECT 1");
    }

    #[test]
    fn truncate_multibyte_utf8() {
        let sql = format!("SELECT '{}'", "ä".repeat(40));
        let result = truncate_sql(&sql, 60);
        assert!(result.ends_with("..."));
        // Must not panic -- the key property is valid UTF-8, not byte-length
        assert!(result.chars().count() <= 60);
    }

    // --- error_hint tests ---

    #[test]
    fn hint_object_not_found() {
        assert_eq!(
            error_hint("object NONEXISTENT_TABLE not found"),
            Some("Check that the table exists and the schema is correct.")
        );
    }

    #[test]
    fn hint_insufficient_privileges() {
        assert_eq!(
            error_hint("insufficient privileges for operation"),
            Some("The user may not have the required permissions.")
        );
    }

    #[test]
    fn hint_not_allowed() {
        assert_eq!(
            error_hint("operation not allowed"),
            Some("The user may not have the required permissions.")
        );
    }

    #[test]
    fn hint_syntax_error() {
        assert_eq!(
            error_hint("syntax error near SELECT"),
            Some("Check your SQL syntax near the marked position.")
        );
    }

    #[test]
    fn hint_connection_error() {
        assert_eq!(
            error_hint("connection refused"),
            Some("Check the connection string and ensure the database is reachable.")
        );
    }

    #[test]
    fn hint_no_match() {
        assert_eq!(error_hint("unknown error occurred"), None);
    }
}
