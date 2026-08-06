# Decision Log: add-export-timeout-option

## Interview

**Q:** The library only lets us set a timeout on CSV exports, not Parquet. What should `--timeout` do when someone exports Parquet?
**A:** CSV only, error on Parquet. `--timeout` works with `--format csv`. Using it with `--format parquet` prints an error and exits non-zero. Explicitly not chosen: filing an upstream exarrow-rs ticket for a Parquet timeout, and wrapping the export in an exapump-side `tokio::time::timeout` that would cover both formats.

**Q:** What should the flag be called?
**A:** `--timeout <seconds>` — a plain integer meaning seconds, converted to milliseconds internally. Matches the issue text. Rejected: `--timeout-secs` for consistency with `exapump wait`; rejected: humanized durations such as `15m` or `1h`.

**Q:** If someone does not pass the flag, what should happen?
**A:** No limit. The export runs to completion. This matches the exarrow-rs 0.16.0 `None` default and fixes the reported bug even for users who never discover the flag.

**Q:** Should this plan expose the database-side `query_timeout`?
**A:** Just document it. Add `--timeout` only. Document in `docs/file_exchange.md` that `--timeout` is a client-side bound and that a database-enforced bound is available by adding `?query_timeout=N` to the DSN. Do not add a `--query-timeout` flag.

## Design Decisions

### [1] Delegate timeout enforcement to exarrow-rs instead of wrapping the export

- **Decision:** exapump parses `--timeout`, converts seconds to milliseconds, and passes the value to `CsvExportOptions::timeout_ms`. exapump arms no timer of its own.
- **Alternatives:** Wrap `export_csv` in `tokio::time::timeout`, which would bound Parquet and Arrow exports too. The user rejected this option in the interview.
- **Rationale:** An exapump-side timer cannot terminate the transport or abort the in-flight HTTP tunnel task. exarrow-rs 0.16.0 does both, and reports through `ExportError::Timeout` whether the transport survived. Duplicating a weaker timer above a stronger one would leave two owners of one decision.
- **Promotes to ADR:** yes

### [2] Bump exarrow-rs to 0.16.0 as part of this plan

- **Decision:** Move the dependency from 0.15.1 to 0.16.0 before wiring the flag.
- **Alternatives:** Ship `--timeout` on 0.15.1, where `timeout_ms` is a plain `u64` defaulting to 300,000. The builder call site would compile unchanged.
- **Rationale:** On 0.15.1 the reported bug survives: omitting the flag still fails at 300 seconds. The fix for issue #38 is the new `None` default, not the flag. exarrow-rs 0.16.0 carries six breaking changes; exapump touches none of the changed surfaces, which task 1.1 verifies by building and running the suite.
- **Promotes to ADR:** no

### [3] Reject `--timeout 0` at parse time

- **Decision:** Constrain the value with `clap::value_parser!(u64).range(1..=18_446_744_073_709_551)`, so `0` fails with clap's standard invalid-value error before any connection opens. The upper bound is `u64::MAX / 1000`, which also removes the millisecond-conversion overflow.
- **Alternatives:** Treat `0` as "no timeout", the convention `wget --timeout=0` follows. Treat `0` as "give up immediately", the convention `docker stop -t 0` follows.
- **Rationale:** Omitting the flag already expresses "no timeout", so a second spelling adds no capability while inviting the reader to guess which of the two conventions applies. A `0` that armed a timer would elapse before any export could finish, making every such invocation fail. Rejecting is the only reading that surprises nobody.
- **Promotes to ADR:** no

### [4] Merge both format-compatibility guards into one function

- **Decision:** Replace `reject_compression_for_csv` with `reject_format_mismatched_options`, holding both the compression rule and the timeout rule.
- **Alternatives:** Add a second guard `reject_timeout_for_parquet` alongside the existing one and call both from `run`.
- **Rationale:** The mapping from export option to compatible format is one design decision and belongs in one place. The existing doc comment explains why validation runs ahead of export-source resolution; that rationale applies identically to the timeout rule and would otherwise be duplicated or silently dropped. Cost is renaming two existing unit tests and updating one call inside a third, with all three assertions unchanged.
- **Promotes to ADR:** no

### [5] Place the Parquet rejection scenario in `export/csv-export`

- **Decision:** Spec the "Timeout option rejected for Parquet format" scenario under `export/csv-export`, and the `--timeout 0` and help-text scenarios under `cli/export-command-structure`.
- **Alternatives:** Put the rejection under `export/parquet-export`, since Parquet is the format named in the failing command.
- **Rationale:** Mirrors the existing split. `export/parquet-export` owns `--compression` and therefore holds "Compression option rejected for CSV format"; `cli/export-command-structure` holds the parse-level "Invalid compression value rejected". `--timeout` is a CSV option, so its cross-format rejection belongs with CSV export. Note for recording: `export/csv-export` already holds 20 scenarios and will trip the recorder's >10 organization threshold, which is a pre-existing condition rather than a consequence of this plan.
- **Promotes to ADR:** no

### [6] Do not spec a separate scenario for timer arming on the split path

- **Decision:** No scenario asserts that `--timeout` arms a deadline on a split export driven by `--max-rows-per-file` or `--max-file-size`. A separate scenario does cover what a timed-out split export leaves on disk.
- **Alternatives:** Add an integration scenario asserting that the deadline fires on the split path.
- **Rationale:** The claim holds for timer arming only. `run` builds `CsvExportOptions` once and hands the same value to both `export_csv_to_file` and `export_csv_to_stream`, and both funnel into exarrow-rs `export_to_callback`, which holds the timer, so no code path can arm the deadline on one and skip it on the other. What the two paths leave behind does differ: the split path propagates the timeout with `?` before `split_writer.finish()` and `rename_single_split` run, so its partial files carry different names and an unflushed tail. That difference is specified by "Split CSV export exceeding its timeout fails".
- **Promotes to ADR:** no

### [7] Accept the loss of the implicit bound on Parquet and Arrow exports

- **Decision:** Ship the 0.16.0 bump without restoring a 300-second default for Parquet and Arrow exports, and record the change in the CHANGELOG and `docs/file_exchange.md`.
- **Alternatives:** Hold the bump until exarrow-rs exposes a Parquet timeout. Add an exapump-side timer for the Parquet path only.
- **Rationale:** The 300-second bound was never documented and never chosen by a user; a Parquet export of a large table hits it for the same wrong reason a CSV export does. Restoring it for one format only would leave two formats with opposite defaults for no stated reason. `?query_timeout=<seconds>` in the DSN gives a server-enforced bound that covers every format.
- **Promotes to ADR:** yes

### [8] Carry the revised Background text into the permanent specs at record time

- **Decision:** Both delta files restate the feature description and Background with the `--timeout` facts added. plan.md § Recording Notes instructs the recorder to copy that text into the permanent specs verbatim before archiving.
- **Alternatives:** Leave the delta Background identical to the permanent text and let the flag appear only in scenarios.
- **Rationale:** `speq plan validate` requires a description and a Background in every delta file, but the documented merge procedure only maps the scenario markers. Without an explicit instruction the two permanent Backgrounds would keep describing `--compression` as the only format-specific option, and a later audit would read the omission as spec drift.
- **Promotes to ADR:** no

### [9] Delete partial output when the export deadline elapses

- **Decision:** When `--timeout` elapses, exapump removes the output files that export wrote and names them on stderr, then returns the timeout error. It removes only paths it opened during that run.
- **Alternatives:** Leave the partial file in place and warn on stderr.
- **Rationale:** `File::create` truncates the target before any transport work runs, so a partial file holds no recoverable data — whatever the path held is already gone by the time the deadline fires. What remains is a CSV with a valid header and a truncated body, which a downstream loader reads as a complete export without any signal that it is not. A warning on stderr does not reach a cron job that checks only the exit code and globs the output directory.
- **Promotes to ADR:** yes

## Review Findings

### [1] [plan-review] Background revisions had no reader at record time

- **Finding:** Decision [8] mitigated Background drift by asserting the recorder applies the revised description and Background alongside the scenario merge. No such step exists. `/speq:spec-merge` maps three scenario markers and nothing else, and `recorder-agent` reads `decision-log.md` only for ADR-promoted entries — which [8] is not. The revised text in both delta files was dead, so recording would have left both permanent Backgrounds naming `--compression` as the only format-specific option while their scenarios described `--timeout`.
- **Direction change:** Added a `## Recording Notes` section to plan.md, which `/speq:spec-merge` § Load Plan Context reads, instructing the recorder to replace the `# Feature:` description line and `## Background` paragraph of both permanent specs with the delta text verbatim. Rewrote decision [8]'s Decision bullet to point at that section instead of asserting automatic behavior.
- **Promotes to ADR:** no

### [2] [plan-review] Timeout scenarios said nothing about the partial output file

- **Finding:** "CSV export exceeding its timeout fails" asserted an exit code and a stderr message and left the fate of `--output` unspecified, so two implementers could ship opposite behavior and both pass. The gap bites hardest on the split path: `export_csv` propagates the timeout with `?`, so `split_writer.finish()` and `rename_single_split` never run and `<stem>_000.csv` survives, unrenamed, with its tail bytes lost in an unflushed buffer. A downstream job reading that file sees a valid header and treats a truncated export as complete.
- **Direction change:** Chose deletion over a warning and made it normative. Added `*AND*` clauses to the existing scenario requiring the partial `data.csv` to be deleted and named on stderr, added a `Split CSV export exceeding its timeout fails` scenario asserting the same post-condition across every `data_NNN.csv`, and added task 2.4 implementing the cleanup behind an `ExportError::Timeout` match plus a new `SplitCsvWriter::discard` method. Deletion destroys nothing recoverable, within a bound round 2 made explicit: exapump removes only the paths it opened during that run, and `File::create` has already truncated each of them. Rewrote decision [6] to scope its structural-impossibility claim to timer arming, which is the only thing the two paths genuinely share.
- **Promotes to ADR:** no

### [3] [plan-review] Decision [7]'s accepted risk was only half-mitigated

- **Finding:** Decision [7], the plan's ADR-promoted acceptance of Parquet and Arrow exports losing their implicit 300-second bound, named the CHANGELOG and `docs/file_exchange.md` as its mitigation. Task 3.2 delivered the CHANGELOG half; task 3.1 specified a `--timeout` table row, a client-versus-server paragraph, and an example, none of which mention the removed bound. The reference page a Parquet user consults would have kept describing the old world.
- **Direction change:** Extended task 3.1 to require a paragraph in the Export section of `docs/file_exchange.md` stating that Parquet and Arrow exports have no client-side deadline and cannot take one, that exapump before 0.12.0 bounded them implicitly at 300 seconds, and that `?query_timeout=<seconds>` is the only bound available for those formats.
- **Promotes to ADR:** no

### [4] [plan-review] Split cleanup deleted by existence rather than authorship

- **Finding:** Task 2.4 specified the split cleanup as `created_paths`, returning `split_path(&self.base_path, i)` for `i` in `0..=self.file_index` filtered to paths that exist. Existence is not authorship, and `SplitCsvWriter` opens files lazily from `flush_line`. A deadline elapsing before the first row means no file was opened while `file_index` is still `0`, so a `data_000.csv` left by an earlier run gets deleted by an invocation that never touched it. In the other direction, a run reaching `file_index == 2` leaves an earlier run's `data_003.csv` and beyond in place, so a downstream glob reads a fresh prefix and a stale tail as one export — the exact defect the cleanup was added to prevent.
- **Direction change:** Replaced the accessor with `SplitCsvWriter::discard(&mut self) -> Vec<PathBuf>`, which removes only files the writer opened and returns their paths for the stderr line. Task 2.4 now specifies the tracking: a `created: Vec<PathBuf>` field, pushed inside `open_next_file` immediately after `File::create(&path)?` succeeds, with `discard` dropping `current_file` before removing each recorded path. The existence filter and the `0..=file_index` range are gone. Unit tests now cover zero, one, and three files opened, and assert that a pre-existing `data_000.csv` the writer never opened survives.
- **Promotes to ADR:** no

### [5] [plan-review] Split scenario demanded a file the split path never owns

- **Finding:** The split scenario's clause "the command MUST NOT leave a file at `data.csv`" conflicted with the same delta's Background, which scopes deletion to files the export had written. A split export never writes `data.csv`: `SplitCsvWriter` creates only `split_path(base, i)`, and `rename_single_split` runs after `finish()`, which the timeout path skips. No task implemented the clause, and the only literal implementation — `fs::remove_file(base_path)` on the split path — deletes a pre-existing user file the command never opened. Task 2.6 and the manual-testing row both pushed toward that reading, and neither test distinguishes it from the safe one in a clean temp directory.
- **Direction change:** Replaced the clause with "the command MUST NOT create or remove a file at `data.csv`" and added "stderr MUST name every deleted file" so the scenario asserts the cleanup positively. Task 2.6 now requires the split test to create a `data.csv` before the export and assert it is present and unmodified afterwards. The manual-testing row expects no `/tmp/big_000.csv` on disk and states that `/tmp/big.csv` was never created.
- **Promotes to ADR:** no

### [6] [plan-review] Deletion was undisclosed to users and to the permanent record

- **Finding:** Deleting a user's output file is the most destructive behavior in this plan and appeared in no ADR-promoted decision, no § Impact paragraph, no docs task, and no CHANGELOG item. The delete-versus-warn choice lived only in Review Finding [2], which does not reach `specs/_decision/`. Decision [7] gave a less consequential change — Parquet losing an undocumented bound — an ADR, an Impact paragraph, a docs paragraph, and a CHANGELOG line.
- **Direction change:** Added design decision [9], marked for ADR promotion, recording the choice and its rationale. Added an § Impact sentence stating that a timed-out CSV export deletes the output files it wrote and names them on stderr, where earlier versions left the partial file. Extended task 3.1 to state the deletion in the `--timeout` paragraph of `docs/file_exchange.md`, and task 3.2 to carry it as a fifth CHANGELOG item.
- **Promotes to ADR:** no

### [7] [plan-review] The stderr-naming clause added by the R2-2 fix had no implementing assertion

- **Finding:** The R2-2 fix added "stderr MUST name every deleted file" to the split scenario, and the single-file scenario already carried the equivalent clause, but task 2.6 asserted only the exit code, the timeout message, and the absence of the output files. A normative MUST with no implementing assertion is the defect class round-1 B3 identified. The gap had teeth on the split path: `SplitCsvWriter` opens `data_000.csv` only when the first complete line reaches `flush_line`, so a deadline elapsing during SQL execution leaves no split file to find, and `export_split_exceeding_timeout_fails` would pass with task 2.4 entirely unimplemented.
- **Direction change:** Task 2.6 now requires each timeout test to assert that stderr names the removed file — `data.csv` in `export_exceeding_timeout_fails`, at least one `data_NNN.csv` in `export_split_exceeding_timeout_fails`. Absence of a file no longer counts as evidence of cleanup; the test fails when the cleanup never ran. No spec delta, task number, or parallel group changed.
- **Promotes to ADR:** no

<!--
Entries [8]-[13] fold the six actionable plan-review advisories into the plan at the maintainer's direction,
deviating knowingly from the plan-review skill's report-only rule. Rationale: /speq:implement and both
implementer agents read plan.md, open-questions.md, and the tasks.md materialized from the plan — nothing in
that flow reads review/round-*.md, so an advisory left in a review file never reaches the implementer.
The two prose advisories were excluded as cosmetic.
-->

### [8] [plan-review] Task 2.2 miscounted the tests it renames

- **Finding:** Task 2.2 and decision [4] both claimed four existing `reject_compression_for_csv_*` unit tests. The source carries two (`src/commands/export.rs:417,429`) plus a third test that calls the same function without carrying the prefix, `compression_with_csv_is_rejected_before_the_missing_source_error` (`:438`, calling at `:445`). An implementer hunting a fourth would either invent one or assume the source had drifted.
- **Direction change:** Task 2.2 now names the two tests to rename, the third call site to update, and states that all three assertions stay unchanged. Corrected the same claim in decision [4]. Line numbers verified against the working tree before writing.
- **Promotes to ADR:** no

### [9] [plan-review] Manual-testing row predicted clap's exit code, not exapump's

- **Finding:** The `--timeout 0` row expected exit 2, clap's default for a parse failure. exapump overrides it: `src/main.rs:21-28` wraps `Cli::try_parse()` in `unwrap_or_else` and calls `std::process::exit(1)` at `:24` whenever `e.use_stderr()` holds, which a `value_parser` range violation sets. A tester following the plan would have logged a real pass as a failure.
- **Direction change:** Changed the expectation to exit 1. The advisory cited `src/main.rs:22-29`; the block is `:21-28`, and the plan records the verified numbers.
- **Promotes to ADR:** no

### [10] [plan-review] Group C claimed parallelism between two tasks editing one file

- **Finding:** Group C paired tasks 2.2 and 2.3 as concurrent. Both edit `src/commands/export.rs` and both add to its `#[cfg(test)] mod tests` — 2.2 rewrites the guard at `:87` and the tests at `:417-450`, 2.3 rewrites `build_csv_options` at `:94-105`. Two implementers running them together would conflict or clobber.
- **Direction change:** Split into Group C = {2.2} and Group C2 = {2.3}, with `Group C → Group C2 (both edit src/commands/export.rs and its test module)` added to the dependency chain and `Group C2 → Group D` replacing the former `Group C → Group D`. Added a line recording that no group now holds two tasks touching one file, and naming Group E's four disjoint targets so the claim is checkable rather than asserted.
- **Promotes to ADR:** no

### [11] [plan-review] The overflow guard was a runtime error path with no scenario and no test

- **Finding:** `range(1..)` admitted values to `u64::MAX`, so task 2.3 needed a `checked_mul` and an `anyhow` error for the seconds-to-milliseconds product. That error path had no spec scenario and no test, and it declined at runtime a decision the parser could settle at the boundary.
- **Direction change:** Tightened task 2.1's parser to `range(1..=18_446_744_073_709_551)`, which is `u64::MAX / 1000`, and deleted the checked-arithmetic sentence from task 2.3. Each task now states why the other makes it safe. Updated the matching expression in plan.md § Design Patterns and decision [3].
- **Promotes to ADR:** no

### [12] [plan-review] The cleanup hung on a runtime downcast where the type is statically known

- **Finding:** Task 2.4 routed cleanup through `err.downcast_ref::<exarrow_rs::ExportError>()`, but both call sites return `Result<u64, ExportError>` concretely and the anyhow conversion is exapump's own `?` one line earlier. A downcast that stops matching — an upstream release wrapping the error, or an added `.context()` — returns `None`, silently disabling a deletion two scenarios declare as MUST, with no compile error and no failure outside the two Docker-gated tests.
- **Direction change:** Task 2.4 now binds the concrete `Result<u64, exarrow_rs::ExportError>` before `?` and matches the `Err` arm on `ExportError::Timeout { .. }` directly, so an upstream shape change breaks the build instead of the guarantee. Updated the stale "downcast" reference in Review Finding [2].
- **Promotes to ADR:** no

### [13] [plan-review] The docs paragraph named two of three reachable timeouts

- **Finding:** After this change a user faces three timeouts and task 3.1 contrasted two. exarrow-rs parses `timeout`/`connection_timeout` (connect deadline), `query_timeout` (server-enforced query bound), and `idle_timeout` from the DSN. A reader told only that the server-enforced bound lives in the DSN will plausibly reach for `?timeout=`, which sets the connect deadline and does nothing for a long export. Task 3.1 also gave one description string for a three-column table without saying what fills `Default`.
- **Direction change:** Task 3.1 now requires the paragraph to name all three — `--timeout`, `?query_timeout=<seconds>`, and `?timeout=<seconds>` with its scope stated as connect-only — and specifies the table row cell by cell, including the em dash for `Default`.
- **Promotes to ADR:** no
