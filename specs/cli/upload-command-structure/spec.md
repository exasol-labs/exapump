# Feature: Upload Command Structure

The `upload` subcommand defines the argument interface for loading files into Exasol. At the scaffold stage, it parses and validates arguments but does not yet perform actual data loading.

## Background

The upload command is the primary entrypoint for data ingestion. It accepts file paths, a target table, and a connection string. All arguments are parsed via clap derive macros.

## Scenarios

### Scenario: Upload help shows all arguments

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump upload --help`
* *THEN* the output MUST show a positional `FILES` argument
* *AND* the output MUST show a `--table` option
* *AND* the output MUST show a `--dsn` option
* *AND* the output MUST show a `--dry-run` flag

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
