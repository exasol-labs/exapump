# Decisions: add-wait-command

## ADR: Replace `scripts/wait-for-exasol.sh` with a first-class `exapump wait` subcommand

**ID:** replace-wait-script-with-exapump-wait
**Plan:** add-wait-command
**Status:** Accepted

### Context

CI workflows for exapump and sibling repos started an Exasol Docker container and blocked via `scripts/wait-for-exasol.sh` before running integration tests. The script depended on bash, `/dev/tcp`, a separately-built `health_check` example binary, and direct `docker` invocations. It was fragile (shell-specific, required building an extra binary), duplicated across repos, and lived outside the spec-governed CLI surface.

### Decision

Replace `scripts/wait-for-exasol.sh` with a built-in `exapump wait` subcommand that reuses the shared `ConnectionArgs`/transport logic and gains full test coverage under the spec-governed CLI surface.

### Options Considered

| Option | Verdict |
|--------|---------|
| Replace with `exapump wait` subcommand | ✓ Chosen — reuses DSN/transport logic, gains test coverage, removes cross-repo shell dependency |
| Keep the bash script | ✗ Rejected — duplicates DSN logic, requires building a separate `health_check` binary, and is untested |
| Ship a thin wrapper script that calls exapump | ✗ Rejected — still requires shell and adds indirection without eliminating the duplication |

### Consequences

Readiness-polling is a first-class CLI capability governed by specs and covered by integration tests. The `health_check` example binary invocation is no longer needed in CI. Sibling repos can adopt `exapump wait` without maintaining their own polling scripts.

## ADR: Derive polled host/port from the resolved DSN rather than separate flags

**ID:** derive-wait-host-port-from-resolved-dsn
**Plan:** add-wait-command
**Status:** Accepted

### Context

The old `wait-for-exasol.sh` script used separate `EXASOL_HOST`/`EXASOL_PORT` environment variables to determine what to poll. The new `exapump wait` needed a connection-info strategy consistent with the rest of the CLI surface.

### Decision

Compute the TCP host and port to poll from the DSN resolved via `ConnectionArgs` (the same flattened struct all other subcommands use), rather than exposing independent `--host`/`--port` flags or separate environment variables.

### Options Considered

| Option | Verdict |
|--------|---------|
| Derive host/port from the resolved DSN via `ConnectionArgs` | ✓ Chosen — single source of truth; `wait` polls exactly what other subcommands connect to |
| Independent `--host`/`--port` flags | ✗ Rejected — duplicates connection configuration; users could inadvertently poll a different endpoint than the one being used |
| Separate env vars (`EXASOL_HOST`/`EXASOL_PORT`) mirroring the old script | ✗ Rejected — introduces a second connection-info mechanism inconsistent with `EXAPUMP_DSN` and profile precedence |

### Consequences

`exapump wait` inherits the full DSN resolution precedence (`--dsn` flag, `EXAPUMP_DSN` env var, profile) automatically. There is no risk of polling a different endpoint than other subcommands would connect to. A missing or unparseable DSN triggers a fast exit with code 1 (configuration error).
