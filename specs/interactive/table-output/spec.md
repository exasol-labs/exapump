# Feature: Table Output

SELECT query results in the interactive REPL are rendered as formatted tables using comfy-table, providing a readable presentation of tabular data in the terminal.

## Background

The table output format is the default for the interactive REPL. It renders Arrow RecordBatches as Unicode-bordered tables with column headers. The table dynamically adjusts column widths to fit the terminal.

## Scenarios

### Scenario: Basic table rendering

* *GIVEN* the REPL is running with table output format (default)
* *WHEN* the user executes a SELECT that returns rows
* *THEN* the output MUST use Unicode box-drawing characters for borders
* *AND* the output MUST include a header row with column names
* *AND* each data row MUST be separated by the table borders
* *AND* the output MUST show a row count line below the table (e.g. `3 rows`)

### Scenario: Table adapts to terminal width

* *GIVEN* the REPL is running with table output format
* *WHEN* the user executes a SELECT with many or wide columns
* *THEN* the table SHOULD dynamically arrange content to fit the terminal width
* *AND* long cell values SHOULD wrap rather than truncate

### Scenario: NULL values displayed distinctly

* *GIVEN* the REPL is running with table output format
* *WHEN* the user executes a SELECT that returns NULL values
* *THEN* NULL values MUST be displayed as `NULL` in the table

### Scenario: Single row result

* *GIVEN* the REPL is running with table output format
* *WHEN* the user executes `SELECT 1 AS num, 'hello' AS msg;`
* *THEN* the output MUST show a table with headers `num` and `msg`
* *AND* the output MUST show one data row with values `1` and `hello`
* *AND* the output MUST show `1 row` (singular)
