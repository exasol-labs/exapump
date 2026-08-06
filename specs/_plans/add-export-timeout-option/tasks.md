# Tasks: add-export-timeout-option

## Phase 2: Implementation (Group A)
- [x] 1.1 Bump `exarrow-rs` to `0.16.0` in `Cargo.toml`, refresh `Cargo.lock`, confirm `cargo build`, `cargo clippy`, `cargo test` green against local Exasol Docker DB. Read the exarrow-rs 0.16.0 CHANGELOG and confirm each of the six breaking changes leaves exapump's sources untouched; report any that does not.

## Phase 2: Implementation (Group B)
- [x] 2.1 Add `timeout: Option<u64>` to `ExportArgs` in `src/cli.rs` with the clap range parser `1..=18_446_744_073_709_551`; help text states seconds and CSV-only. Place next to `--null-value`, ahead of `--compression`. Add `timeout: None` to `base_args()` in the `src/commands/export.rs` test module.

## Phase 2: Implementation (Group C)
- [x] 2.2 Replace `reject_compression_for_csv` with `reject_format_mismatched_options` in `src/commands/export.rs`; rejects `--compression` with csv and `--timeout` with parquet. Rename/extend the unit tests, update the call site in `run`.

## Phase 2: Implementation (Group C2)
- [x] 2.3 Wire the timeout into `build_csv_options`: `Some(secs)` → `.timeout_ms(secs * 1000)`. Add unit tests `build_csv_options_leaves_timeout_unset_by_default` and `build_csv_options_converts_timeout_seconds_to_milliseconds`.

## Phase 2: Implementation (Group D)
- [x] 2.4 Delete partial output when the deadline elapses, in `export_csv`. Match `ExportError::Timeout { .. }` on both paths; add `SplitCsvWriter::discard` plus a `created: Vec<PathBuf>` field in `src/split.rs`. Print removed files on stderr; a failed removal warns but still returns the timeout error. Unit tests for `discard` covering zero, one, three files and a pre-existing untouched `data_000.csv`. [expert]

## Phase 2: Implementation (Group E)
- [x] 2.5 Extend `tests/cli_test.rs`: add `--timeout` to `export_help_shows_all_arguments`; add `export_timeout_help_documents_seconds_and_csv_only`, `export_timeout_zero_rejected`, `export_timeout_rejected_for_parquet`.
- [x] 2.6 Add integration tests `export_with_generous_timeout_succeeds`, `export_exceeding_timeout_fails`, `export_split_exceeding_timeout_fails` to `tests/export_test.rs`, gated by `fixtures::require_exasol!()`. [expert]
- [x] 3.1 Update the export options table and surrounding prose in `docs/file_exchange.md` per the plan's task 3.1.
- [x] 3.2 Bump the package version in `Cargo.toml` `0.11.4` → `0.12.0` and add the `## 0.12.0` CHANGELOG entry.

## Phase 4: Review Fixes
- [x] 4.1 [INFORMATION_LEAKAGE] Give the partial-output removal policy one owner: add `pub fn remove_partial_output(path: &Path) -> Option<PathBuf>` to `src/split.rs`, delegate `SplitCsvWriter::discard` to it, delete the duplicate from `src/commands/export.rs` and call `crate::split::remove_partial_output` at its single call site, and move the two `remove_partial_output_*` unit tests into `src/split.rs`. Leave `report_removed_output` in `src/commands/export.rs`. [expert]
- [x] 4.2 [MAGIC_NUMBER] In `src/cli.rs`, add a module-scope constant `const MAX_TIMEOUT_SECONDS: u64 = u64::MAX / 1000;` with a doc comment stating it is the largest value whose seconds-to-milliseconds conversion in `build_csv_options` cannot overflow, and replace the literal in `ExportArgs::timeout`'s `value_parser` range with `1..=MAX_TIMEOUT_SECONDS`.
- [x] 4.3 [DUPLICATE_TEST] In `src/commands/export.rs`, delete the unit test `timeout_with_parquet_is_rejected_before_the_missing_source_error` in its entirety; `reject_format_mismatched_options_rejects_timeout_with_parquet_format` already covers the rejection and the ordering it names is unreachable through the CLI.
- [x] 4.4 [MISSING_DESIGN_INTENT] In `src/split.rs`, extend the doc comment on `SplitCsvWriter::discard` with two sentences: the writer is spent after this call and neither `finish` nor any further write may follow it, and the method returns an empty vector when the abort preceded the first split file being opened, which is a normal outcome rather than a failure.
- [x] 4.5 [ASSERTION_FREE_TEST] In `tests/cli_test.rs`, replace the three chained predicates in `export_timeout_help_documents_seconds_and_csv_only` with a single `predicate::str::contains("Client-side export deadline in seconds (CSV format only)")` so the assertion binds the wording to the `--timeout` entry.
- [x] 4.6 [REDUNDANT_COMMENT] In `tests/cli_test.rs`, delete the comment line `// --timeout with --format parquet should fail with a descriptive error` at the top of `export_timeout_rejected_for_parquet`.
- [x] 4.7 [MISSING_BOUNDARY_TEST] In `tests/cli_test.rs`, add `export_timeout_above_max_rejected`, modelled on `export_timeout_zero_rejected`, invoking export with `--format csv` and `--timeout 18446744073709552` and asserting `.failure()` with stderr containing `--timeout`. In `src/commands/export.rs`, add unit test `build_csv_options_converts_the_maximum_timeout_without_overflow` that sets `args.timeout = Some(18_446_744_073_709_551)` and asserts `options.timeout_ms == Some(18_446_744_073_709_551_000)`.
- [x] 4.8 Document the split-export timeout caveat in `docs/file_exchange.md`: state, alongside the existing `--timeout` prose, that on a split CSV export `--timeout` bounds only the download phase, not the file-writing phase that follows it, while the single-file path stays bounded through both phases.

## Phase 3: Verification
- [x] 4.1 `cargo build` exit 0
- [x] 4.2 `cargo test` 0 failures against local Exasol
- [x] 4.3 `cargo clippy` 0 errors/warnings
- [x] 4.4 `cargo fmt --check` no changes
