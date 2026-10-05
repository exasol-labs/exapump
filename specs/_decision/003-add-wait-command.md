# Decisions: add-wait-command

## ADR: Replace `scripts/wait-for-exasol.sh` with a first-class `exapump wait` subcommand

**ID:** replace-wait-script-with-exapump-wait
**Plan:** add-wait-command
**Status:** Accepted

### Context

CI workflows in several repositories block on a shell script until Exasol is ready. The script duplicates DSN logic and needs a separate helper binary.

### Decision

`exapump wait` provides readiness polling and reuses the shared connection arguments.

### Options Considered

| Option | Verdict |
|--------|---------|
| Provide `exapump wait` | ✓ Chosen — reuses DSN logic and is covered by specs and tests |
| Keep the shell script | ✗ Rejected — duplicates DSN logic and is untested |
| Wrap exapump in a thin script | ✗ Rejected — keeps the shell dependency |

## ADR: Derive polled host/port from the resolved DSN rather than separate flags

**ID:** derive-wait-host-port-from-resolved-dsn
**Plan:** add-wait-command
**Status:** Accepted

### Context

`exapump wait` needs a host and port to poll.

### Decision

`exapump wait` polls the host and port of the DSN resolved by the shared connection arguments. It has no host or port flags of its own.

### Options Considered

| Option | Verdict |
|--------|---------|
| Use the resolved DSN | ✓ Chosen — `wait` polls the endpoint other commands use |
| Add `--host` and `--port` flags | ✗ Rejected — can poll a different endpoint than the one in use |
| Add separate host and port environment variables | ✗ Rejected — a second connection mechanism beside `EXAPUMP_DSN` and profiles |
