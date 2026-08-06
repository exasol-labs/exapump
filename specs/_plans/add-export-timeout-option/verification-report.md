# Verification Report: add-export-timeout-option

## Verdict

| Result | Details |
|--------|---------|
| **PASS** | `--timeout` ships for CSV exports; exarrow-rs 0.16.0 removes the implicit 300-second bound (issue #38). All suites green, all 9 scenarios covered, all 7 manual cases pass. |
| Code review | 7 findings — 7 fixed |

| Check | Status |
|-------|--------|
| Build | ✓ |
| Tests | ✓ |
| Lint | ✓ |
| Format | ✓ |
| Scenario Coverage | ✓ |
| Manual Tests | ✓ |

## Test Evidence

### Test Results

| Type | Run | Passed | Ignored |
|------|-----|--------|---------|
| Unit | 336 | 336 | 0 |
| Integration | 186 | 185 | 1 |

`cargo test` exit 0 across all 10 binaries: 521 passed, 0 failed, 1 ignored. The ignored test is a pre-existing manual-only case, unrelated to this change.

The `exasol-test` container was recreated during this run with `-p 2581:2581` added, per the CLAUDE.md start command. The 12 BucketFS tests could not reach BucketFS before that and now pass.

### Manual Tests

| Test | Result |
|------|--------|
| `export --help` lists `--timeout <SECONDS>`, help names seconds and CSV-only | ✓ |
| `--timeout 0` → exit 1, `0 is not in 1..=18446744073709551` | ✓ |
| `--timeout 300` on a 3-row table → exit 0, header plus 3 data rows | ✓ |
| No `--timeout` → exit 0, export completes | ✓ |
| `--timeout 1` on a 49M-row query → exit 1, `Export timed out after 1000ms`, `Removed partial output: /tmp/big.csv`, file gone | ✓ |
| Same query split with `--max-rows-per-file 1000 --timeout 1` → exit 1, same timeout error, no `big_NNN.csv`, no `big.csv` | ✓ |
| `--format parquet --timeout 60` → exit 1, `--timeout is only supported for CSV format` | ✓ |
| `DROP SCHEMA EXPT CASCADE` after the aborted exports → `1 statement executed, 0 failed` | ✓ |

## Tool Evidence

### Build

```
cargo build → exit 0
```

### Linter

```
cargo clippy --all-targets -- -D warnings → exit 0, 0 warnings
```

### Formatter

```
cargo fmt --check → exit 0, no diff
```

## Scenario Coverage

| Domain | Feature | Scenario | Test Location | Test Name | Passes |
|--------|---------|----------|---------------|-----------|--------|
| cli | export-command-structure | Export help shows all arguments | `tests/cli_test.rs` | `export_help_shows_all_arguments` | Pass |
| cli | export-command-structure | Timeout help states seconds and CSV-only scope | `tests/cli_test.rs` | `export_timeout_help_documents_seconds_and_csv_only` | Pass |
| cli | export-command-structure | Timeout of zero rejected | `tests/cli_test.rs` | `export_timeout_zero_rejected` | Pass |
| cli | export-command-structure | Timeout option rejected for Parquet format | `tests/cli_test.rs` | `export_timeout_rejected_for_parquet` | Pass |
| export | csv-export | CSV export without a timeout runs unbounded | `src/commands/export.rs` | `build_csv_options_leaves_timeout_unset_by_default` | Pass |
| export | csv-export | Timeout value is interpreted as whole seconds | `src/commands/export.rs` | `build_csv_options_converts_timeout_seconds_to_milliseconds` | Pass |
| export | csv-export | CSV export completes within its timeout | `tests/export_test.rs` | `export_with_generous_timeout_succeeds` | Pass |
| export | csv-export | CSV export exceeding its timeout fails | `tests/export_test.rs` | `export_exceeding_timeout_fails` | Pass |
| export | csv-export | Split CSV export exceeding its timeout fails | `tests/export_test.rs` | `export_split_exceeding_timeout_fails` | Pass |

Review fixes added three tests beyond the plan's coverage table: `export_timeout_above_max_rejected` and `build_csv_options_converts_the_maximum_timeout_without_overflow` pin the upper parser bound from both sides, and `discard_leaves_a_same_named_file_it_never_opened` pins the only-ours cleanup invariant.

## Notes

**`--timeout` bounds only the download phase on split exports.** Upstream `exarrow_rs::export::csv::export_to_stream` buffers the entire result in memory before writing a byte, and `SplitCsvWriter::poll_write` never returns `Pending`. The deadline can therefore elapse only during download, when zero split files exist. Measured on a 9.3-second split export: `--timeout 2` failed with no files created, while `--timeout 3` and `--timeout 5` both succeeded after 8.9 and 9.4 seconds. The single-file path is unaffected — its bound holds through both phases, verified at 3.15 seconds against the same query. `docs/file_exchange.md` states this caveat.

**`SplitCsvWriter::discard`'s removal loop is unreachable through the production timeout path**, for the same reason. Code review ruled to keep it: it has a caller, four deterministic unit tests exercise its body, and its unreachability rests on two undocumented upstream internals rather than a contract — a patch-level exarrow-rs bump could make the path live with no compile error and no failing test. Deleting it would risk orphaned `data_NNN.csv` files on disk while the CHANGELOG and docs promise deletion. The split scenario's `stderr MUST name every deleted file` clause is therefore vacuously satisfied today and becomes load-bearing if upstream streams incrementally.

**The plan's dependency section miscounts exarrow-rs 0.16.0's breaking changes as six; the CHANGELOG lists four** — `timeout_ms` becoming `Option<u64>`, `ExportError::Timeout` gaining `transport_terminated`, `TransportProtocol` gaining `terminate()`, and `Connection::is_closed()` reporting termination. exapump's sources reference none of them, so the bump required no source changes.

**`Exported N rows` counts one more row than the CSV holds.** A 3-row table exports as `Exported 4 rows` with a header plus 3 data rows. The no-timeout path reports the same count, so this is pre-existing upstream behavior, not a regression from this change, and it is out of scope here.

**`export_csv`'s timeout branch has no unit coverage.** `exarrow_rs::Connection` is a concrete type with no trait seam, so a timing-out export cannot be injected without a database. The compile-time `match` on `ExportError::Timeout { .. }` guards the shape; behavioral proof comes from the three integration tests, which passed 12 consecutive runs with no flakiness.
