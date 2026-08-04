# Tasks: fix-sonar-quality-gate

## Phase 1: Record the baseline and fix coverage measurement (Group A)
- [x] 1.1 Start Exasol locally, run `cargo llvm-cov --no-report`, then report `--summary-only` and `--lcov` separately; record TOTAL and per-file coverage, and `profile.rs` baseline coverable/uncovered counts
- [x] 1.1b Emit a per-function uncovered-source-line count for `src/commands/profile.rs` from the lcov file (bucket `DA` lines by enclosing `FN` start line); self-check against `LF`/`LH`; never compare against the JSON export's line rule [expert]
- [x] 1.1b-ii Sum the task 1.1b table into R over the nine trait-reachable functions, then subtract every line task 2.2 relocates into `TerminalPrompter` [expert]
- [x] 1.1c Compare R against 194 lines; record `targets stand` or `targets revised` in implementation notes
- [x] 1.1d Conditional on R < 194: recompute `profile.rs` and overall coverage targets from the baseline denominator, rewrite § Requirements rows, append to decision-log.md [7] § Gate (skipped — R >= 194)
- [x] 1.7 Query the SonarCloud `rust:S3776` rule threshold and profile override; record the counting method (base increment, recursion, labeled jumps, nesting rule) and pass its two mandatory calibrations (`profile::show` = 19, `profile::init` = 22) before any Phase 2 counting

## Phase 1: CI pipeline changes (Group B)
- [x] 1.2 Add `llvm-tools-preview` component and the `cargo-llvm-cov` install step to the `integration-tests` toolchain step
- [x] 1.3 Replace `integration-tests`' `cargo test --verbose` with `cargo llvm-cov --lcov --output-path lcov.info`; add the `lcov.info` upload-artifact step
- [x] 1.4 Delete the `unit-tests` job; change `sonar` job's `needs` to `[integration-tests]`
- [x] 1.5 Add a step reading the `Cargo.toml` version and pass `-Dsonar.projectVersion=<version>` to the sonar-scan action
- [x] 1.6 Update the `sonar.rust.lcov.reportPaths` comment in `sonar-project.properties`

## Phase 2: Cut cognitive complexity (Group C1 — profile.rs / sql.rs / export.rs)
- [x] 2.1 `profile::show` (19): extract pure row-mapping function, mask password fields, unit-test populated/minimal/not-found profiles
- [x] 2.2 `profile::init`/`edit` (22 each): introduce `ProfilePrompter` trait, route all `inquire`/`rpassword` calls through it, `TerminalPrompter` production impl, `ScriptedPrompter` test impl; keep BucketFS port helper non-retrying; unit-test all init/edit paths [expert]
- [x] 2.3 `sql::strip_comments` (35) / `split_statements` (60): extract per-state transition handling below complexity 15 without changing scanner output; extend the 78 existing unit tests for any newly-uncovered branch [expert]
- [x] 2.4 `sql::run` (54): extract input-source selection, per-format rendering, status-line construction; owns the `write_csv`/`write_json` signature change to `&mut impl Write` and updates the four `interactive.rs` call sites
- [x] 2.7 `export::run` (38): extract export-source resolution, CSV option assembly, split-writer selection; unit-test each, including `--compression` + CSV rejection

## Phase 2: Cut cognitive complexity (Group C2 — interactive.rs, after Group C1)
- [x] 2.5 `interactive::execute_statement` (66): extract result-rendering/error-reporting branches; consolidate the 50 duplicated lines; preserve exact prompt/output/error text `tests/cli_test.rs` asserts on [expert]
- [x] 2.6 `interactive::run` (26): extract REPL setup and per-line meta-command/SQL dispatch; unit-test dispatch classification

## Phase 3: Close remaining coverage gaps (Group D, after C2)
- [x] 3.1 Raise `profile.rs` to its § Requirements target via the `ProfilePrompter` seam; re-run the task 1.1b measurement and require the nine-function uncovered sum to have fallen by at least R
- [x] 3.2 Re-measure TOTAL; if below target, add tests for next-largest gaps in order (sql.rs, interactive.rs, bucketfs.rs, export.rs, wait.rs, split.rs); repeat the uninflatable R check as final gate

## Phase 4: Verify the gate
- [x] 4.1 Run full local checklist (build, test, clippy, fmt, cargo deny, coverage) against a running Exasol container

## Phase 4: Review Fixes
- [x] 4.2 [SWALLOWED_ERROR] `.github/workflows/ci.yml`: rewrite the "Read crate version" step to a `run: |` block starting with `set -euo pipefail`, assign the extracted version to a shell variable, and exit non-zero with a message naming `Cargo.toml` when the variable is empty, before writing it to `$GITHUB_OUTPUT`
- [x] 4.3 [ASSERTION_FREE_TEST] `src/commands/export.rs`: change `build_csv_options_applies_null_value_when_non_empty` to assert `options.null_value == Some("N/A".to_string())`; change `build_csv_options_succeeds_without_compression` to assert `null_value == None`, `column_separator == ','`, `column_delimiter == '"'`, `with_column_names == true`
- [x] 4.4 [MISSING_BOUNDARY_TEST] `src/commands/export.rs`: extract `fn reject_compression_for_csv(args: &ExportArgs) -> anyhow::Result<()>` out of `build_csv_options`, call it as the first statement of `run` before `resolve_export_source`; add `compression_with_csv_is_rejected_before_the_missing_source_error` asserting the compression message when compression+CSV+no table/query; retarget `build_csv_options_rejects_compression` at the new function
- [x] 4.5 [TOO_MANY_ARGUMENTS] `src/commands/export.rs`: delete the `with_header: bool` parameter from `export_csv` (read `options.with_column_names` instead), drop the argument at the `run` call site; introduce `struct ParquetTarget<'a> { base_path: &'a Path, limits: SplitLimits, compression: Option<&'a Compression> }` and change `export_parquet`, `export_parquet_split`, `write_parquet_batches` to take it
- [x] 4.6 [INLINE_COMMENT] `src/commands/export.rs` + `src/commands/sql.rs`: delete the three in-body comment blocks in `export_parquet_split` and add a doc comment above it stating the zero-row schema query, batch_size/max_rows alignment, and empty-result file-write rationale; delete the in-body comment in `StatementScanner::scan_script_body` and add a doc comment above it explaining the lone-`/` terminator rule
- [x] 4.7 [TOO_MANY_ARGUMENTS] `src/commands/interactive.rs`: introduce `struct ReplSession { conn: exarrow_rs::Connection, rl: DefaultEditor, buffer: String, format: InteractiveFormat }`, construct it in `run`, convert `dispatch_line` into `async fn dispatch_line(&mut self, line: &str) -> ControlFlow` on it; update `run`'s loop accordingly
- [x] 4.8 [BOOLEAN_FLAG_PARAMETER] `src/commands/profile.rs`: split `ProfilePrompter::text` into `fn text(&mut self, label: &str, default: Option<&str>) -> anyhow::Result<String>` and `fn required_text(&mut self, label: &str, default: Option<&str>) -> anyhow::Result<String>`; update all 15 call sites; rename the two tests to target `required_text`/`text`
- [x] 4.9 [OUTPUT_PARAMETER] `src/commands/sql.rs`: introduce `struct ResultRenderer { rendered_any: bool }` with `fn new()` and `fn render(&mut self, batches, format, writer)`, replacing `render_result`'s `first_select: &mut bool` output parameter; construct one `ResultRenderer` in `run`; retarget the three `render_result_*` unit tests
- [x] 4.10 [SELECTOR_ARGUMENT] `src/commands/sql.rs`: remove the `stmt_type: StatementType` parameter from `execute_one`, compute `StatementType::from_sql(stmt)` as the `match` scrutinee inside it; delete the `let stmt_type = …` binding in `run` and update the call site
- [x] 4.11 [SUPPRESSED_WARNING] `src/commands/profile.rs`: introduce `#[derive(Debug, Clone, PartialEq)] struct BucketFsSettings` with the seven named fields plus `fn none()` and `fn from_profile(profile: &Profile)`; change `prompt_bucketfs`/`edit_bucketfs` to return `anyhow::Result<BucketFsSettings>` and delete both `#[allow(clippy::type_complexity)]`; replace `init_profile`'s `no_bucketfs` arm with `BucketFsSettings::none()` and `edit_profile`'s `no_bucketfs` arm plus `edit_bucketfs`'s declined arm with `BucketFsSettings::from_profile(current)`; assign the fields into the `Profile` literals by name; update every `assert_eq!(settings, (…))` to a struct literal [expert]
- [x] 4.12 [INFORMATION_LEAKAGE] `src/commands/sql.rs` + `src/commands/interactive.rs`: make `execute_one`, `StatementOutcome` (and its variants) and `total_rows` `pub(crate)`; rewrite `interactive::execute_and_report` to call `sql::execute_one` and match `StatementOutcome` (`Rows` → `print_result`, `RowsAffected` → `rows_affected_line`, `Ok` → `"OK"`); delete `interactive::row_count` and route `print_result` through `sql::total_rows`; re-run the task 2.5 byte-level REPL differential check and confirm both hashes still match [expert]
- [x] 4.13 [DUPLICATE_TEST] `src/commands/profile.rs` test module: add `fn edit_bucketfs_prompts(host_default, port_default, bucket_default) -> Vec<Asked>` and `fn prompt_bucketfs_prompts(port_default) -> Vec<Asked>` building the shared confirm+text prompt prefixes once; rewrite each `edit_bucketfs_*`/`prompt_bucketfs_*` test to extend the helper's output with only its distinguishing suffix, without weakening any `assert_eq!` [expert]
