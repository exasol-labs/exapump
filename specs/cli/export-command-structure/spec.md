# Feature: Export Command Structure

The `export` subcommand defines the argument interface for exporting data from Exasol to local files. It accepts a data source (`--table` or `--query`), output file path, format, connection string, and CSV formatting options. All arguments are parsed via clap derive macros.

## Background

The export command is the counterpart to upload. It writes data from Exasol to a local file. The data source is either a full table or a SQL query result. The output format is explicitly specified via `--format`.

## Scenarios

### Scenario: Export help shows all arguments

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --help`
* *THEN* the output MUST show a `--table` option
* *AND* the output MUST show a `--query` option
* *AND* the output MUST show an `--output` option
* *AND* the output MUST show a `--format` option
* *AND* the output MUST show a `--dsn` option
* *AND* the output MUST show a `--delimiter` option
* *AND* the output MUST show a `--quote` option
* *AND* the output MUST show a `--no-header` flag
* *AND* the output MUST show a `--null-value` option

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
