# Feature: SQL Execution Flow

The `sql` command splits input into individual statements, classifies them by type (query, DML, DDL, execute), and executes them sequentially. Per-statement progress is reported on stderr. Execution stops on the first error. The original statement text — including all comments and SQL hints — MUST reach Exasol byte-for-byte; comments are used only to determine the statement type, never to rewrite the SQL sent over the wire.

## Background

SQL input may contain multiple semicolon-separated statements. Statements are split respecting quoted strings (single quotes for values, double quotes for identifiers) AND comments (`-- ...` line comments and `/* ... */` block comments). Empty statements (bare semicolons, trailing semicolons) are silently skipped. Statement classification uses the first SQL keyword AFTER any leading comments are skipped, but the original text is preserved verbatim for execution.

## Scenarios

### Scenario: DDL statement outputs OK

* *GIVEN* a valid connection to Exasol
* *WHEN* the user runs `exapump sql 'CREATE TABLE t(id INT)'`
* *THEN* stderr MUST show a status line like `[1/1] CREATE TABLE t(id INT) ... OK`
* *AND* stderr MUST show a final summary like `1 statement executed, 0 failed`
* *AND* stdout MUST be empty
* *AND* the exit code MUST be 0

### Scenario: DML statement outputs row count

* *GIVEN* a valid connection to Exasol
* *WHEN* the user runs `exapump sql 'INSERT INTO t VALUES (1),(2)'`
* *THEN* stderr MUST show a status line like `[1/1] INSERT INTO t VALUES (1),(2) ... 2 rows affected`
* *AND* stdout MUST be empty
* *AND* the exit code MUST be 0

### Scenario: GRANT statement outputs OK

* *GIVEN* a valid connection to Exasol
* *WHEN* the user runs `exapump sql 'GRANT SELECT ON schema.t TO user1'`
* *THEN* stderr MUST show a status line ending with `OK`
* *AND* stdout MUST be empty
* *AND* the exit code MUST be 0

### Scenario: Multi-statement script executes sequentially

* *GIVEN* a valid connection to Exasol
* *AND* the SQL input is `CREATE TABLE t(id INT); INSERT INTO t VALUES (1),(2); SELECT * FROM t`
* *WHEN* the command executes
* *THEN* stderr MUST show three status lines numbered `[1/3]`, `[2/3]`, `[3/3]`
* *AND* each status line MUST show the statement text and its outcome
* *AND* the SELECT result data MUST appear on stdout
* *AND* stderr MUST show a final summary `3 statements executed, 0 failed`
* *AND* the exit code MUST be 0

### Scenario: Multi-statement stops on first error

* *GIVEN* a valid connection to Exasol
* *AND* the SQL input is `CREATE TABLE t(id INT); SELECT * FROM nonexistent; SELECT 1`
* *WHEN* the command executes
* *THEN* stderr MUST show `[1/3] CREATE TABLE t(id INT) ... OK`
* *AND* stderr MUST show the error for statement 2
* *AND* the third statement MUST NOT be executed
* *AND* stderr MUST show a final summary like `2 statements executed, 1 failed`
* *AND* the exit code MUST be non-zero

### Scenario: Statement splitting respects quoted strings

* *GIVEN* the SQL input is `SELECT 'hello; world' AS val`
* *WHEN* the command splits statements
* *THEN* it MUST treat the entire input as a single statement
* *AND* the semicolon inside the string literal MUST NOT split the statement

### Scenario: Trailing semicolons produce no empty statements

* *GIVEN* the SQL input is `SELECT 1; SELECT 2;`
* *WHEN* the command splits statements
* *THEN* it MUST produce exactly 2 statements
* *AND* the trailing semicolon MUST NOT produce an empty third statement

### Scenario: Status line truncates long SQL

* *GIVEN* a SQL statement longer than 60 characters
* *WHEN* the command prints the status line to stderr
* *THEN* the statement text in the status line SHOULD be truncated with `...`
* *AND* the full statement MUST still be sent to Exasol unmodified

### Scenario: Line comment is stripped from piped input

* *GIVEN* a valid connection to Exasol
* *AND* the SQL input piped on stdin is `-- this is a comment\nSELECT 1`
* *WHEN* the command executes
* *THEN* the statement MUST be classified as a query using the keyword after the comment
* *AND* the statement sent to Exasol MUST be the original input including the leading comment, verbatim
* *AND* stderr MUST show a status line like `[1/1] -- this is a comment ... 1 rows` and the exit code MUST be 0

### Scenario: Trailing line comment on a statement is stripped

* *GIVEN* a valid connection to Exasol
* *AND* the SQL input is `SELECT 1 -- trailing comment\n; SELECT 2`
* *WHEN* the command executes
* *THEN* the command MUST produce exactly 2 statements, both classified as queries
* *AND* the first statement sent to Exasol MUST be `SELECT 1 -- trailing comment` preserved verbatim including the trailing comment
* *AND* the second statement MUST be `SELECT 2`

### Scenario: Block comment spanning lines is stripped

* *GIVEN* a valid connection to Exasol
* *AND* the SQL input is `/* multi\nline\ncomment */ SELECT 1`
* *WHEN* the command executes
* *THEN* the statement MUST be classified as a query using the keyword after the block comment
* *AND* the statement sent to Exasol MUST be the original input including the block comment, verbatim
* *AND* the exit code MUST be 0

### Scenario: Semicolon inside a line comment does not split statements

* *GIVEN* the SQL input is `SELECT 1 -- a; b; c\n`
* *WHEN* the command splits statements
* *THEN* it MUST produce exactly 1 statement
* *AND* the line comment MUST be preserved verbatim in the returned statement text

### Scenario: Semicolon inside a block comment does not split statements

* *GIVEN* the SQL input is `SELECT /* a; b */ 1`
* *WHEN* the command splits statements
* *THEN* it MUST produce exactly 1 statement
* *AND* the block comment MUST be preserved verbatim in the returned statement text

### Scenario: SELECT prefixed with a block-comment hint executes as a query and preserves the hint

* *GIVEN* a valid connection to Exasol
* *WHEN* the user runs `exapump sql '/*snapshot execution*/ SELECT 1'`
* *THEN* the statement MUST be classified as a query
* *AND* the statement sent to Exasol MUST include the `/*snapshot execution*/` text verbatim
* *AND* stdout MUST contain the result row and stderr MUST show a status line ending with `1 rows`
* *AND* the exit code MUST be 0

### Scenario: SELECT prefixed with a line-comment hint executes as a query and preserves the hint

* *GIVEN* a valid connection to Exasol
* *AND* the SQL input is `-- hint: snapshot execution\nSELECT 1`
* *WHEN* the command executes
* *THEN* the statement MUST be classified as a query
* *AND* the statement sent to Exasol MUST include the leading line comment verbatim
* *AND* the exit code MUST be 0

### Scenario: EXECUTE SCRIPT with RETURNS TABLE displays a result set

* *GIVEN* a valid connection to Exasol and a script `S.HELLO` that exists with `RETURNS TABLE` returning at least one row
* *WHEN* the user runs `exapump sql 'EXECUTE SCRIPT "S"."HELLO"()'`
* *THEN* the statement MUST be classified as `Execute` and the command MUST execute the script via `conn.execute` (not `execute_update`)
* *AND* stdout MUST contain the result rows in the chosen output format
* *AND* stderr MUST show a status line ending with `N rows` where N is the result row count
* *AND* the exit code MUST be 0

### Scenario: EXECUTE SCRIPT without a result set displays OK

* *GIVEN* a valid connection to Exasol and a script `S.NOOP` that exists with no `RETURNS TABLE` clause
* *WHEN* the user runs `exapump sql 'EXECUTE SCRIPT "S"."NOOP"()'`
* *THEN* the statement MUST be classified as `Execute`
* *AND* stderr MUST show a status line ending with ` OK`
* *AND* stdout MUST be empty and the exit code MUST be 0

### Scenario: Comments are preserved in the SQL sent to Exasol

* *GIVEN* a valid connection to Exasol
* *AND* the SQL input contains line comments, block comments, and SQL hints
* *WHEN* the command executes a statement
* *THEN* the text sent to Exasol MUST be the original statement text byte-for-byte (after splitting on top-level semicolons), including all comments and hints
* *AND* comments MUST be used only to determine the statement type, not to rewrite the SQL

### Scenario: Statement splitting ignores semicolons inside line comments without rewriting input

* *GIVEN* the SQL input is `SELECT 1 -- a; b; c\nUNION ALL SELECT 2`
* *WHEN* the command splits statements
* *THEN* it MUST produce exactly 1 statement
* *AND* the returned statement text MUST include the line comment verbatim

### Scenario: Statement splitting ignores semicolons inside block comments without rewriting input

* *GIVEN* the SQL input is `SELECT /* a; b */ 1; SELECT 2`
* *WHEN* the command splits statements
* *THEN* it MUST produce exactly 2 statements
* *AND* the first returned statement MUST include the block comment verbatim

### Scenario: Comment-like sequence inside a string literal is preserved

* *GIVEN* the SQL input is `SELECT '-- not a comment' AS val`
* *WHEN* the command executes
* *THEN* the statement sent to Exasol MUST retain the `-- not a comment` text inside the string literal
* *AND* the command MUST produce exactly 1 statement

### Scenario: Input containing only comments yields no statements

* *GIVEN* the SQL input is `-- just a comment\n/* another */\n`
* *WHEN* the command executes
* *THEN* the command MUST fail with an error message `No SQL statements to execute`
* *AND* no connection attempt MUST be made
* *AND* the exit code MUST be non-zero
