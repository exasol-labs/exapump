# Feature: CSV Export

Export an Exasol table or SQL query result to a local CSV file, with configurable formatting options. The export uses exarrow-rs HTTP transport to stream data from Exasol directly to disk. Supports automatic file splitting based on row count or file size thresholds.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn` or `EXAPUMP_DSN`. The export command writes CSV data to the file specified by `--output`. The underlying `Connection::export_csv_to_file()` method handles HTTP transport setup, CSV formatting, and file writing. Column headers are included by default. When split options (`--max-rows-per-file` or `--max-file-size`) are provided, the export writes to multiple files using a splitting writer that preserves Exasol-side CSV formatting. Split files are named `<stem>_000.csv`, `<stem>_001.csv`, etc. Each split file includes the header row unless `--no-header` is set. If splitting is requested but only one file is produced, the output uses the original `--output` name without a suffix.

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

### Scenario: Split CSV by max rows per file

* *GIVEN* a table with 10 rows exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-rows-per-file 3 --dsn <dsn>`
* *THEN* the command MUST produce 4 files: `data_000.csv`, `data_001.csv`, `data_002.csv`, `data_003.csv`
* *AND* each split file MUST include the header row
* *AND* stderr MUST print the total number of rows exported and the number of files written
* *AND* the command MUST exit with code 0

### Scenario: Split CSV by max file size

* *GIVEN* a table with enough data to exceed the size threshold
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-file-size 1MB --dsn <dsn>`
* *THEN* the command MUST split output into multiple files where each file's size SHOULD NOT exceed the threshold
* *AND* files MUST be named `data_000.csv`, `data_001.csv`, etc.
* *AND* each split file MUST include the header row
* *AND* stderr MUST print the total number of rows exported and the number of files written

### Scenario: Split CSV produces single file

* *GIVEN* a table with 5 rows exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-rows-per-file 1000 --dsn <dsn>`
* *THEN* the command MUST write the output to `data.csv` (the original `--output` name, no numeric suffix)
* *AND* the command MUST exit with code 0

### Scenario: Split CSV with no-header

* *GIVEN* a table with 6 rows exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-rows-per-file 3 --no-header --dsn <dsn>`
* *THEN* each split file MUST NOT contain a header row

### Scenario: Split CSV preserves formatting options

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-rows-per-file 3 --delimiter '\t' --dsn <dsn>`
* *THEN* each split file MUST use tab as the field separator

### Scenario: Both CSV split thresholds active

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-rows-per-file 100000 --max-file-size 50MB --dsn <dsn>`
* *THEN* the command MUST split into a new file whenever either threshold is reached first
