# Feature: Export Command Structure

The `export` subcommand defines the argument interface for exporting data from Exasol to local files. It accepts a data source (`--table` or `--query`), output file path, format, connection string, CSV formatting options, a CSV export timeout, and Parquet-specific options for compression and file splitting.

## Background

The export command is the counterpart to upload. It writes data from Exasol to a local file. The data source is either a full table or a SQL query result. The output format is explicitly specified via `--format`. The `--compression` option is only valid with `--format parquet`. The `--timeout` option is only valid with `--format csv`, takes a whole number of seconds of at least 1, and is unset by default. The `--max-rows-per-file` and `--max-file-size` split options work with both CSV and Parquet formats.

## Scenarios

<!-- DELTA:CHANGED -->
### Scenario: Export help shows all arguments

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --help`
* *THEN* the output MUST show the `--table`, `--query`, `--output`, `--format`, `--dsn`, `--profile`, `--delimiter`, `--quote`, `--no-header`, `--null-value`, `--timeout`, `--compression`, `--max-rows-per-file`, and `--max-file-size` options
<!-- /DELTA:CHANGED -->

<!-- DELTA:NEW -->
### Scenario: Timeout help states seconds and CSV-only scope

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --help`
* *THEN* the `--timeout` help text MUST state that the value is a number of seconds
* *AND* the `--timeout` help text MUST state that the option applies to CSV format only
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Timeout of zero rejected

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --timeout 0 --dsn <dsn>`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST name `--timeout` and report `0` as an invalid value
<!-- /DELTA:NEW -->
