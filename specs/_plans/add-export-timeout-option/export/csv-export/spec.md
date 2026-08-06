# Feature: CSV Export

Export an Exasol table or SQL query result to a local CSV file, with configurable formatting options and an optional client-side deadline. The export uses exarrow-rs HTTP transport to stream data from Exasol directly to disk. Supports automatic file splitting based on row count or file size thresholds.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn` or `EXAPUMP_DSN`. The export command writes CSV data to the file specified by `--output`. The underlying `Connection::export_csv_to_file()` method handles HTTP transport setup, CSV formatting, and file writing. Column headers are included by default. When split options (`--max-rows-per-file` or `--max-file-size`) are provided, the export writes to multiple files using a splitting writer that preserves Exasol-side CSV formatting. Split files are named `<stem>_000.csv`, `<stem>_001.csv`, etc. Each split file includes the header row unless `--no-header` is set. If splitting is requested but only one file is produced, the output uses the original `--output` name without a suffix. `--timeout <seconds>` bounds the export with a client-side deadline covering SQL execution and data transfer; it applies to both the single-file and split paths. No deadline is armed unless `--timeout` is given. An elapsed deadline removes the partial output files the export had already written, on both paths. A server-enforced bound comes from `?query_timeout=<seconds>` in the DSN instead.

## Scenarios

<!-- DELTA:NEW -->
### Scenario: CSV export without a timeout runs unbounded

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --dsn <dsn>` without `--timeout`
* *THEN* the export MUST arm no client-side deadline
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Timeout value is interpreted as whole seconds

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --timeout 60 --dsn <dsn>`
* *THEN* the client-side deadline MUST be 60 seconds
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: CSV export completes within its timeout

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --timeout 300 --dsn <dsn>`
* *THEN* the command MUST write all rows from the table to `data.csv` in CSV format
* *AND* stderr MUST print the number of rows exported
* *AND* the command MUST exit with code 0
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: CSV export exceeding its timeout fails

* *GIVEN* a query whose result takes longer than 1 second to export
* *WHEN* the user runs `exapump export --query '<slow query>' --output data.csv --format csv --timeout 1 --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST report that the export timed out after 1000ms
* *AND* the command MUST delete the partially written `data.csv` before exiting
* *AND* stderr MUST name the deleted file
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Split CSV export exceeding its timeout fails

* *GIVEN* a query whose result takes longer than 1 second to export
* *WHEN* the user runs `exapump export --query '<slow query>' --output data.csv --format csv --max-rows-per-file 1000 --timeout 1 --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST report that the export timed out after 1000ms
* *AND* the command MUST delete every `data_NNN.csv` split file it created before exiting
* *AND* stderr MUST name every deleted file
* *AND* the command MUST NOT create or remove a file at `data.csv`
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Timeout option rejected for Parquet format

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --timeout 60 --dsn <dsn>`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that `--timeout` is only supported for CSV format
<!-- /DELTA:NEW -->
