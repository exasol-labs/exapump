use std::io::{IsTerminal, Read, Write};
use std::iter::Peekable;
use std::str::Chars;

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
    let mut quote: Option<char> = None;

    while let Some(c) = chars.next() {
        if let Some(delimiter) = quote {
            out.push(c);
            if c == delimiter {
                quote = None;
            }
            continue;
        }

        if skip_comment(c, &mut chars) {
            out.push(' ');
            continue;
        }

        if c == '\'' || c == '"' {
            quote = Some(c);
        }
        out.push(c);
    }

    out
}

/// Consume the comment that `c` opens, reporting whether it opened one at all.
/// `chars` is left untouched when `c` opens no comment, so the caller can fall
/// through to its ordinary handling of `c`.
fn skip_comment(c: char, chars: &mut Peekable<Chars<'_>>) -> bool {
    if c == '-' && chars.peek() == Some(&'-') {
        chars.next();
        skip_to_line_end(chars);
        return true;
    }

    if c == '/' && chars.peek() == Some(&'*') {
        chars.next();
        skip_to_block_end(chars);
        return true;
    }

    false
}

/// Consume up to, but not including, the newline that ends the current line, so
/// the caller still sees that newline as an ordinary character.
fn skip_to_line_end(chars: &mut Peekable<Chars<'_>>) {
    while let Some(&next) = chars.peek() {
        if next == '\n' {
            break;
        }
        chars.next();
    }
}

/// Consume through the `*/` that closes the current block comment, or to the end
/// of input when the comment is unterminated.
fn skip_to_block_end(chars: &mut Peekable<Chars<'_>>) {
    let mut prev = '\0';
    for next in chars.by_ref() {
        if prev == '*' && next == '/' {
            break;
        }
        prev = next;
    }
}

/// Split SQL input on semicolons using a comment-aware scanner that tracks
/// single-quote, double-quote, line-comment, and block-comment states.
///
/// Comments are preserved verbatim in the returned statements. Semicolons
/// inside string literals or comments do not split. Statements whose only
/// content is whitespace and/or comments are skipped. Unterminated block
/// comments run to the end of input.
pub fn split_statements(input: &str) -> Vec<String> {
    StatementScanner::new(input).scan()
}

#[derive(Clone, Copy)]
enum ScanState {
    Normal,
    SingleQuote,
    DoubleQuote,
    LineComment,
    BlockComment,
    ScriptBody,
}

/// Tracks progress through the `CREATE … SCRIPT … AS` header in Normal state.
#[derive(Clone, Copy)]
enum HeaderKeyword {
    None,
    SawCreate,
    SawScript,
}

/// Character-at-a-time scanner behind `split_statements`. It owns the scan
/// cursor and the statement being built so that each state's transition rule is
/// a method of its own rather than an arm of one oversized loop; the loop in
/// `scan` then only routes a character to the rule the current state names.
struct StatementScanner<'a> {
    chars: Peekable<Chars<'a>>,
    state: ScanState,
    statements: Vec<String>,
    current: String,
    header: HeaderKeyword,
    word: String,
    line_start: bool,
}

impl<'a> StatementScanner<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
            state: ScanState::Normal,
            statements: Vec::new(),
            current: String::new(),
            header: HeaderKeyword::None,
            word: String::new(),
            line_start: true,
        }
    }

    fn scan(mut self) -> Vec<String> {
        while let Some(ch) = self.chars.next() {
            match self.state {
                ScanState::Normal => self.scan_normal(ch),
                ScanState::ScriptBody => self.scan_script_body(ch),
                ScanState::SingleQuote => self.copy_until(ch, '\''),
                ScanState::DoubleQuote => self.copy_until(ch, '"'),
                ScanState::LineComment => self.copy_until(ch, '\n'),
                ScanState::BlockComment => self.scan_block_comment(ch),
            }
        }

        self.flush();
        self.statements
    }

    fn scan_normal(&mut self, ch: char) {
        if ch.is_alphanumeric() || ch == '_' {
            self.word.push(ch);
            self.current.push(ch);
            return;
        }

        if self.finish_word() {
            self.header = HeaderKeyword::None;
            self.state = ScanState::ScriptBody;
            self.current.push(ch);
            self.line_start = ch == '\n' || ch == ' ' || ch == '\t';
            return;
        }

        match ch {
            '\'' => {
                self.current.push(ch);
                self.state = ScanState::SingleQuote;
            }
            '"' => {
                self.current.push(ch);
                self.state = ScanState::DoubleQuote;
            }
            '-' if self.chars.peek() == Some(&'-') => {
                self.current.push(ch);
                self.current.push(self.chars.next().unwrap());
                self.state = ScanState::LineComment;
            }
            '/' if self.chars.peek() == Some(&'*') => {
                self.current.push(ch);
                self.current.push(self.chars.next().unwrap());
                self.state = ScanState::BlockComment;
            }
            ';' => {
                self.header = HeaderKeyword::None;
                self.flush();
            }
            _ => self.current.push(ch),
        }
    }

    /// Advance the scan cursor through a `CREATE ... SCRIPT ... AS` body. A `/`
    /// that starts its own line (and is followed only by a newline or the end
    /// of input) is the lone-slash terminator Exasol scripts use in place of a
    /// semicolon: it is not pushed onto the current statement, and the newline
    /// that began that line is dropped along with it so the flushed body does
    /// not end with a trailing blank line.
    fn scan_script_body(&mut self, ch: char) {
        if self.line_start && ch == '/' && matches!(self.chars.peek(), Some('\n') | None) {
            if self.current.ends_with('\n') {
                self.current.pop();
            }
            self.flush();
            self.header = HeaderKeyword::None;
            self.state = ScanState::Normal;
            return;
        }

        self.current.push(ch);
        if ch == '\n' {
            self.line_start = true;
        } else if ch != ' ' && ch != '\t' {
            self.line_start = false;
        }
    }

    fn copy_until(&mut self, ch: char, terminator: char) {
        self.current.push(ch);
        if ch == terminator {
            self.state = ScanState::Normal;
        }
    }

    fn scan_block_comment(&mut self, ch: char) {
        self.current.push(ch);
        if ch == '*' && self.chars.peek() == Some(&'/') {
            self.current.push(self.chars.next().unwrap());
            self.state = ScanState::Normal;
        }
    }

    /// Close off the token that just ended at a word boundary, reporting whether
    /// it was the `AS` that completes a `CREATE … SCRIPT … AS` header.
    fn finish_word(&mut self) -> bool {
        if self.word.is_empty() {
            return false;
        }

        let entered = self.advance_header();
        self.word.clear();
        entered
    }

    fn advance_header(&mut self) -> bool {
        let upper = self.word.to_uppercase();
        match self.header {
            HeaderKeyword::None => {
                if upper == "CREATE" {
                    self.header = HeaderKeyword::SawCreate;
                }
            }
            HeaderKeyword::SawCreate => {
                if upper == "SCRIPT" {
                    self.header = HeaderKeyword::SawScript;
                }
            }
            HeaderKeyword::SawScript => {
                if upper == "AS" {
                    return true;
                }
            }
        }
        false
    }

    /// Emit the statement built so far, dropping it when it holds nothing but
    /// whitespace and comments, and start a fresh one either way.
    fn flush(&mut self) {
        let trimmed = self.current.trim();
        if !trimmed.is_empty() && !strip_comments(trimmed).trim().is_empty() {
            self.statements.push(trimmed.to_string());
        }
        self.current.clear();
    }
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

/// Sum the row count across every batch.
pub(crate) fn total_rows(batches: &[RecordBatch]) -> usize {
    batches.iter().map(|b| b.num_rows()).sum()
}

/// Write record batches as CSV (with header) to `writer`.
pub fn write_csv(batches: &[RecordBatch], writer: &mut impl Write) -> anyhow::Result<()> {
    let mut writer = arrow_csv::WriterBuilder::new()
        .with_header(true)
        .build(writer);

    for batch in batches {
        writer.write(batch)?;
    }

    Ok(())
}

/// Write record batches as a JSON array to `writer`.
pub fn write_json(batches: &[RecordBatch], writer: &mut impl Write) -> anyhow::Result<()> {
    if total_rows(batches) == 0 {
        write!(writer, "[]")?;
        return Ok(());
    }

    let mut json_writer = arrow_json::ArrayWriter::new(writer);
    for batch in batches {
        json_writer.write(batch)?;
    }
    json_writer.finish()?;

    Ok(())
}

/// Renders each statement's query results in the requested output format,
/// tracking across calls whether a result has already been rendered so it
/// can insert a blank line before every result but the first (keeping
/// consecutive `SELECT`s visually separated).
struct ResultRenderer {
    rendered_any: bool,
}

impl ResultRenderer {
    fn new() -> Self {
        Self {
            rendered_any: false,
        }
    }

    fn render(
        &mut self,
        batches: &[RecordBatch],
        format: &OutputFormat,
        writer: &mut impl Write,
    ) -> anyhow::Result<()> {
        if self.rendered_any {
            writeln!(writer)?;
        }
        self.rendered_any = true;

        match format {
            OutputFormat::Csv => write_csv(batches, writer),
            OutputFormat::Json => write_json(batches, writer),
        }
    }
}

/// Build the `"[i/n] sql"` progress prefix printed before a statement's outcome.
fn status_line_prefix(stmt_num: usize, total: usize, sql: &str) -> String {
    format!(
        "[{}/{}] {}",
        stmt_num,
        total,
        truncate_sql(sql, STATUS_LINE_MAX_SQL_LEN)
    )
}

/// The result of successfully executing one statement: just enough information
/// to report a status line and, for `Rows`, render the fetched results. Format
/// neutral on purpose, so both the script runner and the REPL can report it in
/// their own output format without either restating how a statement is run.
pub(crate) enum StatementOutcome {
    Rows(Vec<RecordBatch>),
    RowsAffected(i64),
    Ok,
}

/// Build the status-line suffix describing a successful statement's outcome.
fn outcome_status_line(outcome: &StatementOutcome) -> String {
    match outcome {
        StatementOutcome::Rows(batches) => format!(" {} rows", total_rows(batches)),
        StatementOutcome::RowsAffected(n) => format!(" {} rows affected", n),
        StatementOutcome::Ok => " OK".to_string(),
    }
}

/// Execute one statement against `conn`, dispatching by its [`StatementType`].
/// `Query` results and streaming `Execute` results are fully fetched here so
/// the caller can report a row count and render them; a non-streaming
/// `Execute` (most `EXECUTE SCRIPT` calls) reports plain `OK`.
///
/// This is the single owner of the per-statement-type execution rule — in
/// particular of the non-obvious one, that an `Execute` result is fetched only
/// when it streams. Both `exapump sql` and the REPL run statements through it
/// so the two can never drift apart on how a statement kind is executed.
pub(crate) async fn execute_one(
    conn: &mut exarrow_rs::Connection,
    stmt: &str,
) -> Result<StatementOutcome, exarrow_rs::QueryError> {
    match StatementType::from_sql(stmt) {
        StatementType::Query => {
            let batches = conn.execute(stmt).await?.fetch_all().await?;
            Ok(StatementOutcome::Rows(batches))
        }
        StatementType::Dml => {
            let row_count = conn.execute_update(stmt).await?;
            Ok(StatementOutcome::RowsAffected(row_count))
        }
        StatementType::Ddl => {
            conn.execute_update(stmt).await?;
            Ok(StatementOutcome::Ok)
        }
        StatementType::Execute => {
            let result_set = conn.execute(stmt).await?;
            if result_set.is_stream() {
                Ok(StatementOutcome::Rows(result_set.fetch_all().await?))
            } else {
                Ok(StatementOutcome::Ok)
            }
        }
    }
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
    let mut renderer = ResultRenderer::new();
    let mut exec_error: Option<anyhow::Error> = None;

    for (i, stmt) in statements.iter().enumerate() {
        let stmt_num = i + 1;
        eprint!("{}", status_line_prefix(stmt_num, total, stmt));

        match execute_one(&mut conn, stmt.as_str()).await {
            Ok(outcome) => {
                executed += 1;
                eprintln!("{}", outcome_status_line(&outcome));
                if let StatementOutcome::Rows(batches) = &outcome {
                    renderer.render(batches, &args.format, &mut std::io::stdout())?;
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

    // --- split_statements ScriptBody tests ---

    #[test]
    fn split_script_body_keeps_internal_semicolons() {
        let input = "CREATE OR REPLACE LUA SCRIPT TEST.HELLO() RETURNS TABLE AS\n  local x = 1;\n  return query [[ SELECT 1 ]]\n/\n";
        let result = split_statements(input);
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("local x = 1;"));
    }

    #[test]
    fn split_script_body_excludes_lone_slash() {
        let input = "CREATE OR REPLACE LUA SCRIPT TEST.HELLO() RETURNS TABLE AS\n  local x = 1;\n  return query [[ SELECT 1 ]]\n/\n";
        let result = split_statements(input);
        assert_eq!(result.len(), 1);
        assert!(!result[0].contains("/\n"));
        assert!(!result[0].contains("\n/"));
        assert!(result[0].ends_with("  return query [[ SELECT 1 ]]"));
    }

    #[test]
    fn split_script_block_mixed_with_statements() {
        let input = "SELECT 1;\nCREATE LUA SCRIPT S.HELLO() AS\n  return 0;\n/\nSELECT 2;";
        let result = split_statements(input);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "SELECT 1");
        assert_eq!(result[1], "CREATE LUA SCRIPT S.HELLO() AS\n  return 0;");
        assert_eq!(result[2], "SELECT 2");
    }

    #[test]
    fn split_python3_adapter_script_header() {
        let input =
            "CREATE OR REPLACE PYTHON3 ADAPTER SCRIPT schema.name() AS\n  x = 1;\n  y = 2;\n/\n";
        let result = split_statements(input);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn split_create_script_without_as_splits_normally() {
        let result = split_statements("CREATE SCRIPT foo; SELECT 1");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "CREATE SCRIPT foo");
        assert_eq!(result[1], "SELECT 1");
    }

    #[test]
    fn split_script_body_terminator_at_end_of_input() {
        let result = split_statements("CREATE LUA SCRIPT S.H() AS\n  return 0;\n/");
        assert_eq!(result, vec!["CREATE LUA SCRIPT S.H() AS\n  return 0;"]);
    }

    #[test]
    fn split_script_body_terminator_may_be_indented() {
        let result = split_statements("CREATE LUA SCRIPT S.H() AS\n  return 0;\n  /\nSELECT 1");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "CREATE LUA SCRIPT S.H() AS\n  return 0;");
        assert_eq!(result[1], "SELECT 1");
    }

    #[test]
    fn split_script_body_keeps_slash_that_is_not_line_start() {
        let result = split_statements("CREATE LUA SCRIPT S.H() AS\n  return 1/2;\n/\n");
        assert_eq!(result, vec!["CREATE LUA SCRIPT S.H() AS\n  return 1/2;"]);
    }

    #[test]
    fn split_script_body_keeps_line_starting_slash_with_more_text() {
        let result = split_statements("CREATE LUA SCRIPT S.H() AS\n/tmp is a path\n/\n");
        assert_eq!(result, vec!["CREATE LUA SCRIPT S.H() AS\n/tmp is a path"]);
    }

    #[test]
    fn split_script_body_runs_to_end_of_input_without_terminator() {
        let result = split_statements("CREATE LUA SCRIPT S.H() AS\n  return 0;\n");
        assert_eq!(result, vec!["CREATE LUA SCRIPT S.H() AS\n  return 0;"]);
    }

    #[test]
    fn split_dash_at_end_of_input_is_not_a_comment() {
        let result = split_statements("SELECT 1 -");
        assert_eq!(result, vec!["SELECT 1 -"]);
    }

    #[test]
    fn split_slash_at_end_of_input_is_not_a_comment() {
        let result = split_statements("SELECT 4 /");
        assert_eq!(result, vec!["SELECT 4 /"]);
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

    #[test]
    fn strip_comments_empty_input_yields_empty_output() {
        assert_eq!(strip_comments(""), "");
    }

    #[test]
    fn strip_comments_preserves_double_quoted_identifier() {
        let result = strip_comments("SELECT \"a -- b /* c */ d\" FROM t");
        assert_eq!(result, "SELECT \"a -- b /* c */ d\" FROM t");
    }

    #[test]
    fn strip_comments_keeps_single_dash() {
        assert_eq!(strip_comments("SELECT 1 - 2"), "SELECT 1 - 2");
    }

    #[test]
    fn strip_comments_keeps_single_slash() {
        assert_eq!(strip_comments("SELECT 4 / 2"), "SELECT 4 / 2");
    }

    #[test]
    fn strip_comments_keeps_dash_at_end_of_input() {
        assert_eq!(strip_comments("SELECT 1 -"), "SELECT 1 -");
    }

    #[test]
    fn strip_comments_keeps_slash_at_end_of_input() {
        assert_eq!(strip_comments("SELECT 4 /"), "SELECT 4 /");
    }

    #[test]
    fn strip_comments_keeps_unterminated_single_quote() {
        assert_eq!(strip_comments("SELECT 'abc"), "SELECT 'abc");
    }

    #[test]
    fn strip_comments_keeps_unterminated_double_quote() {
        assert_eq!(strip_comments("SELECT \"abc"), "SELECT \"abc");
    }

    #[test]
    fn strip_comments_line_comment_runs_to_end_of_input() {
        assert_eq!(strip_comments("SELECT 1 -- tail"), "SELECT 1  ");
    }

    #[test]
    fn strip_comments_keeps_the_newline_that_ends_a_line_comment() {
        assert_eq!(
            strip_comments("SELECT 1 -- tail\nFROM t"),
            "SELECT 1  \nFROM t"
        );
    }

    #[test]
    fn strip_comments_collapses_each_comment_to_one_space() {
        assert_eq!(strip_comments("a/*x*/b/*y*/c"), "a b c");
    }

    #[test]
    fn strip_comments_treats_slash_star_slash_as_unterminated() {
        assert_eq!(strip_comments("SELECT 1 /*/"), "SELECT 1  ");
    }

    #[test]
    fn strip_comments_reenters_normal_state_after_a_string_literal() {
        assert_eq!(strip_comments("'a' -- b\n'c'"), "'a'  \n'c'");
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

    // --- resolve_sql_input tests ---

    fn test_args(sql: Option<&str>) -> SqlArgs {
        SqlArgs {
            sql: sql.map(str::to_string),
            conn: crate::connection::ConnectionArgs {
                dsn: None,
                profile: None,
                certificate_fingerprint: None,
                transport: crate::connection::Transport::default(),
            },
            format: OutputFormat::Csv,
        }
    }

    #[test]
    fn resolve_sql_input_returns_the_positional_argument_verbatim() {
        let args = test_args(Some("SELECT 1"));
        assert_eq!(resolve_sql_input(&args).unwrap(), "SELECT 1");
    }

    // --- write_csv / write_json tests ---

    fn sample_batch() -> RecordBatch {
        use arrow::array::{Int64Array, StringArray};
        use arrow::datatypes::{DataType, Field, Schema};
        use std::sync::Arc;

        let schema = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new("age", DataType::Int64, false),
        ]));
        let names = Arc::new(StringArray::from(vec!["alice", "bob"])) as _;
        let ages = Arc::new(Int64Array::from(vec![30, 40])) as _;
        RecordBatch::try_new(schema, vec![names, ages]).unwrap()
    }

    #[test]
    fn write_csv_writes_a_header_and_every_row_to_the_given_writer() {
        let batch = sample_batch();
        let mut buf: Vec<u8> = Vec::new();
        write_csv(&[batch], &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "name,age\nalice,30\nbob,40\n");
    }

    #[test]
    fn write_json_writes_a_json_array_of_rows_to_the_given_writer() {
        let batch = sample_batch();
        let mut buf: Vec<u8> = Vec::new();
        write_json(&[batch], &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(
            output,
            r#"[{"name":"alice","age":30},{"name":"bob","age":40}]"#
        );
    }

    #[test]
    fn write_json_writes_an_empty_array_for_zero_rows() {
        let mut buf: Vec<u8> = Vec::new();
        write_json(&[], &mut buf).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "[]");
    }

    // --- ResultRenderer tests ---

    #[test]
    fn render_result_writes_csv_without_a_leading_blank_line_for_the_first_result() {
        let batch = sample_batch();
        let mut renderer = ResultRenderer::new();
        let mut buf: Vec<u8> = Vec::new();
        renderer
            .render(&[batch], &OutputFormat::Csv, &mut buf)
            .unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.starts_with('\n'));
        assert!(renderer.rendered_any);
    }

    #[test]
    fn render_result_inserts_a_blank_line_before_a_subsequent_result() {
        let batch = sample_batch();
        let mut renderer = ResultRenderer { rendered_any: true };
        let mut buf: Vec<u8> = Vec::new();
        renderer
            .render(&[batch], &OutputFormat::Json, &mut buf)
            .unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with('\n'));
    }

    #[test]
    fn render_result_dispatches_json_format_to_write_json() {
        let batch = sample_batch();
        let mut renderer = ResultRenderer::new();
        let mut buf: Vec<u8> = Vec::new();
        renderer
            .render(&[batch], &OutputFormat::Json, &mut buf)
            .unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with('['));
    }

    // --- status_line_prefix tests ---

    #[test]
    fn status_line_prefix_formats_index_total_and_sql() {
        assert_eq!(status_line_prefix(2, 5, "SELECT 1"), "[2/5] SELECT 1");
    }

    #[test]
    fn status_line_prefix_truncates_long_sql() {
        let sql = "a".repeat(80);
        let prefix = status_line_prefix(1, 1, &sql);
        assert!(prefix.starts_with("[1/1] "));
        assert!(prefix.ends_with("..."));
    }

    // --- outcome_status_line tests ---

    #[test]
    fn outcome_status_line_for_rows_reports_the_row_count() {
        let outcome = StatementOutcome::Rows(vec![sample_batch()]);
        assert_eq!(outcome_status_line(&outcome), " 2 rows");
    }

    #[test]
    fn outcome_status_line_for_rows_affected_reports_the_count() {
        let outcome = StatementOutcome::RowsAffected(7);
        assert_eq!(outcome_status_line(&outcome), " 7 rows affected");
    }

    #[test]
    fn outcome_status_line_for_ok_reports_ok() {
        assert_eq!(outcome_status_line(&StatementOutcome::Ok), " OK");
    }

    // --- total_rows tests ---

    #[test]
    fn total_rows_sums_rows_across_batches() {
        assert_eq!(total_rows(&[sample_batch(), sample_batch()]), 4);
    }

    #[test]
    fn total_rows_of_no_batches_is_zero() {
        assert_eq!(total_rows(&[]), 0);
    }
}
