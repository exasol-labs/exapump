# Feature: Interactive Command Structure

The `interactive` subcommand starts an interactive SQL REPL session against an Exasol database. It provides a readline-style interface with persistent history and multi-line input support.

## Background

The `interactive` subcommand is available as `exapump interactive`. Connection arguments (`--dsn`, `--profile` / `EXAPUMP_DSN`) are provided via the shared `ConnectionArgs` flattened into the command's args.

## Scenarios

### Scenario: Interactive help shows all arguments

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump interactive --help`
* *THEN* the output MUST show a `--dsn` option
* *AND* the output MUST show a `--profile` option
* *AND* the output MUST describe the command as starting an interactive SQL session

### Scenario: Interactive connects to Exasol

* *GIVEN* exapump is installed
* *AND* a valid DSN is provided
* *WHEN* the user runs `exapump interactive --dsn <dsn>`
* *THEN* the REPL MUST establish a connection to Exasol
* *AND* the REPL MUST display a welcome message including the exapump version
* *AND* the REPL MUST display a `exapump> ` prompt

### Scenario: Missing DSN and no profile

* *GIVEN* exapump is installed
* *AND* the `EXAPUMP_DSN` environment variable is not set
* *AND* no `.env` file is present
* *AND* no config file profile exists
* *WHEN* the user runs `exapump interactive` without `--dsn` or `--profile`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that no connection info is available

### Scenario: DSN from environment variable

* *GIVEN* the `EXAPUMP_DSN` environment variable is set
* *WHEN* the user runs `exapump interactive` without `--dsn`
* *THEN* the CLI MUST accept the DSN from the environment variable
