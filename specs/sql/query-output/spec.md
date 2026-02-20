# Feature: SQL Query Output

SELECT results are written to stdout in CSV (default) or JSON format. This keeps stdout clean for piping while progress and errors go to stderr.

## Background

The `sql` command routes SELECT result data to stdout. Non-query statements produce no stdout output. When multiple SELECTs appear in a script, their result sets are concatenated with blank line separators.

## Scenarios

### Scenario: Single SELECT outputs CSV

* *GIVEN* a valid connection to Exasol
* *WHEN* the user runs `exapump sql 'SELECT col1, col2 FROM t' --format csv`
* *THEN* stderr MUST show a status line like `[1/1] SELECT col1, col2 FROM t ... N rows`
* *AND* stdout MUST contain a CSV header row with column names
* *AND* subsequent lines of stdout MUST contain the row data in CSV format
* *AND* stderr MUST show a final summary like `1 statement executed, 0 failed`
* *AND* the exit code MUST be 0

### Scenario: Single SELECT outputs JSON

* *GIVEN* a valid connection to Exasol
* *WHEN* the user runs `exapump sql 'SELECT col1, col2 FROM t' --format json`
* *THEN* stdout MUST contain a JSON array
* *AND* each element MUST be an object with column names as keys
* *AND* the exit code MUST be 0

### Scenario: Multiple SELECTs concatenate result sets

* *GIVEN* a valid connection to Exasol
* *AND* the SQL input contains two SELECT statements separated by semicolons
* *WHEN* the command executes
* *THEN* stdout MUST contain both result sets
* *AND* result sets MUST be separated by a blank line
* *AND* each result set MUST have its own header row (for CSV format)

### Scenario: Empty result set outputs header only for CSV

* *GIVEN* a valid connection to Exasol
* *AND* a table exists with columns but no rows
* *WHEN* the user runs `exapump sql 'SELECT * FROM empty_table' --format csv`
* *THEN* stdout MUST contain the CSV header row with column names
* *AND* there MUST be no data rows after the header
* *AND* stderr MUST show a status line like `[1/1] SELECT * FROM empty_table ... 0 rows`
* *AND* the exit code MUST be 0

### Scenario: Empty result set outputs empty array for JSON

* *GIVEN* a valid connection to Exasol
* *AND* a table exists with columns but no rows
* *WHEN* the user runs `exapump sql 'SELECT * FROM empty_table' --format json`
* *THEN* stdout MUST contain `[]`
* *AND* the exit code MUST be 0

### Scenario: Summary line with row count for single SELECT

* *GIVEN* a single SELECT statement returning 42 rows
* *WHEN* the command finishes execution
* *THEN* stderr MUST show `1 statement executed, 0 failed`
