# Code Review Findings: add-export-timeout-option

## Summary
- Files reviewed: 9
- Total findings: 7 (standard: 6, expert: 1)

Verified clean during review: `cargo clippy --all-targets` (0 warnings), `cargo fmt --check` (0 diffs). No suppressed warnings, no unused imports, no skipped tests, no work-tracking comments.

## Standard fixes

### src/cli.rs

#### [MAGIC_NUMBER] Timeout upper bound is an opaque 17-digit literal
- Location: line 149
- Issue: `value_parser = clap::value_parser!(u64).range(1..=18_446_744_073_709_551)` states the bound as a bare literal. Its meaning — `u64::MAX / 1000`, the largest whole-second value whose millisecond conversion in `build_csv_options` (`src/commands/export.rs:110`, `secs * 1000`) cannot overflow — is recorded only in `plan.md`, not in the code. Nothing at either site links the bound to the multiplication it protects, so a future edit to the range silently arms an overflow in a different file.
- Fix: In src/cli.rs, add a module-scope constant `const MAX_TIMEOUT_SECONDS: u64 = u64::MAX / 1000;` with a doc comment stating that it is the largest value whose seconds-to-milliseconds conversion in `build_csv_options` cannot overflow, and replace the literal in the `ExportArgs::timeout` `value_parser` range with `1..=MAX_TIMEOUT_SECONDS`.

### src/commands/export.rs

#### [DUPLICATE_TEST] Ordering test cannot fail independently of the plain rejection test
- Location: line 519, `timeout_with_parquet_is_rejected_before_the_missing_source_error`
- Issue: the test calls `reject_format_mismatched_options(&args)` with `format = Parquet`, `timeout = Some(30)` and asserts the message `--timeout is only supported for CSV format` — identical inputs and identical assertion to `reject_format_mismatched_options_rejects_timeout_with_parquet_format` (line 505). The only difference is setting `args.table = None; args.query = None`, and `reject_format_mismatched_options` (lines 89-97) never reads either field, so those assignments cannot change the outcome. The test therefore fails only when the other one also fails. It also does not test the ordering its name claims: ordering is a property of `run` (line 347), not of the validator called in isolation, and the ordering is unreachable from the CLI anyway because `ExportArgs::table` and `ExportArgs::query` carry `required_unless_present` (`src/cli.rs:104-119`), so clap rejects a missing source before `run` is entered.
- Fix: In src/commands/export.rs, delete the unit test `timeout_with_parquet_is_rejected_before_the_missing_source_error` (line 519) in its entirety. Do not add a replacement — `reject_format_mismatched_options_rejects_timeout_with_parquet_format` already covers the rejection, and the ordering it names is unreachable through the CLI.

### src/split.rs

#### [MISSING_DESIGN_INTENT] `discard`'s doc comment omits the post-condition and the no-op case
- Location: lines 130-135, `SplitCsvWriter::discard`
- Issue: the doc comment states what `discard` removes but not that the writer is spent afterwards. `discard` clears `created` and `current_file` while leaving `file_index`, `total_rows` and `rows_in_file` untouched, so a later `finish()` (line 108) returns `self.file_index + 1` as the file count for files that no longer exist, and a later `write_all` re-creates `data_000.csv`. That is an undocumented ordering contract on a public method. The comment also does not state that the method is a legitimate no-op when the abort happened before the first split file was opened, which is the case every caller hits today.
- Fix: In src/split.rs, extend the doc comment on `SplitCsvWriter::discard` with two sentences: that the writer is spent after this call and neither `finish` nor any further write may follow it, and that the method returns an empty vector when the abort preceded the first split file being opened, which is a normal outcome rather than a failure.

### tests/cli_test.rs

#### [ASSERTION_FREE_TEST] Help test's `CSV` assertion cannot fail
- Location: lines 535-546, `export_timeout_help_documents_seconds_and_csv_only`
- Issue: the test chains `contains("--timeout").and(contains("seconds")).and(contains("CSV"))` over the whole of `export --help` stdout, so it never checks that those words belong to the `--timeout` entry. `cargo run -- export --help` emits `CSV field delimiter [default: ,]` and `CSV quoting character [default: "]` (from `src/cli.rs:132,136`), so `contains("CSV")` passes regardless of what `--timeout`'s help says. The test would still pass if the timeout help text were changed to omit the CSV-only scope entirely — exactly the regression it is named for. clap's `wrap_help` feature is not enabled (`Cargo.toml` requests only `derive` and `env`), and the verified help output keeps the description on a single unbroken line: `Client-side export deadline in seconds (CSV format only)`.
- Fix: In tests/cli_test.rs, replace the three chained predicates in `export_timeout_help_documents_seconds_and_csv_only` with a single `predicate::str::contains("Client-side export deadline in seconds (CSV format only)")` so the assertion binds the wording to the `--timeout` entry.

#### [REDUNDANT_COMMENT] Comment restates the test name
- Location: line 569
- Issue: `// --timeout with --format parquet should fail with a descriptive error` inside `export_timeout_rejected_for_parquet` repeats what the test name and the assertions already say, adding no rationale.
- Fix: In tests/cli_test.rs, delete the comment line `// --timeout with --format parquet should fail with a descriptive error` at the top of `export_timeout_rejected_for_parquet`.

#### [MISSING_BOUNDARY_TEST] Upper bound of `--timeout` is untested
- Location: line 548 (`export_timeout_zero_rejected` is the only bound test); `src/commands/export.rs:565` (`build_csv_options_converts_timeout_seconds_to_milliseconds` covers only 30)
- Issue: the overflow safety of `secs * 1000` in `build_csv_options` (`src/commands/export.rs:110`) rests entirely on the clap range's upper bound, and no test exercises that bound from either side. The lower bound has `export_timeout_zero_rejected`; the maximum accepted value, the first rejected value, and the conversion at the maximum have no coverage. A change to the range would pass the whole suite while making the multiplication overflow.
- Fix: In tests/cli_test.rs, add `export_timeout_above_max_rejected`, modelled on `export_timeout_zero_rejected`, invoking export with `--format csv` and `--timeout 18446744073709552` and asserting `.failure()` with stderr containing `--timeout`. In src/commands/export.rs, add unit test `build_csv_options_converts_the_maximum_timeout_without_overflow` that sets `args.timeout = Some(18_446_744_073_709_551)` and asserts `options.timeout_ms == Some(18_446_744_073_709_551_000)`.

## Expert fixes

### src/split.rs

#### [INFORMATION_LEAKAGE] Partial-output removal policy and its warning string live in two modules
- Location: `src/split.rs:136-151` (`SplitCsvWriter::discard`) and `src/commands/export.rs:143-155` (`remove_partial_output`)
- Issue: both functions independently implement the same policy — remove the file, treat `ErrorKind::NotFound` as success-with-nothing-removed, warn on any other error, return only the paths actually removed — and both emit the byte-identical string `"Warning: failed to remove partial output {}: {err}"`. One design decision (how exapump removes and reports a partial export output) has two owners, so a change to the wording or to the NotFound tolerance must be made in both files or they drift, with nothing to catch the drift. The duplication also puts the first and only user-facing `eprintln!` in `src/split.rs`: every other `eprintln!` in the crate lives in `src/commands/*` or `src/config.rs`, so this change pushes CLI-facing output into a byte-level writer module that previously had none.
- Fix: In src/split.rs, add `pub fn remove_partial_output(path: &Path) -> Option<PathBuf>` holding the single implementation of the policy — `std::fs::remove_file`, `Some(path)` on success, `None` on `ErrorKind::NotFound`, and `None` plus the existing `Warning: failed to remove partial output` line on any other error. Rewrite `SplitCsvWriter::discard`'s body to `std::mem::take(&mut self.created).into_iter().filter_map(|p| remove_partial_output(&p)).collect()` after clearing `current_file`. Delete `remove_partial_output` from src/commands/export.rs and call `crate::split::remove_partial_output` there instead, adapting the single call site at src/commands/export.rs:202 to the `Option` return (`report_removed_output(crate::split::remove_partial_output(base_path).as_slice())`). Move the two unit tests `remove_partial_output_deletes_the_file_and_names_it` (src/commands/export.rs:631) and `remove_partial_output_names_nothing_when_the_file_is_absent` (src/commands/export.rs:643) into src/split.rs's test module, updating their assertions from `Vec<PathBuf>` to `Option<PathBuf>`. Leave `report_removed_output` in src/commands/export.rs — the `Removed partial output:` line is CLI-layer output and must not move into src/split.rs. Re-run the `src/split.rs` and `src/commands/export.rs` unit tests plus `tests/export_test.rs::export_exceeding_timeout_fails` to confirm the stderr contract is unchanged.

## Assessment: split-path cleanup — no action required

Not a finding. Recorded here because the integration-test author raised it and the reviewer was asked to rule on it.

**Ruling: keep `SplitCsvWriter::discard` and the `created: Vec<PathBuf>` tracking. Keep `tests/export_test.rs::export_split_exceeding_timeout_fails` as written, including its explanatory comment. No implementer action.**

Reasoning:

1. **It is not dead code by the taxonomy's definition.** `[UNREACHABLE_CODE]` and `[UNUSED_FUNCTION]` describe code with no caller and no execution path. `discard` has a caller (`src/commands/export.rs:187`) and its body is fully exercised by four deterministic unit tests in `src/split.rs` covering zero, one, and three files opened plus the foreign-file case. What is currently unreachable is one specific *route* into it — the production timeout path — not the code.

2. **The unreachability rests on two undocumented upstream internals, not on a contract.** The analysis depends on `exarrow_rs::export::csv::export_to_stream` buffering the entire result before writing a byte, and on `SplitCsvWriter::poll_write` never returning `Pending`. Neither is a promise exarrow-rs makes; the first is precisely the kind of thing a library fixes, since buffering a full export in memory is a scalability defect. A patch-level bump could make the path live with no compile error, no test failure, and no signal in this repo.

3. **The failure mode of deleting it is silent and user-visible.** If upstream starts streaming and the cleanup is gone, a timed-out split export leaves orphaned `data_000.csv … data_NNN.csv` on the user's disk while `CHANGELOG.md` and `docs/file_exchange.md` both promise the files are deleted. Nothing in the suite would catch it. The failure mode of keeping it is one unused `Vec<PathBuf>` field.

4. **Re-adding it later is disproportionately expensive.** This is error-path cleanup — the hardest category to test and the easiest to get subtly wrong. `discard`'s non-obvious requirement (remove only files this writer opened, so a same-named pre-existing file survives) is already solved and already pinned by `discard_leaves_a_same_named_file_it_never_opened`. That knowledge is cheap to keep and expensive to rediscover under a bug report.

5. **The vacuous assertions are the correct shape for a forward guard.** `survivors.is_empty()` passing vacuously today is the honest outcome, and it becomes load-bearing the instant upstream streams — which is exactly when it is needed. The author's comment at the assertion documents why it is currently vacuous, which is a genuine non-obvious WHY and earns its place under the minimal-comment rule. Rewriting the test to assert the *absence* of the `Removed partial output` line would lock in the upstream implementation detail and fail when upstream improves — strictly worse.

The one gap worth closing is documentation, and it is filed above as `[MISSING_DESIGN_INTENT]` on `src/split.rs` rather than as a code change.
