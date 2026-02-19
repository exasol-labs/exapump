# Feature: Upload Command Structure

The `upload` subcommand defines the argument interface for loading files into Exasol. It accepts file paths, a target table, a connection string, and format-specific options. All arguments are parsed via clap derive macros.

## Background

The upload command is the primary entrypoint for data ingestion. It accepts file paths, a target table, and a connection string. All arguments are parsed via clap derive macros.

## Scenarios

### Scenario: Upload help shows all arguments

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump upload --help`
* *THEN* the output MUST show a positional `<FILES>` argument
* *AND* the output MUST show a `--table` option
* *AND* the output MUST show a `--dsn` option
* *AND* the output MUST show a `--dry-run` flag
* *AND* the output MUST show a `--delimiter` option
* *AND* the output MUST show a `--no-header` flag
* *AND* the output MUST show a `--quote` option
* *AND* the output MUST show a `--escape` option
* *AND* the output MUST show a `--null-value` option

### Scenario: Missing required arguments

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump upload` with no arguments
* *THEN* the CLI MUST exit with a non-zero code
* *AND* the stderr MUST indicate which arguments are missing

### Scenario: DSN from environment variable

* *GIVEN* the `EXAPUMP_DSN` environment variable is set
* *WHEN* the user runs `exapump upload data.parquet --table my_table` without `--dsn`
* *THEN* the CLI MUST accept the DSN from the environment variable

### Scenario: DSN flag overrides environment variable

* *GIVEN* the `EXAPUMP_DSN` environment variable is set
* *AND* the user provides `--dsn` on the command line
* *WHEN* the upload command parses arguments
* *THEN* the `--dsn` flag value MUST take precedence over the environment variable

### Scenario: CSV flags ignored for Parquet files

* *GIVEN* a Parquet file exists at the specified path
* *WHEN* the user runs `exapump upload data.parquet --table schema.table --dsn <dsn> --delimiter ';'`
* *THEN* the command MUST ignore the `--delimiter` flag
* *AND* the command MUST proceed with Parquet import as normal

### Scenario: CSV flags shown with defaults in help

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump upload --help`
* *THEN* the `--delimiter` option MUST show a default value of `,`
* *AND* the `--quote` option MUST show a default value of `"`
* *AND* the `--null-value` option SHOULD show a default value of empty string
