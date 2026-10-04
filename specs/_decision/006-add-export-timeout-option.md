# Decisions: add-export-timeout-option

## ADR: Delegate timeout enforcement to exarrow-rs instead of wrapping the export

**ID:** delegate-timeout-enforcement-to-exarrow-rs
**Plan:** add-export-timeout-option
**Status:** Accepted

### Context

Only exarrow-rs can terminate the transport and abort the in-flight tunnel on timeout.

### Decision

exapump passes `--timeout` to exarrow-rs and arms no timer of its own.

### Options Considered

| Option | Verdict |
|--------|---------|
| Pass `--timeout` to exarrow-rs | ✓ Chosen — one owner for timeouts |
| Wrap the export in an exapump timer | ✗ Rejected — a weaker timer above a stronger one |

## ADR: Accept the loss of the implicit bound on Parquet and Arrow exports

**ID:** accept-loss-of-implicit-bound-on-parquet-and-arrow
**Plan:** add-export-timeout-option
**Status:** Accepted

### Context

Parquet exports have no timeout option. The earlier fixed 300-second bound was undocumented and never chosen by a user. A DSN parameter `?query_timeout=<seconds>` bounds every format.

### Decision

exapump applies no default timeout to Parquet exports. The documentation names `?query_timeout=` as their bound.

### Options Considered

| Option | Verdict |
|--------|---------|
| No default; document `?query_timeout=` | ✓ Chosen — the DSN bound covers every format |
| Add a timer for Parquet only | ✗ Rejected — formats would have opposite defaults |

## ADR: Delete partial output when the export deadline elapses

**ID:** delete-partial-output-on-export-timeout
**Plan:** add-export-timeout-option
**Status:** Accepted

### Context

A timed-out CSV export leaves a valid header and a truncated body. A downstream loader reads it as a complete export.

### Decision

On timeout, exapump deletes the output files it wrote in that run, names them on stderr, and returns the timeout error.

### Options Considered

| Option | Verdict |
|--------|---------|
| Delete the partial files | ✓ Chosen — a truncated file looks complete |
| Keep the file and warn on stderr | ✗ Rejected — a caller that checks only the exit code never sees the warning |
