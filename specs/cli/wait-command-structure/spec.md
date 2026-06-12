# Feature: Wait Command Structure

The `wait` subcommand blocks until an Exasol database is ready to accept queries, then exits. It replaces the ad-hoc `scripts/wait-for-exasol.sh` shell script that every CI uses to poll a freshly started Exasol Docker container, making readiness-polling a first-class CLI capability.

## Background

The `wait` subcommand is available as `exapump wait`. Connection arguments (`--dsn`, `--profile` / `EXAPUMP_DSN`) are provided via the shared `ConnectionArgs` flattened into the wait command's arguments. Readiness is determined in two ordered phases: Phase 1 confirms the TCP port derived from the resolved DSN is open; Phase 2 confirms a `SELECT 1` query succeeds over the configured transport. The poll interval is an internal constant of 5 seconds and is not user-configurable. The overall deadline defaults to 1500 seconds and is overridable via `--timeout-secs`. An optional `--container` flag names a Docker container whose running state is verified before and during polling, and whose logs are dumped on failure; when omitted, no Docker interaction occurs.

The `wait` subcommand uses a fixed, stable exit-code mapping so CI scripts can branch on the failure mode:

| Exit code | Meaning | Triggers |
|-----------|---------|----------|
| 0 | Ready | Phase 1 and Phase 2 both succeeded within the deadline |
| 1 | Configuration error | No connection info available; DSN parse failure; any usage/argument error |
| 2 | Timeout | Polling loop exhausted `--timeout-secs` without reaching readiness |
| 3 | Container failure | Named `--container` is absent at startup, or stops running during polling |

These small distinct codes (rather than `sysexits.h` values such as 64/69/75) follow common Rust CLI convention, where 0 is success, 1 is the catch-all error, and additional low integers encode specific recoverable conditions that callers branch on. The codes are part of the command's contract and MUST NOT be reassigned.

## Scenarios

### Scenario: Wait help shows all arguments

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump wait --help`
* *THEN* the output MUST show the `--dsn` and `--profile` connection options
* *AND* the output MUST show a `--container` option
* *AND* the output MUST show a `--timeout-secs` option with `1500` as its default

### Scenario: Database already ready exits zero

* *GIVEN* an Exasol database is reachable and accepting queries
* *WHEN* the user runs `exapump wait --dsn <dsn>`
* *THEN* Phase 1 MUST observe the TCP port open
* *AND* Phase 2 MUST observe `SELECT 1` succeed
* *AND* the CLI MUST exit with code 0

### Scenario: Database becomes ready before timeout

* *GIVEN* an Exasol database whose TCP port is initially closed
* *AND* the port opens and `SELECT 1` begins to succeed before the timeout elapses
* *WHEN* the user runs `exapump wait --dsn <dsn> --timeout-secs <n>`
* *THEN* the CLI MUST continue polling at the internal 5-second interval until readiness
* *AND* the CLI MUST exit with code 0 once Phase 2 succeeds

### Scenario: Timeout while SQL never becomes ready

* *GIVEN* an Exasol TCP port is open but `SELECT 1` never succeeds
* *WHEN* the user runs `exapump wait --dsn <dsn> --timeout-secs 1`
* *THEN* the CLI MUST stop polling once the elapsed time reaches the timeout
* *AND* the CLI MUST exit with code 2
* *AND* the stderr MUST indicate that the wait timed out

### Scenario: Named container is not running

* *GIVEN* exapump is installed
* *AND* no Docker container with the given name appears in `docker ps`
* *WHEN* the user runs `exapump wait --dsn <dsn> --container <name>`
* *THEN* the CLI MUST detect the absent container before entering the polling loop
* *AND* the CLI MUST exit with code 3 immediately
* *AND* the stderr MUST indicate the named container is not running

### Scenario: Named container crashes during wait

* *GIVEN* a Docker container with the given name is running when the wait starts
* *AND* the database is not yet ready
* *WHEN* the container stops while `exapump wait --dsn <dsn> --container <name>` is polling
* *THEN* the CLI MUST detect that the container is no longer running
* *AND* the CLI MUST print the container's recent Docker logs to stderr
* *AND* the CLI MUST exit with code 3

### Scenario: No container flag skips Docker checks

* *GIVEN* exapump is installed
* *AND* the Docker CLI is unavailable or the database is not running in Docker
* *WHEN* the user runs `exapump wait --dsn <dsn>` without `--container`
* *THEN* the CLI MUST NOT invoke `docker ps` or `docker logs`
* *AND* the CLI MUST poll only the TCP port and `SELECT 1`

### Scenario: Connection info resolved from profile or environment

* *GIVEN* the `EXAPUMP_DSN` environment variable is set or a usable profile exists
* *WHEN* the user runs `exapump wait` without `--dsn`
* *THEN* the CLI MUST resolve connection info using the same precedence as other subcommands
* *AND* the CLI MUST derive the polled host and port from the resolved DSN

### Scenario: Missing connection info fails fast

* *GIVEN* the `EXAPUMP_DSN` environment variable is not set
* *AND* no usable profile exists
* *WHEN* the user runs `exapump wait` without `--dsn` or `--profile`
* *THEN* the CLI MUST exit with code 1
* *AND* the stderr MUST indicate that no connection info is available

### Scenario: Progress output reports phases and elapsed time

* *GIVEN* an Exasol database that is not immediately ready
* *WHEN* the user runs `exapump wait --dsn <dsn>`
* *THEN* the output MUST print a header identifying the current phase
* *AND* the output SHOULD emit a diagnostic line approximately every 30 seconds while still waiting
* *AND* each diagnostic line MUST include the elapsed seconds
* *AND* the output MUST report total elapsed time when readiness is reached
