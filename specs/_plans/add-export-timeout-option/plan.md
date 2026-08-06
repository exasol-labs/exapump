# Plan: add-export-timeout-option

## Summary

Add `--timeout <seconds>` to `exapump export` for CSV exports and bump exarrow-rs to 0.16.0, whose new `None` default removes the fixed 300-second client-side bound that failed long exports (issue #38). Without the flag an export now runs until Exasol finishes the EXPORT statement.

## Design

### Context

`exapump export` inherits a client-side timer from exarrow-rs. Until 0.16.0, `CsvExportOptions::timeout_ms` was a plain `u64` defaulting to 300,000, so any CSV export running past 300 seconds died with `Error: Export timed out after 300000ms`. exapump never set the field and exposed no flag, leaving the user no way to raise or remove the bound.

exarrow-rs 0.16.0 changes `timeout_ms` to `Option<u64>` defaulting to `None`, and arms the timer only when the value is `Some`. The bug therefore disappears on the version bump alone; the flag restores deliberate control over the bound.

One upstream constraint shapes the whole design: only the CSV export path accepts a timeout. `ParquetExportOptions` and `ArrowExportOptions` expose none, which exarrow-rs states in the doc comment on `shared_csv_export_options` (`src/export/csv.rs:257-261`). Parquet exports in exapump run through those two option types, so no timeout can be honored there.

- **Goals** — give the user an explicit, documented client-side bound on CSV exports; remove the undocumented 300-second bound that no user asked for; report a mis-paired `--timeout`/`--format parquet` combination instead of silently ignoring the flag.
- **Non-Goals** — a timeout for Parquet or Arrow exports; an exapump-side `tokio::time::timeout` wrapper around the whole export; a `--query-timeout` flag for the server-enforced bound; humanized durations such as `15m`.

### Decision

Pass the flag straight through to the upstream builder. exapump adds argument parsing, one unit conversion, and one validation rule; exarrow-rs owns the timer.

#### Architecture

```
exapump export --timeout <seconds>
        │
        ▼
  ExportArgs.timeout: Option<u64>        (src/cli.rs, clap range 1..)
        │
        ├─ reject_format_mismatched_options()   parquet + --timeout  → bail
        │                                       csv     + --compression → bail
        ▼
  build_csv_options()                    seconds × 1000 → .timeout_ms(ms)
        │
        ▼
  CsvExportOptions { timeout_ms: Option<u64> }
        │
        ├─ conn.export_csv_to_file()     single-file path
        └─ conn.export_csv_to_stream()   split path (SplitCsvWriter)
```

Both CSV entry points funnel into exarrow-rs `export_to_callback`, which holds the timer, so the split path inherits the bound with no extra wiring.

#### Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| Pass-through option builder | `build_csv_options` | exarrow-rs owns timeout enforcement; exapump converts units and nothing more |
| Single format-compatibility guard | `reject_format_mismatched_options` | One function owns the rule "which export options belong to which format", replacing `reject_compression_for_csv` |
| Parse-time range constraint | `clap::value_parser!(u64).range(1..=18_446_744_073_709_551)` | Rejects `--timeout 0` with clap's standard error before any connection opens, and caps the value at `u64::MAX / 1000` so the millisecond conversion cannot overflow |

### Consequences

| Decision | Alternatives Considered | Rationale |
|----------|------------------------|-----------|
| `--timeout` applies to CSV only; `--format parquet` is a hard error | Silently ignore the flag for Parquet; wrap the whole export in an exapump-side `tokio::time::timeout` covering both formats | Upstream exposes no Parquet timeout. Silence would leave a user believing a bound exists. The user rejected the exapump-side wrapper. |
| Default is no client-side timeout | Keep 300 seconds as an exapump-side default | Fixes issue #38 for users who never find the flag; matches the exarrow-rs 0.16.0 default. |
| `--timeout 0` is rejected at parse time | Treat `0` as "no timeout" | Omitting the flag already means "no timeout". A `0` that arms a timer elapsing immediately would fail every export, so no reading of `0` is useful. |
| Merge the compression guard into one format-compatibility function | Add a second parallel guard `reject_timeout_for_parquet` | Keeps one home for the format-to-option mapping and one statement of the "validate before touching the export source" ordering intent. |

## Features

| Feature | Status | Spec |
|---------|--------|------|
| cli/export-command-structure | CHANGED | `specs/_plans/add-export-timeout-option/cli/export-command-structure/spec.md` |
| export/csv-export | CHANGED | `specs/_plans/add-export-timeout-option/export/csv-export/spec.md` |

## Recording Notes

Instructions for the recorder, beyond the scenario-marker merge.

Before archiving, replace the `# Feature:` description line and the `## Background` paragraph of `specs/cli/export-command-structure/spec.md` and `specs/export/csv-export/spec.md` with the corresponding text from this plan's delta files verbatim.

Both permanent Backgrounds currently name `--compression` as the only format-specific export option. The marker merge carries scenarios only, so without this step the recorded specs describe `--timeout` in their scenarios while their Background text denies it exists.

## Impact

Long CSV exports stop failing at 300 seconds. `exapump export --format csv` without `--timeout` now runs until Exasol finishes the EXPORT statement, so an export that previously died at 300 seconds completes.

Parquet and Arrow exports lose the same implicit 300-second bound, without gaining a flag to restore it. exarrow-rs 0.16.0 builds their internal CSV options from the new `None` default, and neither `ParquetExportOptions` nor `ArrowExportOptions` accepts a timeout. A Parquet export that previously failed fast on a hung server now waits indefinitely unless the DSN carries `?query_timeout=<seconds>`.

A CSV export that exceeds `--timeout` now deletes the output files it wrote and names them on stderr; earlier versions left the partial file in place.

Two behaviors are newly rejected instead of accepted or ignored: `--timeout` with `--format parquet`, and `--timeout 0`. Both exit non-zero with a message naming the flag.

No existing command line changes meaning. No config, DSN, or output format changes.

## Dependencies

| Dependency | Current | Required | Reason |
|------------|---------|----------|--------|
| exarrow-rs | 0.15.1 | 0.16.0 | `CsvExportOptions::timeout_ms` becomes `Option<u64>` defaulting to `None` |

exarrow-rs 0.16.0 lists six breaking changes. exapump is expected to compile unchanged because it never sets `timeout_ms`, never constructs or destructures `ExportError::Timeout`, never implements `TransportProtocol`, and never calls `Connection::is_closed()`. Task 1.1 verifies this rather than assuming it.

## Implementation Tasks

- [ ] 1.1 Bump `exarrow-rs` to `0.16.0` in `Cargo.toml`, refresh `Cargo.lock`, and confirm `cargo build`, `cargo clippy`, and `cargo test` are green against the local Exasol Docker database. Read the exarrow-rs 0.16.0 CHANGELOG and confirm each of the six breaking changes leaves exapump's sources untouched; report any that does not.
- [ ] 2.1 Add `timeout: Option<u64>` to `ExportArgs` in `src/cli.rs` with `#[arg(long, value_name = "SECONDS", value_parser = clap::value_parser!(u64).range(1..=18_446_744_073_709_551))]`. The upper bound is `u64::MAX / 1000`, so the seconds-to-milliseconds product in task 2.3 cannot overflow and needs no runtime guard. The help text states the unit (seconds) and that the option applies to CSV format only. Place the field next to `--null-value`, ahead of `--compression`. Add `timeout: None` to `base_args()` in the `src/commands/export.rs` test module so the existing unit tests still compile.
- [ ] 2.2 Replace `reject_compression_for_csv` in `src/commands/export.rs` with `reject_format_mismatched_options`, which rejects `--compression` with `--format csv` and `--timeout` with `--format parquet`. Keep the existing error text for the compression case verbatim; the timeout case reads `--timeout is only supported for CSV format`. Carry the existing doc comment's ordering rationale onto the merged function and update the single call site in `run`. Rename the two `reject_compression_for_csv_*` unit tests (`src/commands/export.rs:417,429`) to the new function name and update the call inside `compression_with_csv_is_rejected_before_the_missing_source_error` (`:438`, calling at `:445`), keeping all three assertions unchanged, and add unit tests covering `--timeout` with Parquet, `--timeout` with CSV, and the rejection firing ahead of the missing-source error.
- [ ] 2.3 Wire the timeout into `build_csv_options` in `src/commands/export.rs`: when `args.timeout` is `Some(secs)`, call `.timeout_ms(secs * 1000)`; leave the builder untouched otherwise. Task 2.1's parser bound makes the multiplication overflow-free, so no checked arithmetic is needed here. Add unit tests `build_csv_options_leaves_timeout_unset_by_default` and `build_csv_options_converts_timeout_seconds_to_milliseconds`.
- [ ] 2.4 Delete partial output when the deadline elapses, in `src/commands/export.rs` `export_csv`. On both paths, bind the call's `Result<u64, exarrow_rs::ExportError>` before applying `?` and `match` the `Err` arm on `ExportError::Timeout { .. }` directly, so a change to the upstream error shape is a compile error rather than a silently skipped cleanup. Remove the files the export had written, then return the error unchanged. Single-file path: remove `base_path`. Split path: add `SplitCsvWriter::discard(&mut self) -> Vec<PathBuf>` to `src/split.rs`, which removes only the files the writer itself opened and returns their paths for the stderr line. Track them explicitly: add a `created: Vec<PathBuf>` field to `SplitCsvWriter`, push `path` inside `open_next_file` (`src/split.rs:129-144`) immediately after `File::create(&path)?` succeeds, and have `discard` drop `current_file`, then `std::fs::remove_file` each recorded path, collecting the ones removed. Neither `split_writer.finish()` nor `rename_single_split` runs on the timeout path, so nothing else cleans up after them. Print one stderr line naming every removed file. Treat a failed removal as non-fatal: warn on stderr and still return the timeout error. Add unit tests for `discard` covering zero, one, and three files opened, and asserting that a pre-existing `data_000.csv` the writer never opened survives. [expert]
- [ ] 2.5 Extend `tests/cli_test.rs`: add `--timeout` to the assertions in `export_help_shows_all_arguments`; add `export_timeout_help_documents_seconds_and_csv_only`, `export_timeout_zero_rejected`, and `export_timeout_rejected_for_parquet`. Model the last one on `export_compression_rejected_for_csv`, using `fixtures::DUMMY_DSN` so no database is needed.
- [ ] 2.6 Add integration tests `export_with_generous_timeout_succeeds`, `export_exceeding_timeout_fails`, and `export_split_exceeding_timeout_fails` to `tests/export_test.rs`, all gated by `fixtures::require_exasol!()`. The two timeout tests need a query whose export reliably exceeds 1 second: start from a self-cross-joined row generator such as `SELECT t1.n * t2.n AS v FROM (SELECT LEVEL AS n FROM DUAL CONNECT BY LEVEL <= 3000) t1, (SELECT LEVEL AS n FROM DUAL CONNECT BY LEVEL <= 3000) t2`, verify against the running Docker database that the untimed export takes well over 1 second, and raise the row count until it does. Assert a non-zero exit and `timed out after 1000ms` on stderr. Assert the output file is gone: `data.csv` for the single-file test, and every `data_NNN.csv` for the split test. Assert stderr names the removed file — `data.csv` in `export_exceeding_timeout_fails`, at least one `data_NNN.csv` in `export_split_exceeding_timeout_fails` — so a test fails when the cleanup never ran rather than passing on an export that produced no file. The split test adds `--max-rows-per-file 1000`, creates a `data.csv` before the export, and asserts that file is still present and unmodified afterwards. Confirm the aborted export leaves the database usable by running the schema teardown afterwards. [expert]
- [ ] 3.1 Update the export options table in `docs/file_exchange.md` (the three-column `Flag | Default | Description` table at `docs/file_exchange.md:50`): add a `--timeout` row whose three cells are `--timeout`, an em dash for the default, and "Client-side export deadline in seconds; CSV only". Add a paragraph below the table naming all three timeouts a user can reach, because two of them do not bound an export and a reader told only "the server-enforced bound lives in the DSN" will reach for the wrong one: `--timeout` (client-side export deadline, CSV only), `?query_timeout=<seconds>` in the DSN (server-enforced query bound, all formats), and `?timeout=<seconds>` in the DSN (connect deadline only, not an export bound). State in that same paragraph that an elapsed `--timeout` deletes the output files the export wrote. Add a paragraph to the Export section stating that Parquet and Arrow exports have no client-side deadline and cannot take one, that exapump versions before 0.12.0 bounded them implicitly at 300 seconds, and that `?query_timeout=<seconds>` in the DSN is the only bound available for those formats. Add an example invocation using `--timeout`.
- [ ] 3.2 Bump the package version in `Cargo.toml` from `0.11.4` to `0.12.0` and add a `## 0.12.0` CHANGELOG entry covering: the new `--timeout` option, the exarrow-rs 0.16.0 bump, the removal of the implicit 300-second bound on CSV exports (issue #38), the same removal for Parquet and Arrow exports with `?query_timeout=` named as the replacement bound, and the fact that a timed-out CSV export now deletes the partial output files it wrote.

## Parallelization

| Parallel Group | Tasks |
|----------------|-------|
| Group A | 1.1 |
| Group B | 2.1 |
| Group C | 2.2 |
| Group C2 | 2.3 |
| Group D | 2.4 |
| Group E | 2.5, 2.6, 3.1, 3.2 |

Sequential dependencies:
- Group A → Group B (the 0.16.0 default is what makes an unset timeout mean "unbounded")
- Group B → Group C (2.2 reads `ExportArgs.timeout`)
- Group C → Group C2 (both edit `src/commands/export.rs` and its test module)
- Group C2 → Group D (cleanup runs on the error path of the export that task 2.3 bounds)
- Group D → Group E (the integration tests in 2.6 assert the cleanup post-condition)

No group holds two tasks that touch the same file. Group E's four tasks are disjoint: `tests/cli_test.rs`, `tests/export_test.rs`, `docs/file_exchange.md`, and `Cargo.toml` plus `CHANGELOG.md`.

## Dead Code Removal

| Type | Location | Reason |
|------|----------|--------|
| Function | `reject_compression_for_csv` in `src/commands/export.rs:87` | Absorbed into `reject_format_mismatched_options`, which owns both format-compatibility rules |

## Verification

### Scenario Coverage

| Scenario | Test Type | Test Location | Test Name |
|----------|-----------|---------------|-----------|
| Export help shows all arguments | Integration | `tests/cli_test.rs` | `export_help_shows_all_arguments` |
| Timeout help states seconds and CSV-only scope | Integration | `tests/cli_test.rs` | `export_timeout_help_documents_seconds_and_csv_only` |
| Timeout of zero rejected | Integration | `tests/cli_test.rs` | `export_timeout_zero_rejected` |
| CSV export without a timeout runs unbounded | Unit | `src/commands/export.rs` | `build_csv_options_leaves_timeout_unset_by_default` |
| Timeout value is interpreted as whole seconds | Unit | `src/commands/export.rs` | `build_csv_options_converts_timeout_seconds_to_milliseconds` |
| CSV export completes within its timeout | Integration | `tests/export_test.rs` | `export_with_generous_timeout_succeeds` |
| CSV export exceeding its timeout fails | Integration | `tests/export_test.rs` | `export_exceeding_timeout_fails` |
| Split CSV export exceeding its timeout fails | Integration | `tests/export_test.rs` | `export_split_exceeding_timeout_fails` |
| Timeout option rejected for Parquet format | Integration | `tests/cli_test.rs` | `export_timeout_rejected_for_parquet` |

The two unit-tested scenarios cover pure construction of `CsvExportOptions` from parsed arguments, with no I/O.

### Manual Testing

Set `export EXAPUMP_DSN='exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0'` first, and create `EXPT.T` with `exapump sql 'CREATE SCHEMA IF NOT EXISTS EXPT; CREATE TABLE EXPT.T (id INT); INSERT INTO EXPT.T VALUES (1), (2), (3);'`.

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| cli/export-command-structure | `exapump export --help` | Lists `--timeout <SECONDS>` with help text naming seconds and CSV format |
| cli/export-command-structure | `exapump export --table EXPT.T --output /tmp/t.csv --format csv --timeout 0` | Exit 1; stderr reports `0` as an invalid value for `--timeout` |
| export/csv-export | `exapump export --table EXPT.T --output /tmp/t.csv --format csv --timeout 300` | Exit 0; stderr `Exported 3 rows`; `/tmp/t.csv` holds a header row and 3 data rows |
| export/csv-export | `exapump export --table EXPT.T --output /tmp/t.csv --format csv` | Exit 0; stderr `Exported 3 rows` |
| export/csv-export | `exapump export --query 'SELECT t1.n * t2.n FROM (SELECT LEVEL AS n FROM DUAL CONNECT BY LEVEL <= 3000) t1, (SELECT LEVEL AS n FROM DUAL CONNECT BY LEVEL <= 3000) t2' --output /tmp/big.csv --format csv --timeout 1` | Exit 1; stderr names the removed `/tmp/big.csv` and reports `Error: Export timed out after 1000ms; the transport was terminated, reconnect before the next operation`; no `/tmp/big.csv` on disk |
| export/csv-export | the same query with `--output /tmp/big.csv --format csv --max-rows-per-file 1000 --timeout 1` | Exit 1; stderr reports the same timeout; no `/tmp/big_000.csv` on disk; `/tmp/big.csv` was never created |
| export/csv-export | `exapump export --table EXPT.T --output /tmp/t.parquet --format parquet --timeout 60` | Exit 1; stderr `Error: --timeout is only supported for CSV format` |

Tear down with `exapump sql 'DROP SCHEMA EXPT CASCADE'`.

### Checklist

| Step | Command | Expected |
|------|---------|----------|
| Build | `cargo build` | Exit 0 |
| Test | `cargo test` | 0 failures, Exasol reachable at `localhost:8563` |
| Lint | `cargo clippy` | 0 errors/warnings |
| Format | `cargo fmt --check` | No changes |
