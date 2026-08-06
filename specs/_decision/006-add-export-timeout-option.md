# Decisions: add-export-timeout-option

## ADR: Delegate timeout enforcement to exarrow-rs instead of wrapping the export

**ID:** delegate-timeout-enforcement-to-exarrow-rs
**Plan:** add-export-timeout-option
**Status:** Accepted

### Context

An exapump-side timer cannot terminate the transport or abort the in-flight HTTP tunnel task. exarrow-rs 0.16.0 does both, and reports through `ExportError::Timeout` whether the transport survived. Duplicating a weaker timer above a stronger one would leave two owners of one decision. The user rejected wrapping `export_csv` in `tokio::time::timeout`, which would have bounded Parquet and Arrow exports too, during the planning interview.

### Decision

exapump parses `--timeout`, converts seconds to milliseconds, and passes the value to `CsvExportOptions::timeout_ms`. exapump arms no timer of its own.

### Options Considered

| Option | Verdict |
|--------|---------|
| Pass `--timeout` straight through to `CsvExportOptions::timeout_ms` | ✓ Chosen — exarrow-rs owns transport termination and tunnel abort, which an exapump-side timer cannot perform |
| Wrap `export_csv` in `tokio::time::timeout`, covering Parquet and Arrow too | ✗ Rejected by the user — a weaker timer duplicated above a stronger one |

### Consequences

Timeout enforcement has one owner: exarrow-rs. exapump's contribution is unit conversion and argument parsing, nothing more. Because exarrow-rs's `CsvExportOptions` accepts no equivalent field for Parquet or Arrow, `--timeout` cannot extend to those formats without an upstream change.

## ADR: Accept the loss of the implicit bound on Parquet and Arrow exports

**ID:** accept-loss-of-implicit-bound-on-parquet-and-arrow
**Plan:** add-export-timeout-option
**Status:** Accepted

### Context

The exarrow-rs 0.16.0 bump changes `timeout_ms`'s default from a fixed 300,000ms to `None`, removing the bound not only for CSV but also, as a side effect, for Parquet and Arrow exports that build their internal CSV options from the same default. That 300-second bound was never documented and never chosen by a user; a Parquet export of a large table hits it for the same wrong reason a CSV export does. `?query_timeout=<seconds>` in the DSN gives a server-enforced bound that covers every format.

### Decision

Ship the 0.16.0 bump without restoring a 300-second default for Parquet and Arrow exports, and record the change in the CHANGELOG and `docs/file_exchange.md`.

### Options Considered

| Option | Verdict |
|--------|---------|
| Ship the bump; document `?query_timeout=` as the replacement bound for Parquet and Arrow | ✓ Chosen — the removed default was undocumented and unchosen; a DSN-level bound already covers every format |
| Hold the bump until exarrow-rs exposes a Parquet timeout | ✗ Rejected — blocks the CSV fix on an unrelated upstream feature |
| Add an exapump-side timer for the Parquet path only | ✗ Rejected — leaves two formats with opposite defaults for no stated reason |

### Consequences

A Parquet or Arrow export against a hung server now waits indefinitely unless the DSN carries `?query_timeout=<seconds>`. `docs/file_exchange.md` and the CHANGELOG name that DSN parameter as the only remaining bound for those two formats.

## ADR: Delete partial output when the export deadline elapses

**ID:** delete-partial-output-on-export-timeout
**Plan:** add-export-timeout-option
**Status:** Accepted

### Context

`File::create` truncates the output target before any transport work runs, so a partial file holds no recoverable data — whatever the path held is already gone by the time the deadline fires. What remains after a timeout is a CSV with a valid header and a truncated body, which a downstream loader reads as a complete export with no signal that it is not. A warning on stderr does not reach a cron job that checks only the exit code and globs the output directory.

### Decision

When `--timeout` elapses, exapump removes the output files that export wrote and names them on stderr, then returns the timeout error. It removes only paths it opened during that run.

### Options Considered

| Option | Verdict |
|--------|---------|
| Delete the partial output files and name them on stderr | ✓ Chosen — a truncated file with a valid header is indistinguishable from a complete export to a downstream loader |
| Leave the partial file in place and warn on stderr | ✗ Rejected — a warning does not reach a caller that checks only the exit code |

### Consequences

A timed-out CSV export leaves no output file where earlier versions left a silently truncated one. The cleanup tracks only files the current run opened, so a pre-existing file at the same path that the run never touched survives untouched.
