# Feature: Parquet Import

Upload a single Parquet file into an Exasol table, with automatic table creation from Parquet metadata and dry-run schema preview.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn` or `EXAPUMP_DSN`. File format is detected from the file extension. The upload command is async.

## Scenarios

### Scenario: Upload Parquet file to existing table

* *GIVEN* a Parquet file exists at the specified path
* *AND* the target table already exists in Exasol with a compatible schema
* *WHEN* the user runs `exapump upload data.parquet --table schema.table --dsn <dsn>`
* *THEN* the command MUST import all rows from the Parquet file into the target table
* *AND* the command MUST print the number of rows imported
* *AND* the command MUST exit with code 0

### Scenario: Upload Parquet file with auto table creation

* *GIVEN* a Parquet file exists at the specified path
* *AND* the target table does not exist in Exasol
* *WHEN* the user runs `exapump upload data.parquet --table schema.new_table --dsn <dsn>`
* *THEN* the command MUST create the target table with schema inferred from Parquet metadata
* *AND* the command MUST import all rows and print the number of rows imported
* *AND* the command MUST exit with code 0

### Scenario: Dry-run shows inferred schema

* *GIVEN* a Parquet file exists at the specified path
* *WHEN* the user runs `exapump upload data.parquet --table schema.table --dsn <dsn> --dry-run`
* *THEN* the command MUST print the column names and Exasol types inferred from Parquet metadata
* *AND* the command MUST print the planned CREATE TABLE DDL statement
* *AND* the command MUST NOT connect to Exasol or modify any data
* *AND* the command MUST exit with code 0

### Scenario: File not found error

* *GIVEN* the specified file path does not exist
* *WHEN* the user runs `exapump upload missing.parquet --table schema.table --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST contain the file path that was not found

### Scenario: Unsupported file extension

* *GIVEN* a file exists with an unrecognized extension (not `.parquet`)
* *WHEN* the user runs `exapump upload data.csv --table schema.table --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that the file format is not supported
* *AND* stderr SHOULD list the supported formats

### Scenario: Connection failure

* *GIVEN* the DSN points to an unreachable or invalid Exasol host
* *AND* a valid Parquet file exists
* *WHEN* the user runs `exapump upload data.parquet --table schema.table --dsn exasol://bad:bad@nowhere:9999`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that the connection to Exasol failed
