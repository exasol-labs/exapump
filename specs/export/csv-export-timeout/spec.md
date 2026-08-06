# Feature: CSV Export Timeout

Bound a CSV export with an optional client-side deadline, armed on both the single-file and split paths of `export/csv-export`.

## Background

`--timeout <seconds>` bounds the export with a client-side deadline covering SQL execution and download; it is armed on both the single-file and split paths. No deadline is armed unless `--timeout` is given.

The two paths differ in what the deadline reaches. The single-file path writes as it downloads, so the deadline covers the whole run, and an elapsed deadline removes the partial output file the export had already written. The split path's file-writing phase starts only after the download completes, so it runs unbounded: an elapsed deadline can only fire before the first split file is opened, leaving nothing to remove.

A server-enforced bound comes from `?query_timeout=<seconds>` in the DSN instead. `--timeout` is valid only with `--format csv`.

## Scenarios

### Scenario: CSV export without a timeout runs unbounded

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --dsn <dsn>` without `--timeout`
* *THEN* the export MUST arm no client-side deadline

### Scenario: Timeout value is interpreted as whole seconds

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --timeout 60 --dsn <dsn>`
* *THEN* the client-side deadline MUST be 60 seconds

### Scenario: CSV export completes within its timeout

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --timeout 300 --dsn <dsn>`
* *THEN* the command MUST write all rows from the table to `data.csv` in CSV format
* *AND* stderr MUST print the number of rows exported
* *AND* the command MUST exit with code 0

### Scenario: CSV export exceeding its timeout fails

* *GIVEN* a query whose result takes longer than 1 second to export
* *WHEN* the user runs `exapump export --query '<slow query>' --output data.csv --format csv --timeout 1 --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST report that the export timed out after 1000ms
* *AND* the command MUST delete the partially written `data.csv` before exiting
* *AND* stderr MUST name the deleted file

### Scenario: Split CSV export exceeding its timeout fails

* *GIVEN* a query whose result takes longer than 1 second to export
* *AND* a file already exists at `data.csv`
* *WHEN* the user runs `exapump export --query '<slow query>' --output data.csv --format csv --max-rows-per-file 1000 --timeout 1 --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST report that the export timed out after 1000ms
* *AND* the command MUST NOT create any `data_NNN.csv` split file
* *AND* the pre-existing `data.csv` MUST survive untouched, neither removed nor rewritten

### Scenario: Timeout option rejected for Parquet format

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --timeout 60 --dsn <dsn>`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that `--timeout` is only supported for CSV format
