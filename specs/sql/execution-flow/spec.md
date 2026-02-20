# Feature: SQL Execution Flow

The `sql` command splits input into individual statements, classifies them by type (query, DDL, DML), and executes them sequentially. Per-statement progress is reported on stderr. Execution stops on the first error.

## Background

SQL input may contain multiple semicolon-separated statements. Statements are split respecting quoted strings (single quotes for values, double quotes for identifiers). Empty statements (bare semicolons, trailing semicolons) are silently skipped.

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
