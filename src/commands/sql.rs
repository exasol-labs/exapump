use std::io::{IsTerminal, Read, Write};

use arrow::array::RecordBatch;

use crate::cli::{OutputFormat, SqlArgs};

const STATUS_LINE_MAX_SQL_LEN: usize = 60;

/// Strip SQL line comments (`-- ...` to end of line) and block comments (`/* ... */`)
/// from the input, preserving single- and double-quoted string literals. Each comment
/// span is replaced with a single space so that adjacent tokens stay separated.
/// Unterminated block comments are treated as a comment to end of input.
pub fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while let Some(c) = chars.next() {
        if in_single_quote {
            out.push(c);
            if c == '\'' {
                in_single_quote = false;
            }
            continue;
        }

        if in_double_quote {
            out.push(c);
            if c == '"' {
                in_double_quote = false;
            }
            continue;
        }

        if c == '-' && chars.peek() == Some(&'-') {
            chars.next();
            out.push(' ');
            while let Some(&next) = chars.peek() {
                if next == '\n' {
                    break;
                }
                chars.next();
            }
            continue;
        }

        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            out.push(' ');
            let mut prev = '\0';
            for next in chars.by_ref() {
                if prev == '*' && next == '/' {
                    break;
                }
                prev = next;
            }
            continue;
        }

        if c == '\'' {
            in_single_quote = true;
        } else if c == '"' {
            in_double_quote = true;
        }

        out.push(c);
    }

    out
}

/// Split SQL input on semicolons using a comment-aware scanner that tracks
/// single-quote, double-quote, line-comment, and block-comment states.
///
/// Comments are preserved verbatim in the returned statements. Semicolons
/// inside string literals or comments do not split. Statements whose only
/// content is whitespace and/or comments are skipped. Unterminated block
/// comments run to the end of input.
pub fn split_statements(input: &str) -> Vec<String> {
    enum ScanState {
        Normal,
        SingleQuote,
        DoubleQuote,
        LineComment,
        BlockComment,
    }

    let mut statements = Vec::new();
    let mut current = String::new();
    let mut state = ScanState::Normal;
    let mut chars = input.chars().peekable();

    let flush = |buf: &mut String, statements: &mut Vec<String>| {
        let trimmed = buf.trim();
        if !trimmed.is_empty() && !strip_comments(trimmed).trim().is_empty() {
            statements.push(trimmed.to_string());
        }
        buf.clear();
    };

    while let Some(ch) = chars.next() {
        match state {
            ScanState::Normal => match ch {
                '\'' => {
                    current.push(ch);
                    state = ScanState::SingleQuote;
                }
                '"' => {
                    current.push(ch);
                    state = ScanState::DoubleQuote;
                }
                '-' if chars.peek() == Some(&'-') => {
                    current.push(ch);
                    current.push(chars.next().unwrap());
                    state = ScanState::LineComment;
                }
                '/' if chars.peek() == Some(&'*') => {
                    current.push(ch);
                    current.push(chars.next().unwrap());
                    state = ScanState::BlockComment;
                }
                ';' => flush(&mut current, &mut statements),
                _ => current.push(ch),
            },
            ScanState::SingleQuote => {
                current.push(ch);
                if ch == '\'' {
                    state = ScanState::Normal;
                }
            }
            ScanState::DoubleQuote => {
                current.push(ch);
                if ch == '"' {
                    state = ScanState::Normal;
                }
            }
            ScanState::LineComment => {
                current.push(ch);
                if ch == '\n' {
                    state = ScanState::Normal;
                }
            }
            ScanState::BlockComment => {
                current.push(ch);
                if ch == '*' && chars.peek() == Some(&'/') {
                    current.push(chars.next().unwrap());
                    state = ScanState::Normal;
                }
            }
        }
    }

    flush(&mut current, &mut statements);

    statements
}

/// Classifies a SQL statement to choose the correct execution path:
/// `Query` returns rows, `Dml` returns a row count, `Ddl` returns OK,
/// and `Execute` (e.g. `EXECUTE SCRIPT`) may return either and must be
/// resolved at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementType {
    Query,
    Dml,
    Ddl,
    Execute,
}

impl StatementType {
    /// Classify a SQL statement by its first keyword, ignoring any leading
    /// comments and whitespace. The original SQL is not modified — comments
    /// are stripped only for the purpose of extracting the keyword.
    pub fn from_sql(sql: &str) -> Self {
        let stripped = strip_comments(sql);
        let first_word = stripped
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_uppercase();

        match first_word.as_str() {
            "SELECT" | "WITH" | "DESCRIBE" | "EXPLAIN" | "SHOW" => StatementType::Query,
            "INSERT" | "UPDATE" | "DELETE" | "MERGE" => StatementType::Dml,
            "EXECUTE" => StatementType::Execute,
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
pub fn error_hint(message: &str) -> Option<&'static str> {
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
pub fn write_csv(batches: &[RecordBatch]) -> anyhow::Result<()> {
    let mut writer = arrow_csv::WriterBuilder::new()
        .with_header(true)
        .build(std::io::stdout());

    for batch in batches {
        writer.write(batch)?;
    }

    Ok(())
}

/// Write record batches as JSON array to stdout.
pub fn write_json(batches: &[RecordBatch]) -> anyhow::Result<()> {
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
            StatementType::Execute => match conn.execute(stmt.as_str()).await {
                Ok(result_set) => {
                    if result_set.is_stream() {
                        match result_set.fetch_all().await {
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
                        }
                    } else {
                        executed += 1;
                        eprintln!(" OK");
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
        Some(sql) if sql == "-" => read_stdin(),
        Some(sql) => Ok(sql.clone()),
        None => {
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

    #[test]
    fn split_preserves_block_comment_hint_prefix() {
        let result = split_statements("/*snapshot execution*/ SELECT 1");
        assert_eq!(result, vec!["/*snapshot execution*/ SELECT 1"]);
    }

    #[test]
    fn split_preserves_leading_block_comment() {
        let result = split_statements("/* hint */ SELECT 1");
        assert_eq!(result, vec!["/* hint */ SELECT 1"]);
    }

    #[test]
    fn split_preserves_leading_line_comment() {
        let result = split_statements("-- a leading comment\nSELECT 1");
        assert_eq!(result, vec!["-- a leading comment\nSELECT 1"]);
    }

    #[test]
    fn split_preserves_trailing_line_comment_per_statement() {
        let result = split_statements("SELECT 1 -- trailing\n; SELECT 2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "SELECT 1 -- trailing");
        assert_eq!(result[1], "SELECT 2");
    }

    #[test]
    fn split_preserves_line_comment_with_semicolons() {
        let result = split_statements("SELECT 1 -- a; b; c\n; SELECT 2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "SELECT 1 -- a; b; c");
        assert_eq!(result[1], "SELECT 2");
    }

    #[test]
    fn split_preserves_block_comment_with_semicolons() {
        let result = split_statements("SELECT /* a; b; c */ 1; SELECT 2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "SELECT /* a; b; c */ 1");
        assert_eq!(result[1], "SELECT 2");
    }

    #[test]
    fn split_preserves_multiline_block_comment() {
        let result = split_statements("/* multi\nline\ncomment */ SELECT 1");
        assert_eq!(result, vec!["/* multi\nline\ncomment */ SELECT 1"]);
    }

    #[test]
    fn split_preserves_unterminated_block_comment_verbatim() {
        let result = split_statements("SELECT 1 /* unterminated");
        assert_eq!(result, vec!["SELECT 1 /* unterminated"]);
    }

    #[test]
    fn split_comment_only_input_yields_no_statements() {
        let result = split_statements("-- just a comment\n/* another */\n");
        assert!(result.is_empty());
    }

    #[test]
    fn split_does_not_start_comment_inside_single_quote() {
        let result = split_statements("SELECT '-- not a comment' AS val; SELECT 2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "SELECT '-- not a comment' AS val");
        assert_eq!(result[1], "SELECT 2");
    }

    #[test]
    fn split_does_not_start_block_comment_inside_single_quote() {
        let result = split_statements("SELECT '/* keep */;' AS val; SELECT 2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "SELECT '/* keep */;' AS val");
        assert_eq!(result[1], "SELECT 2");
    }

    #[test]
    fn split_does_not_start_comment_inside_double_quote() {
        let result = split_statements("SELECT \"x-- y;\" FROM t; SELECT 2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "SELECT \"x-- y;\" FROM t");
        assert_eq!(result[1], "SELECT 2");
    }

    #[test]
    fn split_minus_minus_only_when_two_dashes() {
        let result = split_statements("SELECT 1 - 2; SELECT 3");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "SELECT 1 - 2");
        assert_eq!(result[1], "SELECT 3");
    }

    #[test]
    fn split_slash_star_only_when_followed_by_star() {
        let result = split_statements("SELECT 4 / 2; SELECT 3");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "SELECT 4 / 2");
        assert_eq!(result[1], "SELECT 3");
    }

    #[test]
    fn split_empty_block_comment() {
        let result = split_statements("SELECT /**/ 1; SELECT 2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "SELECT /**/ 1");
        assert_eq!(result[1], "SELECT 2");
    }

    // --- strip_comments tests ---

    #[test]
    fn strip_comments_trailing_line_comment() {
        let result = strip_comments("SELECT 1 -- trailing comment\n; SELECT 2");
        let statements = split_statements(&result);
        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0], "SELECT 1");
        assert_eq!(statements[1], "SELECT 2");
    }

    #[test]
    fn strip_comments_multiline_block() {
        let result = strip_comments("/* multi\nline\ncomment */ SELECT 1");
        assert_eq!(result.trim(), "SELECT 1");
    }

    #[test]
    fn split_preserves_line_comment_with_semicolons_simple() {
        let result = split_statements("SELECT 1 -- a; b; c\n");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "SELECT 1 -- a; b; c");
    }

    #[test]
    fn split_preserves_block_comment_with_semicolons_simple() {
        let result = split_statements("SELECT /* a; b */ 1");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "SELECT /* a; b */ 1");
    }

    #[test]
    fn strip_comments_preserves_string_literal() {
        let result = strip_comments("SELECT '-- not a comment' AS val");
        assert_eq!(result, "SELECT '-- not a comment' AS val");
        let statements = split_statements(&result);
        assert_eq!(statements.len(), 1);
    }

    #[test]
    fn strip_comments_preserves_block_comment_in_string_literal() {
        let result = strip_comments("SELECT '/* keep me */' AS val");
        assert_eq!(result, "SELECT '/* keep me */' AS val");
    }

    #[test]
    fn strip_comments_only_yields_empty() {
        let result = strip_comments("-- just a comment\n/* another */\n");
        let statements = split_statements(&result);
        assert!(statements.is_empty());
    }

    #[test]
    fn strip_comments_line_comment_at_start() {
        let result = strip_comments("-- this is a comment\nSELECT 1");
        let statements = split_statements(&result);
        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0], "SELECT 1");
    }

    #[test]
    fn strip_comments_unterminated_block() {
        let result = strip_comments("SELECT 1 /* unterminated");
        assert_eq!(result.trim(), "SELECT 1");
    }

    #[test]
    fn strip_comments_preserves_utf8() {
        let result = strip_comments("SELECT 'äöü' /* コメント */ AS val");
        assert!(result.contains("'äöü'"));
        assert!(!result.contains("コメント"));
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

    #[test]
    fn statement_type_execute_script() {
        assert_eq!(
            StatementType::from_sql("EXECUTE SCRIPT \"S\".\"HELLO\"()"),
            StatementType::Execute
        );
    }

    #[test]
    fn statement_type_execute_lowercase() {
        assert_eq!(
            StatementType::from_sql("execute script my_script()"),
            StatementType::Execute
        );
    }

    #[test]
    fn classify_with_leading_line_comment() {
        assert_eq!(
            StatementType::from_sql("-- comment\nSELECT 1"),
            StatementType::Query
        );
    }

    #[test]
    fn classify_with_leading_block_comment() {
        assert_eq!(
            StatementType::from_sql("/* hint */ SELECT 1"),
            StatementType::Query
        );
    }

    #[test]
    fn statement_type_block_comment_prefix_select() {
        assert_eq!(
            StatementType::from_sql("/*snapshot execution*/ SELECT 1"),
            StatementType::Query
        );
    }

    #[test]
    fn statement_type_line_comment_prefix_select() {
        assert_eq!(
            StatementType::from_sql("-- leading comment\nSELECT 1"),
            StatementType::Query
        );
    }

    #[test]
    fn statement_type_block_comment_prefix_execute() {
        assert_eq!(
            StatementType::from_sql("/* hint */ EXECUTE SCRIPT s()"),
            StatementType::Execute
        );
    }

    #[test]
    fn statement_type_block_comment_prefix_insert() {
        assert_eq!(
            StatementType::from_sql("/* hint */ INSERT INTO t VALUES (1)"),
            StatementType::Dml
        );
    }

    #[test]
    fn statement_type_comment_only_is_ddl_fallback() {
        assert_eq!(
            StatementType::from_sql("-- only comment\n"),
            StatementType::Ddl
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
