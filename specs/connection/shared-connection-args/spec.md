# Feature: Shared Connection Args

All subcommands that connect to Exasol share a common set of connection arguments. These are encapsulated in a `ConnectionArgs` struct that is flattened into each subcommand's args via clap. A `.env` file in the working directory is auto-loaded at startup to supply environment variables like `EXAPUMP_DSN`.

## Background

Connection arguments (`--dsn` and env file loading) are shared across the `upload`, `export`, and `sql` subcommands. The `.env` file is loaded before clap parses arguments, so that `env = "EXAPUMP_DSN"` picks up values from both the `.env` file and the shell environment.

## Scenarios

### Scenario: ConnectionArgs is flattened into upload

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump upload --help`
* *THEN* the output MUST show a `--dsn` option
* *AND* the `--dsn` option MUST accept `EXAPUMP_DSN` as an environment variable source

### Scenario: ConnectionArgs is flattened into sql

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump sql --help`
* *THEN* the output MUST show a `--dsn` option
* *AND* the `--dsn` option MUST accept `EXAPUMP_DSN` as an environment variable source

### Scenario: Env file auto-loaded from working directory

* *GIVEN* a `.env` file exists in the current working directory
* *AND* the file contains `EXAPUMP_DSN=exasol://user:pwd@host:8563`
* *WHEN* exapump starts
* *THEN* the `EXAPUMP_DSN` variable MUST be available for argument resolution
* *AND* the command MUST NOT fail due to a missing DSN

### Scenario: Env file missing is not an error

* *GIVEN* no `.env` file exists in the current working directory
* *WHEN* exapump starts
* *THEN* the startup MUST NOT fail
* *AND* argument resolution MUST proceed using only shell environment variables and flags

### Scenario: Shell environment overrides env file

* *GIVEN* a `.env` file contains `EXAPUMP_DSN=exasol://file:pwd@host:8563`
* *AND* the shell environment has `EXAPUMP_DSN=exasol://shell:pwd@host:8563`
* *WHEN* exapump resolves the DSN
* *THEN* the shell environment value MUST take precedence over the `.env` file value

### Scenario: CLI flag overrides env file and shell environment

* *GIVEN* a `.env` file contains `EXAPUMP_DSN=exasol://file:pwd@host:8563`
* *AND* the shell environment has `EXAPUMP_DSN=exasol://shell:pwd@host:8563`
* *AND* the user provides `--dsn exasol://flag:pwd@host:8563` on the command line
* *WHEN* the command parses arguments
* *THEN* the `--dsn` flag value MUST take precedence over both the env file and shell environment

### Scenario: Env file loads any variable

* *GIVEN* a `.env` file contains `EXAPUMP_DSN=exasol://user:pwd@host:8563` and `OTHER_VAR=value`
* *WHEN* exapump starts
* *THEN* all variables from the `.env` file MUST be loaded into the process environment
* *AND* the loading MUST NOT be restricted to `EXAPUMP_*` variables only

### Scenario: Connect helper creates connection from DSN

* *GIVEN* a resolved DSN string from `ConnectionArgs`
* *WHEN* the `connect()` helper is called
* *THEN* it MUST parse the DSN via `ConnectionParams::from_str()`
* *AND* it MUST return a connected `Connection` ready for use
* *AND* connection errors MUST propagate as `anyhow::Result` errors
