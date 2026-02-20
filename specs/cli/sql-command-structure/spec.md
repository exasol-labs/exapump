# Feature: SQL Command Structure

The `sql` subcommand executes one or more SQL statements against an Exasol database and outputs results to stdout. It accepts SQL as a positional argument or from stdin, and supports CSV or JSON output for SELECT results.

## Background

The `sql` subcommand is available as `exapump sql`. Connection arguments (`--dsn` / `EXAPUMP_DSN`) are provided via the shared `ConnectionArgs` flattened into `SqlArgs`.

## Scenarios

### Scenario: SQL help shows all arguments

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump sql --help`
* *THEN* the output MUST show a positional `<SQL>` argument
* *AND* the output MUST show a `--dsn` option
* *AND* the output MUST show a `--format` option
* *AND* the output MUST describe `csv` and `json` as format choices

### Scenario: SQL from positional argument

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump sql 'SELECT 1' --dsn <dsn>`
* *THEN* the command MUST execute the SQL from the positional argument

### Scenario: SQL from stdin via dash argument

* *GIVEN* exapump is installed
* *AND* stdin contains `SELECT 1`
* *WHEN* the user runs `echo 'SELECT 1' | exapump sql - --dsn <dsn>`
* *THEN* the command MUST read SQL from stdin
* *AND* the command MUST execute the SQL read from stdin

### Scenario: SQL from stdin via pipe without dash

* *GIVEN* exapump is installed
* *AND* stdin is a pipe (not a terminal)
* *AND* no positional SQL argument is provided
* *WHEN* the user runs `echo 'SELECT 1' | exapump sql --dsn <dsn>`
* *THEN* the command MUST read SQL from stdin

### Scenario: Missing SQL argument with terminal stdin

* *GIVEN* exapump is installed
* *AND* stdin is a terminal (not piped)
* *WHEN* the user runs `exapump sql --dsn <dsn>` with no positional argument
* *THEN* the CLI MUST exit with a non-zero code
* *AND* the stderr MUST indicate the SQL argument is required

### Scenario: Missing DSN

* *GIVEN* exapump is installed
* *AND* the `EXAPUMP_DSN` environment variable is not set
* *AND* no `.env` file is present
* *WHEN* the user runs `exapump sql 'SELECT 1'` without `--dsn`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* the stderr MUST indicate the DSN is required

### Scenario: DSN from environment variable

* *GIVEN* the `EXAPUMP_DSN` environment variable is set
* *WHEN* the user runs `exapump sql 'SELECT 1'` without `--dsn`
* *THEN* the CLI MUST accept the DSN from the environment variable

### Scenario: DSN flag overrides environment variable

* *GIVEN* the `EXAPUMP_DSN` environment variable is set
* *AND* the user provides `--dsn` on the command line
* *WHEN* the sql command parses arguments
* *THEN* the `--dsn` flag value MUST take precedence over the environment variable

### Scenario: Default output format is CSV

* *GIVEN* a valid DSN and SQL statement
* *WHEN* the user runs `exapump sql 'SELECT ...'` without `--format`
* *THEN* the command MUST default to CSV output format
