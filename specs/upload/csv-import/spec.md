# Feature: CSV Import

Upload a single CSV file into an Exasol table, with automatic table creation from schema inference and dry-run schema preview. CSV format options (delimiter, quoting, header, null values) are configurable via CLI flags with sensible defaults.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn` or `EXAPUMP_DSN`. File format is detected from the file extension (`.csv`). Schema inference is performed by exarrow-rs via `infer_schema_from_csv()`, which samples rows to determine Exasol column types. The upload command is async.

Default CSV parsing behavior: comma delimiter, double-quote quoting, first row treated as header, empty strings treated as NULL.

Exasol applies one `ROW SEPARATOR` to an entire IMPORT, so the row separator is not a user option: exapump scans the file and names the separator the file actually uses. The scan is quote-aware, so a `\r`, an `\n` or a `\r\n` inside a quoted field is data, not a record boundary.

## Scenarios

### Scenario: Upload CSV file to existing table

* *GIVEN* a CSV file with a header row exists at the specified path
* *AND* the target table already exists in Exasol with a compatible schema
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn>`
* *THEN* the command MUST import all data rows from the CSV file into the target table
* *AND* the command MUST print the number of rows imported
* *AND* the command MUST exit with code 0

### Scenario: Upload CSV file with auto table creation

* *GIVEN* a CSV file with a header row exists at the specified path
* *AND* the target table does not exist in Exasol
* *WHEN* the user runs `exapump upload data.csv --table schema.new_table --dsn <dsn>`
* *THEN* the command MUST create the target table with schema inferred from CSV row sampling
* *AND* the command MUST import all data rows and print the number of rows imported
* *AND* the command MUST exit with code 0

### Scenario: Dry-run shows inferred schema from CSV

* *GIVEN* a CSV file with a header row exists at the specified path
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn> --dry-run`
* *THEN* the command MUST print the column names and Exasol types inferred from CSV row sampling
* *AND* the command MUST print the planned CREATE TABLE DDL statement
* *AND* the command MUST NOT connect to Exasol or modify any data
* *AND* the command MUST exit with code 0

### Scenario: CRLF line endings

* *GIVEN* a CSV file whose rows all end with `\r\n`
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn>`
* *THEN* the command MUST import the file under a CRLF row separator
* *AND* no imported value may carry a trailing carriage return

### Scenario: LF line endings

* *GIVEN* a CSV file whose rows all end with `\n`
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn>`
* *THEN* the command MUST import the file under an LF row separator

### Scenario: Line breaks inside a quoted field

* *GIVEN* a CSV file with a quoted field containing `\r`, `\n` or `\r\n`
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn>`
* *THEN* the command MUST NOT treat the quoted bytes as a record boundary
* *AND* the imported value MUST keep those bytes unchanged

### Scenario: Mixed line endings

* *GIVEN* a CSV file where some rows end with `\r\n` and others with `\n`
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code before creating a table or importing any row
* *AND* stderr MUST name the file, report how many rows use each style, and say how to convert the file

### Scenario: Custom delimiter

* *GIVEN* a tab-separated file exists at `data.csv`
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn> --delimiter '\t'`
* *THEN* the command MUST parse the file using tab as the field delimiter
* *AND* the command MUST import all data rows into the target table

### Scenario: No header row

* *GIVEN* a CSV file exists without a header row
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn> --no-header`
* *THEN* the command MUST generate column names as `col_1`, `col_2`, ... `col_N`
* *AND* the command MUST import all rows into the target table

### Scenario: Custom quote character

* *GIVEN* a CSV file exists that uses single quotes for quoting
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn> --quote "'"`
* *THEN* the command MUST parse quoted fields using the single-quote character

### Scenario: Custom escape character

* *GIVEN* a CSV file exists that uses backslash as an escape character
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn> --escape '\'`
* *THEN* the command MUST interpret backslash as the escape character within quoted fields

### Scenario: Custom null value

* *GIVEN* a CSV file exists where the string `NULL` represents missing values
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn> --null-value NULL`
* *THEN* the command MUST treat fields matching `NULL` as SQL NULL values during import

### Scenario: CSV file not found

* *GIVEN* the specified CSV file path does not exist
* *WHEN* the user runs `exapump upload missing.csv --table schema.table --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST contain the file path that was not found

### Scenario: Empty CSV file

* *GIVEN* a CSV file exists but contains only a header row and no data rows
* *WHEN* the user runs `exapump upload empty.csv --table schema.table --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that the file contains no data rows
