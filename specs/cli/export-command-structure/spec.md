# Feature: Export Command Structure

The `export` subcommand defines the argument interface for exporting data from Exasol to local files. It accepts a data source (`--table` or `--query`), output file path, format, connection string, CSV formatting options, a CSV export timeout, and Parquet-specific options for compression and file splitting.

## Background

The export command is the counterpart to upload. It writes data from Exasol to a local file. The data source is either a full table or a SQL query result. The output format is explicitly specified via `--format`. The `--compression` option is only valid with `--format parquet`. The `--timeout` option is only valid with `--format csv`, takes a whole number of seconds of at least 1, and is unset by default. The `--max-rows-per-file` and `--max-file-size` split options work with both CSV and Parquet formats.

## Scenarios

### Scenario: Export help shows all arguments

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --help`
* *THEN* the output MUST show the `--table`, `--query`, `--output`, `--format`, `--dsn`, `--profile`, `--delimiter`, `--quote`, `--no-header`, `--null-value`, `--timeout`, `--compression`, `--max-rows-per-file`, and `--max-file-size` options

### Scenario: Timeout help states seconds and CSV-only scope

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --help`
* *THEN* the `--timeout` help text MUST state that the value is a number of seconds
* *AND* the `--timeout` help text MUST state that the option applies to CSV format only

### Scenario: Timeout of zero rejected

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --timeout 0 --dsn <dsn>`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST name `--timeout` and report `0` as an invalid value

### Scenario: Format accepts csv and parquet

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --help`
* *THEN* the `--format` option MUST accept `csv` and `parquet` as values

### Scenario: Missing required arguments

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export` with no arguments
* *THEN* the CLI MUST exit with a non-zero code
* *AND* the stderr MUST indicate which arguments are missing

### Scenario: DSN from environment variable

* *GIVEN* the `EXAPUMP_DSN` environment variable is set (via shell or `.env` file)
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv` without `--dsn`
* *THEN* the CLI MUST accept the DSN from the environment variable

### Scenario: DSN flag overrides environment variable

* *GIVEN* the `EXAPUMP_DSN` environment variable is set (via shell or `.env` file)
* *AND* the user provides `--dsn` on the command line
* *WHEN* the export command parses arguments
* *THEN* the `--dsn` flag value MUST take precedence over the environment variable

### Scenario: Format is required

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --dsn <dsn>` without `--format`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that `--format` is required

### Scenario: Compression default is snappy

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --dsn <dsn>` without `--compression`
* *THEN* the export MUST use Snappy compression by default

### Scenario: Invalid compression value rejected

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --compression brotli --dsn <dsn>`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST list the valid compression values
