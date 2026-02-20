# Feature: CSV Export

Export an Exasol table or SQL query result to a local CSV file, with configurable formatting options. The export uses exarrow-rs HTTP transport to stream data from Exasol directly to disk.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn` or `EXAPUMP_DSN`. The export command writes CSV data to the file specified by `--output`. The underlying `Connection::export_csv_to_file()` method handles HTTP transport setup, CSV formatting, and file writing. Column headers are included by default.

## Scenarios

### Scenario: Export table to CSV file

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --dsn <dsn>`
* *THEN* the command MUST write all rows from the table to `data.csv` in CSV format
* *AND* the first row of the output file MUST contain column headers
* *AND* stderr MUST print the number of rows exported
* *AND* the command MUST exit with code 0

### Scenario: Export query result to CSV file

* *GIVEN* a valid SQL query that returns rows
* *WHEN* the user runs `exapump export --query 'SELECT id, name FROM schema.table WHERE active = true' --output results.csv --format csv --dsn <dsn>`
* *THEN* the command MUST write all result rows to `results.csv` in CSV format
* *AND* the first row of the output file MUST contain column headers
* *AND* stderr MUST print the number of rows exported
* *AND* the command MUST exit with code 0

### Scenario: Table and query are mutually exclusive

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --query 'SELECT 1' --output out.csv --format csv --dsn <dsn>`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that `--table` and `--query` cannot be used together

### Scenario: Either table or query is required

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --output out.csv --format csv --dsn <dsn>`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that either `--table` or `--query` is required

### Scenario: Custom delimiter

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.tsv --format csv --dsn <dsn> --delimiter '\t'`
* *THEN* the command MUST write CSV data using tab as the field separator

### Scenario: Custom quote character

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --dsn <dsn> --quote "'"`
* *THEN* the command MUST use single quotes as the quoting character in the CSV output

### Scenario: No header row

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --dsn <dsn> --no-header`
* *THEN* the output file MUST NOT contain a header row
* *AND* the first row MUST be a data row

### Scenario: Custom null value

* *GIVEN* a table with NULL values exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --dsn <dsn> --null-value 'NA'`
* *THEN* NULL values in the output MUST be represented as `NA`

### Scenario: Output file not writable

* *GIVEN* the output path is in a directory that does not exist or is not writable
* *WHEN* the user runs `exapump export --table schema.table --output /nonexistent/dir/data.csv --format csv --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST contain the output file path

### Scenario: Table not found

* *GIVEN* the specified table does not exist in Exasol
* *WHEN* the user runs `exapump export --table schema.nonexistent --output data.csv --format csv --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that the table was not found

### Scenario: Query error

* *GIVEN* a SQL query with a syntax error
* *WHEN* the user runs `exapump export --query 'SELEC * FROM t' --output data.csv --format csv --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST display the error from Exasol

### Scenario: Export empty table

* *GIVEN* a table exists but contains no rows
* *WHEN* the user runs `exapump export --table schema.empty_table --output data.csv --format csv --dsn <dsn>`
* *THEN* the command MUST write a CSV file containing only the header row
* *AND* stderr MUST print that 0 rows were exported
* *AND* the command MUST exit with code 0

### Scenario: Table name with schema parsing

* *GIVEN* a table exists as `my_schema.my_table`
* *WHEN* the user runs `exapump export --table my_schema.my_table --output data.csv --format csv --dsn <dsn>`
* *THEN* the command MUST export from the correct schema and table
