# Plan Review Findings: add-export-timeout-option (round 1)

## Summary
- Axes checked: 6/6
- Total findings: 11 (Blockers: 3, Advisory: 8)
- Intent Fidelity blockers: 0

## Premortem

Three failure stories drove this review.

1. **The permanent specs forget `--timeout` exists.** The plan ships, `/speq:record` runs, and the marker-only merge carries the five new scenarios but not the revised Background text. Six months later `specs/cli/export-command-structure/spec.md` still reads "The `--compression` option is only valid with `--format parquet`" as the sole format rule. A later planner reads that Background, sees no timeout rule, and either re-adds `--timeout` for Parquet or deletes the guard branch as unspecified. → B1.
2. **A nightly Parquet export hangs forever.** A user upgrades for the CSV fix. Their scheduled `--format parquet` job previously died loudly at 300 s on a wedged tunnel; now it blocks until the scheduler kills it. They open `docs/file_exchange.md`, the reference page for export options, and find nothing about the removed bound — only a CHANGELOG line that scrolled away two releases ago. → B2.
3. **A cron job loads a truncated CSV.** A `--timeout 900` export elapses at 900 s. exapump exits non-zero, but `data.csv` (or `data_000.csv`, `data_001.csv` on the split path) sits on disk holding a partial result with a valid header. The downstream `LOAD` step reads it as complete. Nothing in the plan says whether that file survives. → B3.

## Intent Fidelity

[no objection — axis checked: issue #38's ask (`--timeout <seconds>` on `exapump export`, used to set the export timeout) is delivered by tasks 2.1/2.3; all four interview answers are operationalized and their rejected alternatives are named in plan.md § Non-Goals and decision-log [1]/[3]/[7]; the exarrow-rs bump is load-bearing rather than creep — verified that 0.15.1 `CsvExportOptions::timeout_ms` is `u64` with a `300_000` default (`~/.cargo/registry/src/*/exarrow-rs-0.15.1/src/export/csv.rs:103,125`), so interview answer Q3 ("no limit by default") is unreachable without 0.16.0; no follow-up Parquet ticket is filed, matching the user's explicit rejection]

## Feasibility

#### [UNSTATED_ASSUMPTION] BLOCKER
- Location: decision-log.md § Design Decisions [8]; plan.md § Features
- Issue: decision [8] states "the recorder applies those revisions to the permanent specs alongside the scenario merge". No such step exists. `/speq:spec-merge` § Apply Deltas maps exactly three markers (`DELTA:NEW` → append scenario, `DELTA:CHANGED` → replace scenario with same name, `DELTA:REMOVED` → delete scenario) and nothing else; `recorder-agent.md` describes recording as "deterministic file surgery" and reads `decision-log.md` only for entries marked `Promotes to ADR: yes` — decision [8] is marked `Promotes to ADR: no`, so the note has no reader. The revised feature-description lines and Background paragraphs in both delta files are therefore dead text. After recording, `specs/cli/export-command-structure/spec.md:3,7` and `specs/export/csv-export/spec.md:3,7` keep their current wording, which never mentions `--timeout`. The plan identified this risk and then mitigated it with a mechanism that does not fire.
- Fix: Add a `## Recording Notes` section to plan.md — `/speq:spec-merge` § Load Plan Context reads `plan.md`, so instructions placed there reach the recorder. State in it: "Before archiving, replace the `# Feature:` description line and the `## Background` paragraph of `specs/cli/export-command-structure/spec.md` and `specs/export/csv-export/spec.md` with the corresponding text from this plan's delta files verbatim." Then rewrite decision-log [8]'s Decision bullet to point at that section instead of asserting the recorder does it automatically.

#### [UNSTATED_ASSUMPTION] ADVISORY
- Location: plan.md § Dependencies, para 2; task 1.1
- Issue: "exarrow-rs 0.16.0 lists six breaking changes" is wrong. The 0.16.0 CHANGELOG lists six *entries*: four `Breaking:` and two `Fix:`. The four surfaces the plan then names (`timeout_ms`, `ExportError::Timeout`, `TransportProtocol`, `Connection::is_closed()`) are exactly the four breaking ones — verified absent from exapump (`grep -rn "timeout_ms\|ExportError\|TransportProtocol\|is_closed" src/ tests/` returns nothing), so the compile-clean conclusion holds. The two `Fix:` entries are never assessed, and one is directly relevant: "an elapsed explicit export timeout no longer leaves the connection open with an unread EXPORT response… the driver now terminates the transport when the timeout elapses before the EXPORT response was read." That is the exact code path task 2.3 makes reachable for the first time, and task 1.1 sends the implementer looking for two breaking changes that do not exist.
- Fix: In plan.md § Dependencies, replace "lists six breaking changes" with "lists four breaking changes and two fixes", and add one sentence recording that the transport-termination fix governs connection state after `--timeout` elapses. Rewrite task 1.1's second sentence to "confirm each of the four breaking changes leaves exapump's sources untouched".

#### [UNSTATED_ASSUMPTION] ADVISORY
- Location: plan.md § Manual Testing, row 2; task 2.2 (test-count claim)
- Issue: two claims contradict the current source. (a) The manual-testing row for `--timeout 0` expects "Exit 2". exapump overrides clap's exit code: `src/main.rs:22-29` calls `Cli::try_parse().unwrap_or_else(|e| { if e.use_stderr() { e.print(); std::process::exit(1) } … })`, and a `value_parser` range violation sets `use_stderr() == true`, so the shell sees **1**. (b) Task 2.2 says "Rename the four existing `reject_compression_for_csv_*` unit tests". Only two carry that prefix (`src/commands/export.rs:417,429`); a third caller, `compression_with_csv_is_rejected_before_the_missing_source_error` (`:438`), does not. There is no fourth.
- Fix: In plan.md § Manual Testing, change the `--timeout 0` row's expected output from "Exit 2" to "Exit 1". In task 2.2, replace "Rename the four existing `reject_compression_for_csv_*` unit tests to the new function name" with "Rename the two `reject_compression_for_csv_*` unit tests (`src/commands/export.rs:417,429`) to the new function name and update the call inside `compression_with_csv_is_rejected_before_the_missing_source_error` (`:438`), keeping all three assertions unchanged", and make the same correction to decision-log [4]'s closing "Cost is renaming four existing unit tests" sentence.

#### [EFFORT_MISESTIMATION] ADVISORY
- Location: plan.md § Implementation Tasks, task 2.5
- Issue: task 2.5 hides three jobs behind one checkbox and specifies two of them loosely. First, "verify against the running Docker database that the untimed export takes well over 1 second, and raise the row count until it does" is a hand-calibration loop against one machine; the row count that survives is never written back into the plan, so nothing pins what CI runs. Second, "Confirm the aborted export leaves the database usable by running the schema teardown afterwards" is not implementable as written — the suggested query is `SELECT … FROM DUAL CONNECT BY …`, which touches no schema, and every `fixtures::exapump()` invocation is a separate process whose connection dies with it, so a subsequent teardown proves nothing about the terminated transport. Third, the whole instruction is one 73-word sentence.
- Fix: Split task 2.5 into 2.5a (`export_with_generous_timeout_succeeds`, untagged) and 2.5b (`export_exceeding_timeout_fails`, `[expert]`). In 2.5b, pin the row generator in the plan text with a fixed bound (state the exact `CONNECT BY LEVEL <= N` value the implementer must commit) and require the test to assert both a non-zero exit and `timed out after 1000ms` on stderr. Replace the "database usable" sentence with a concrete post-condition: "after the timed-out export, a second `exapump sql 'SELECT 1'` invocation MUST exit 0."

#### [NFR_IGNORED] ADVISORY
- Location: plan.md § Impact, para 1
- Issue: paragraph 2 discloses that a Parquet export "now waits indefinitely unless the DSN carries `?query_timeout=<seconds>`". Paragraph 1 describes the identical new default for CSV as "runs until Exasol finishes the EXPORT statement", which understates it. exarrow-rs 0.16.0 `src/export/csv.rs:428-431` states the HTTP tunnel read "has no timeout of its own", and the `None` arm at `:590` awaits the work future with no bound at all. A CSV export against a stalled tunnel hangs with no client-side escape, and that is now the default for the format this feature targets. The asymmetric disclosure reads as though CSV is protected and only Parquet is exposed.
- Fix: Extend plan.md § Impact paragraph 1 with one sentence: "Without `--timeout`, a CSV export against a stalled HTTP tunnel also waits indefinitely; `--timeout` or `?query_timeout=<seconds>` is the only bound."

## Requirement Quality

#### [COMPLETENESS_GAP] BLOCKER
- Location: `specs/_plans/add-export-timeout-option/export/csv-export/spec.md` § Scenario: CSV export exceeding its timeout fails; decision-log.md [6]
- Issue: the scenario asserts a non-zero exit and a stderr message and says nothing about `--output`, the artifact the command exists to produce. Two implementers would ship opposite behavior and both would pass. The gap is concrete on the split path: `src/commands/export.rs:141-155` propagates the timeout with `?` from `conn.export_csv_to_stream(...)`, so `split_writer.finish()` (which flushes `line_buffer` and the current `BufWriter`, `src/split.rs:106-113`) and `rename_single_split` (`src/split.rs:33-39`) never run. A timed-out split export therefore leaves `<stem>_000.csv` on disk — never renamed to `<stem>.csv` even when it is the only file — with its tail bytes lost in an unflushed buffer. The single-file path likewise leaves a partial file written by exarrow-rs `File::create`. Decision [6] declines a split-specific scenario on the grounds that "no code path can honor the timeout on one and skip it on the other, so the test would assert a structural impossibility." That rationale is correct about *whether the timer fires* and silent about *what the two paths leave behind*, which differ.
- Fix: Add one `*AND*` clause to the "CSV export exceeding its timeout fails" scenario stating the required output-file behavior (either "the CLI MUST delete any partially written output file before exiting" or "the CLI MUST leave the partially written output file in place and stderr MUST warn that it is incomplete" — choose one and make it normative). Add a `DELTA:NEW` scenario "Split CSV export exceeding its timeout fails" to the same delta file asserting the same post-condition for `--max-rows-per-file`. Rewrite decision-log [6]'s Rationale to scope its "structural impossibility" claim to timer arming, and add a task implementing the chosen cleanup in `export_csv`.

#### [COMPLETENESS_GAP] ADVISORY
- Location: plan.md § Implementation Tasks, task 2.3
- Issue: `clap::value_parser!(u64).range(1..)` admits values up to `u64::MAX`, so task 2.3 must add a `checked_mul` and "surface an overflow as an `anyhow` error". That error path has no spec scenario and no test — task 2.3's two named tests cover the default and the happy-path conversion only. It is also avoidable: tightening the parse-time range removes the runtime branch entirely, which is the better answer under `/speq:design-philosophy` (make the decision at the boundary rather than declining it into a runtime error).
- Fix: Change task 2.1's parser to `clap::value_parser!(u64).range(1..=18_446_744_073_709_551)` so the seconds→milliseconds product cannot overflow, and delete the `checked_mul`/`anyhow` overflow sentence from task 2.3. If the runtime error is kept instead, add a third unit test `build_csv_options_rejects_timeout_overflow` to task 2.3 and a `DELTA:NEW` scenario for it to the csv-export delta.

#### [COMPLETENESS_GAP] ADVISORY
- Location: plan.md § Implementation Tasks, task 3.1
- Issue: after this change a user faces three distinct timeouts and the planned documentation names two. exarrow-rs `src/connection/params.rs:514,523,532` parses `timeout`/`connection_timeout` (connect deadline), `query_timeout` (server-enforced query bound), and `idle_timeout` from the DSN. Task 3.1's paragraph contrasts `--timeout` with `?query_timeout=` only. A reader told "the server-enforced bound lives in the DSN" will plausibly reach for `?timeout=`, which sets the connect deadline and silently does nothing for a long export. Task 3.1 also supplies a single description string for a three-column table (`Flag | Default | Description`, `docs/file_exchange.md:50`) without saying what goes in `Default`.
- Fix: In task 3.1, extend the required paragraph to name all three: `--timeout` (client-side export deadline, CSV only), `?query_timeout=<seconds>` (server-enforced query bound, all formats), and `?timeout=<seconds>` (connect deadline only, not an export bound). Specify the table row explicitly as `| `--timeout` | — | Client-side export deadline in seconds; CSV only |`.

## Task Breakdown

#### [TRACEABILITY_GAP] BLOCKER
- Location: plan.md § Implementation Tasks, task 3.1; decision-log.md [7]
- Issue: decision [7], the plan's only ADR-promoted risk acceptance, states the mitigation as "record the change in the CHANGELOG **and** `docs/file_exchange.md`". Task 3.2 delivers the CHANGELOG half. Task 3.1 does not deliver the other half — it specifies a `--timeout` table row, a client-vs-server paragraph, and an example, with no mention that Parquet and Arrow exports lose their implicit 300-second bound. The behavior change therefore reaches users only through a CHANGELOG entry, while `docs/file_exchange.md` — the page a Parquet user consults for export options — keeps describing the old world. An accepted risk whose stated mitigation is half-unimplemented is an unaccepted risk.
- Fix: Add to task 3.1: "Add a paragraph to the Export section of `docs/file_exchange.md` stating that Parquet and Arrow exports have no client-side deadline and cannot take one, that exapump versions before 0.12.0 bounded them implicitly at 300 seconds, and that `?query_timeout=<seconds>` in the DSN is the only bound available for those formats."

#### [TASK_GRANULARITY] ADVISORY
- Location: plan.md § Parallelization, Group C
- Issue: Group C pairs tasks 2.2 and 2.3 as parallel. Both edit `src/commands/export.rs`, and both edit its `#[cfg(test)] mod tests`: 2.2 rewrites the guard at `:87` plus tests at `:417-450`, 2.3 rewrites `build_csv_options` at `:90-101` plus adds tests to the same module. Concurrent implementers working the same file will conflict or clobber each other's edits. Group D's four tasks are genuinely disjoint (`tests/cli_test.rs`, `tests/export_test.rs`, `docs/file_exchange.md`, `Cargo.toml`+`CHANGELOG.md`) — Group C is the only false claim.
- Fix: In plan.md § Parallelization, split Group C into two sequential groups — Group C = {2.2}, Group C2 = {2.3} — and add `Group C → Group C2 (both edit src/commands/export.rs and its test module)` to the sequential-dependencies list, renaming the current Group D to Group E with `Group C2 → Group E`.

## Design Depth

[no objection — axis checked. The `reject_compression_for_csv` → `reject_format_mismatched_options` merge (decision [4]) was assessed specifically for scope creep and is justified, not creep: "which export option belongs to which format" is one design decision, and the existing doc comment at `src/commands/export.rs:82-86` carries the "validate before resolving the export source" ordering rationale that would have to be duplicated into a second parallel guard. Regression risk is bounded — the compression branch and its error text move verbatim and all three existing assertions are preserved. The merged function extends by adding a case inside its owner rather than by editing a dispatch. The pass-through builder in `build_csv_options` leaks no timer ownership: exarrow-rs holds the `tokio::time::timeout` (`src/export/csv.rs:585-605`) and exapump performs one unit conversion, so there is exactly one owner of the deadline decision. No new module or boundary is introduced.]

## Prose Quality

#### [PROSE_BLOAT] ADVISORY
- Location: plan.md § Summary (line 5), § Design Goals and Non-Goals (lines 17-18); decision-log.md [7]-[8] Rationale
- Issue: five governed sentences exceed the 25-word cap. plan.md § Summary sentence 1 runs 30 words; the Goals bullet runs 36; the Non-Goals bullet runs 30; decision-log [7]'s Rationale runs 32 and [8]'s Decision bullet runs 30. The Summary two-sentence cap is met and BLUF holds throughout, so this is length only.
- Fix: Split plan.md § Summary sentence 1 at "and bump exarrow-rs to 0.16.0" into two sentences. Convert the Goals and Non-Goals semicolon lists into nested bullets, one item per line. Split decision-log [7]'s Rationale at the semicolon and [8]'s Decision bullet at "and".
