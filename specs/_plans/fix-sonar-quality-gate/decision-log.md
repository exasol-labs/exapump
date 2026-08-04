# Decision Log: fix-sonar-quality-gate

## Interview

Interview mode was headless. No live interview ran. The orchestrator passed a fixed brief from a human operator, reproduced here as the questions it answers.

**Q:** What is the goal?
**A:** Raise test coverage and get the SonarCloud quality gate green for exapump. Not a feature change.

**Q:** Which spec surfaces may this plan touch?
**A:** None. Non-negotiable: nothing under `specs/<domain>/**/spec.md` and nothing in `specs/mission.md`. Quality and test work, not a behavior change.

**Q:** What is in scope?
**A:** Two things. Add tests to raise coverage where SonarCloud flags it low. Fix the specific reliability and maintainability issues the quality gate is failing on.

**Q:** How are the concrete gate failures determined?
**A:** The planner determines them as part of planning research: the SonarCloud project `exasol-labs_exapump` status or the CI run output, or failing direct access, `.github/workflows/ci.yml` plus a locally runnable equivalent. Document what was found and how.

**Q:** Is this a regression?
**A:** No. PR #36 (commit `6073f22`) added SonarCloud analysis to CI. The gate is newly wired, so this is the first plan addressing it.

## Research Method

Recorded because the brief asked for it. The repository `exasol-labs/exapump` is public (`gh api repos/exasol-labs/exapump --jq .private` returned `false`), so the SonarCloud public API answered every question without a token.

| Question | Command | Finding |
|----------|---------|---------|
| Which CI job failed | `gh run view 30800552742` | `Sonar Analysis` failed; `Check`, `Integration Tests` and `Unit Tests` passed |
| Why it failed | `gh api repos/exasol-labs/exapump/actions/jobs/91644470359/logs` | `ERROR QUALITY GATE STATUS: FAILED`, scanner exit code 3. No condition detail in the log |
| Which gate applies | `api/qualitygates/get_by_project` then `api/qualitygates/show?name=Sonar way` | Built-in `Sonar way`, `isCleanAsYouCode: true`, six conditions, all on new code |
| Stored gate status | `api/qualitygates/project_status?projectKey=exasol-labs_exapump&branch=main` | `status: NONE`, `conditions: []`, `periods: []` |
| Why no conditions | `api/measures/search_history` for `new_coverage`, `new_lines`, `new_maintainability_rating`, `quality_gate_details` | One analysis on `main`, every new-code metric empty |
| Analysis history | `api/project_analyses/search` | Exactly one analysis, `projectVersion: "not provided"`, `manualNewCodePeriodBaseline: false` |
| PR behavior | `api/project_pull_requests/list` | PR #36 returned `qualityGateStatus: OK` on the same commit |
| Project measures | `api/measures/component` | coverage 55.7, lines_to_cover 3780, uncovered_lines 1674, ncloc 4964, code_smells 9, duplicated_lines_density 2.1, bugs 0, vulnerabilities 0, hotspots 0, all ratings 1.0 |
| Which issues | `api/issues/search?componentKeys=exasol-labs_exapump&resolved=false` | 9 issues, all `rust:S3776`, total effort 54 minutes |
| Per-file detail | `api/measures/component_tree?qualifiers=FIL` | Uncovered lines and duplication per file; duplication sits in `interactive.rs` 8.9%, `bucketfs.rs` 7.9%, `sql.rs` 3.7% |
| Current coverage command | `cargo llvm-cov --bin exapump --summary-only` | 54.96% lines, reproduces the CI figure |
| Full-suite coverage | `cargo llvm-cov --all-targets --no-fail-fast --summary-only` | 82.46% lines with no Exasol container running |

**Limitation.** No Exasol container was available during planning, so the Exasol-gated tests failed during the full-suite run. 82.46% is therefore a floor, not the true full-suite figure. Task 1.1 records the real baseline against a running container.

## Design Decisions

### [1] Coverage failures are not why the gate is red

- **Decision:** Treat the red CI check as an empty new-code period, and treat low overall coverage as the separate quality problem the operator asked to fix. Address both, in that order.
- **Alternatives:** Assume the gate failed on coverage and plan only test-writing. Rejected because no condition in `Sonar way` measures overall `coverage`; all six measure new code. Writing tests alone would leave the CI check red.
- **Rationale:** The stored status is `NONE` with zero evaluated conditions and every new-code metric empty. `sonar.qualitygate.wait=true` fails on any status other than `OK`. The first analysis of a project with no version history has no baseline to measure against, so nothing was evaluated.
- **ADR rationale:** this records SonarCloud state observed once on 2026-08-03, not a durable architectural choice. The record goes stale the moment the next analysis runs, so it stays in this plan's log rather than becoming a permanent file under `specs/_decision/`.
- **Promotes to ADR:** no

### [2] Measure coverage in the job that has a database

- **Decision:** Run `cargo llvm-cov` inside `integration-tests`, which already starts `exasol/docker-db:2025.2.0`, and delete the separate `unit-tests` job.
- **Alternatives:** (a) Keep `unit-tests` and merge two lcov reports through a comma-separated `sonar.rust.lcov.reportPaths`. Rejected as two runs and two artifacts for one number. (b) Enumerate the Exasol-free test targets in `unit-tests`. Rejected as brittle: this project's tests panic rather than skip when Exasol is absent, per the workspace testing rule, so the list breaks the moment a test file gains a database case.
- **Rationale:** `cargo llvm-cov --bin exapump` measures only the `#[cfg(test)]` modules in `src/`, so the 179 integration tests contribute nothing. Including them lifts measured coverage from 54.96% to at least 82.46% with no new test written. `scripts/check.sh` already runs `cargo test --bin exapump` in the `Check` job, so the unit-test pass/fail signal survives the deletion.
- **Capability lost:** the coverage signal does not survive intact. After this change the repository has no container-free coverage producer, so every coverage number depends on a container start whose readiness step alone reserves `timeout-minutes: 30`, and a container flake leaves `Sonar Analysis` skipped rather than evaluated. Accepted because unit-test-only coverage is the defect being fixed, and `integration-tests` already blocks the merge when the container fails.
- **Reinstatement trigger:** add a container-free coverage job back when either condition holds — `integration-tests` flakes on container start in more than one run out of ten over a calendar month, or a contributor needs a coverage number on a machine that cannot run the Exasol image. The reinstated job publishes a second lcov file and `sonar-project.properties` consumes both through a comma-separated `sonar.rust.lcov.reportPaths`, which is alternative (a) above, deferred rather than rejected outright.
- **ADR rationale:** the durable choice of where coverage is measured. `/speq:record` writes it to `specs/_decision/005-fix-sonar-quality-gate.md`, alongside [4]. Those two are the only decisions in this log that promote; [1] and [8] were demoted in round 1 as one-time observed state.
- **Promotes to ADR:** yes

### [3] Set `sonar.projectVersion` from `Cargo.toml`

- **Decision:** Pass the crate version to the scanner so the new-code period always has a baseline.
- **Alternatives:** Leave it unset and rely on the second analysis resolving a baseline by itself. Rejected as a guess about undocumented behavior on the exact failure mode this plan must fix.
- **Rationale:** `Sonar way` defaults its new-code definition to the previous version, and this project has no version history. `speq-implement-pr` bumps the version on this branch, so the next `main` analysis carries a version different from `not provided`. This is an assumption, not a verified fact, which is why operator step O2 checks the status empirically and names the operator-side fallback.
- **Promotes to ADR:** no

### [4] Extract decision logic instead of suppressing S3776

- **Decision:** Every flagged function gets its branching logic extracted into pure functions that take values and return values. No suppressions, no threshold changes. For `profile.rs` `init`, `edit`, `prompt_bucketfs` and `edit_bucketfs` the chosen design is named **injected prompter**: a private `ProfilePrompter` trait (`text`, `confirm`, `password`, `notice`) that those functions take as `&mut dyn ProfilePrompter`, with a `TerminalPrompter` production implementation and a `ScriptedPrompter` test implementation. The rejected alternative is the **pure-helper** design, which leaves `init` and `edit` prompting for themselves and extracts decision helpers beside them; it was rejected because it covers only the extracted subset while the injected prompter puts the whole flow under test, and the coverage arithmetic in [7] needs the larger yield. Task 2.2 and the § Design/Patterns row both state the injected-prompter design; neither describes the pure-helper design.
- **Alternatives:** Suppress the rule per function, or raise the S3776 threshold in the quality profile. Both rejected: they leave the untestable code untestable and hide the reason coverage is stuck.
- **Rationale:** The two problems share one cause. `profile.rs` is at 0% because `init`, `edit`, `prompt_bucketfs` and `edit_bucketfs` bail on `!stdin().is_terminal()` and then call `inquire` and `rpassword`, so a CLI-driven test stops at the first guard. Those same functions carry three of the nine S3776 issues. Separating the decision from the prompting fixes complexity and coverage in one change, and follows `/speq:design-philosophy`: business logic performs no I/O, and the consumer defines the abstraction it needs.
- **ADR rationale:** a deliberate rejection of the commonly expected approach (suppress the rule or raise the threshold) that future planners need on record to avoid re-litigating. It is not one-time observed state, so the round-1 demotion of [1] and [8] does not apply to it. `/speq:record` writes it to `specs/_decision/005-fix-sonar-quality-gate.md` alongside [2].
- **Promotes to ADR:** yes

### [5] Cover each file before refactoring it

- **Decision:** Pair every refactor with its tests in the same task, and land the corrected coverage pipeline before any refactor.
- **Alternatives:** Refactor everything, then write tests. Rejected.
- **Rationale:** Refactored lines are new code on this PR's own analysis and must reach 80% coverage. `profile.rs` and `export.rs` are at 0% as Sonar measures them today, so a large refactor landing before its tests would push a block of uncovered new lines into the gate and fail it.
- **Promotes to ADR:** no

### [6] Zero spec deltas

- **Decision:** Create no file under `specs/<domain>/`, and do not touch `specs/mission.md`.
- **Alternatives:** Invent a `quality` or `ci` domain to hold coverage and gate requirements. Rejected.
- **Rationale:** The operator fixed this constraint. It also matches the evidence: `speq domain list`, `speq feature list` and `speq search query "test coverage sonar quality gate reliability maintainability"` found no related spec surface, best score 0.71. `specs/_recorded/2026-05-22-change-exarrow-rs-bump-0.12.3` is the precedent for a validated plan whose Features table reads `(none)`. The refactors are behavior-preserving, so the specs stay accurate as written.
- **Promotes to ADR:** no

### [7] Coverage target of 85%

- **Decision:** Require Sonar `coverage` at or above 85% overall, and `src/commands/profile.rs` at or above 70%.
- **Alternatives:** (a) Target the gate's 80% number. Rejected: 80% applies to new code, and wiring the integration suite in already reaches roughly 82% overall, so 80% would be satisfied on arrival and would justify no test writing at all. (b) Target 90% or higher. Rejected: reaching it means covering most of the remaining 684 lines, which are concentrated in interactive prompt flows and error paths that need a database or a TTY.
- **Rationale:** 85% requires roughly 120 lines of real new coverage beyond the pipeline fix. `profile.rs` at 70% supplies about 194 of them on its own, which lands overall coverage near 87% and leaves margin. Task 3.2 caps the work at 85% and forbids assertion-free tests written to move the number.
- **Arithmetic and its status:** the 194-line figure is a derivation, not a measurement. `profile.rs` holds roughly 611 coverable lines and sits at 38.30%, so moving it to 70% covers about `611 * (0.70 - 0.383) = 194` more lines. Overall, `lines_to_cover` is 3780, so 194 lines is about 5.1 points on top of the 82.46% container-free floor. The derivation assumes 194 coverable lines are reachable under decision [4]'s injected-prompter design. Nothing measured that before planning ended.
- **Gate:** task 1.1b measures the real figure per function from the `DA:` and `FN:` records of the lcov export task 1.1 writes, and task 1.1b-ii sums the reachable functions into R. **R is a count of uncovered source lines** — the same unit as the 194 lines it is compared against and as the `(coverable - uncovered + R) / coverable` recomputation formula. It is not a count of coverage regions; `llvm-cov` emits several regions per line, so the two units differ by a large and variable factor. The lcov export is the source because it carries llvm-cov's own line mapping, the mapping Sonar consumes; the JSON export uses a different line rule (611 lines for `profile.rs` against lcov's 604) and the two are never compared. Task 1.1b-ii also excludes from R every line that task 2.2 relocates into `TerminalPrompter`, because those lines stay uncovered wherever they live. Tasks 1.1c and 1.1d carry the gate: Phase 2 may not begin until one of the two is checked off, and 1.1d recomputes the 70% and 85% targets from R and rewrites them in § Requirements and in this entry when R is below 194 lines. Both figures are computed on the task 1.1 baseline denominator, never on one re-measured after this plan's `#[cfg(test)]` module exists. If 70% turns out unreachable, the recomputed figure is the requirement; the plan does not hold a target its own measurement contradicts, and it does not close the gap with assertion-free tests.
- **R correction (task 3.1, secondary check only):** task 1.1b-ii's subtraction list (38 lines) was incomplete. Task 3.1 re-measured the nine-function residual post-refactor, closed every line that was genuinely closeable with real `ScriptedPrompter` error-propagation tests (6 lines, across `prompt_bucketfs`, `edit_bucketfs`, `edit_profile`), and verified against `src/commands/profile.rs` directly that the remaining residual is structural: 32 lines in the `init`/`edit` wrappers (task 2.2's own design leaves them uncovered; task 1.1b-ii's caveat 2 estimated only ~4) plus 19 lines in `TerminalPrompter`'s four methods (`ask_text`, `confirm`, `password`, `notice` — the production I/O implementation `ScriptedPrompter` can never execute; `password`/`notice`, 6 lines, were never on the original subtraction list at all). Verified floor: residual = 51 (down from a pre-task 57). **Corrected R = 349 − 51 = 298** (was 311); corrected residual target = 51 (was 38). Achieved residual sits exactly at 298/51, with no further line closeable under this design without either a TTY-emulation harness (out of scope) or an assertion-free test written only to move the number (forbidden below). This correction touches only the auxiliary uninflatable check — the § Requirements `profile.rs coverage` row (70%) is unaffected and is met at 94.59%, and task 1.1c's `targets stand` finding against 194 is unaffected (298 and 311 both clear 194 by a wide margin). Full reasoning and the source-line verification table are in `implementation-notes.md` under `## Task 3.1`.
- **Promotes to ADR:** no

### [8] Existing tests are the refactor safety net

- **Decision:** No existing test may be edited, weakened or deleted to accommodate a refactor. A refactor that requires touching an assertion has changed behavior and must be reverted.
- **Alternatives:** Allow assertion updates where output formatting is judged equivalent. Rejected.
- **Rationale:** This plan ships no spec delta, so the only proof that behavior is preserved is that the existing suite passes unchanged. `interactive.rs` and `sql.rs` carry the highest-complexity functions and the most exact output assertions in `tests/cli_test.rs`, which is where a silent behavior change would otherwise hide.
- **ADR rationale:** this is a working rule for one refactor whose justification ("this plan ships no spec delta") expires with the plan, not a project-wide constraint. It stays in this plan's log.
- **Promotes to ADR:** no

## Review Findings

<!-- Populated by speq-plan-pr after plan-reviewer resolves a blocker, and by speq-implement after code review. -->

### [plan-review] The 194-line coverage arithmetic was never measured

- **Finding:** round 1, `[UNSTATED_ASSUMPTION]` BLOCKER. The 70% `profile.rs` and 85% overall targets rest on an undocumented claim that 194 coverable lines of I/O-free logic exist inside the prompt functions. No derivation appeared in either artifact and no per-function measurement backed it. If the real figure were 120, both requirements fail with no stated fallback.
- **Direction change:** added task 1.1b, which emits a per-function uncovered-line count for `src/commands/profile.rs` from `cargo llvm-cov --json` and sums the functions the task 2.2 design reaches into a figure R. Added a Phase 1 coverage-feasibility gate that blocks Phase 2 until R is known and requires both targets recomputed from R and rewritten in § Requirements and in [7] when R is below 194. Wrote the derivation into [7] and marked it a derivation, not a measurement. § Requirements now states that the revised figure becomes the requirement when 70% is unreachable, and tasks 3.1 and 3.2 reference the § Requirements figure rather than a hardcoded number.
- **Promotes to ADR:** no

### [plan-review] Two incompatible designs for `profile.rs` `init` and `edit`

- **Finding:** round 1, `[REQUIREMENT_CONFLICT]` BLOCKER. § Design/Patterns described injecting a prompt source so the whole flow runs under test, while task 2.2 described pure functions taking already-collected answers, leaving `init` and `edit` prompting for themselves. The two differ in blast radius and decisively in coverage yield, and an implementer could not tell which to build.
- **Direction change:** chose the injected-prompter design and named it in [4]; recorded the pure-helper design as the rejected alternative and why. Rewrote the Patterns row and task 2.2 to state the same design. Task 2.2 now specifies the trait shape directly — `ProfilePrompter` with `text`, `confirm`, `password` and `notice`, the exact functions that take it, `TerminalPrompter` and `ScriptedPrompter`, and the placement of the `is_terminal` guard and the config load/save in a thin uncovered wrapper — so the `[expert]` implementer does not redesign it. Chosen for coverage yield: the injected design is what makes the [7] arithmetic reachable at all.
- **ADR rationale:** the durable half of this finding lives in [4], which carries its own promotion flag.
- **Promotes to ADR:** no

### [plan-review] The cognitive-complexity requirement had no local verification

- **Finding:** round 1, `[AMBIGUOUS_REQUIREMENT]` BLOCKER. "Every listed function at or below 15" was unverifiable during implementation: nothing in § Dependencies, § Verification or `scripts/check.sh` measures SonarSource cognitive complexity, no local `sonar-scanner` step existed, the threshold 15 was asserted without a source, and no remediation loop covered a post-analysis count above zero.
- **Direction change:** added task 1.7, which reads the `threshold` parameter from `api/rules/show?key=rust:S3776&organization=exasol-labs` and confirms the project's Rust `Sonar way` profile does not override it, then writes it into § Requirements and records the counting method. Planning verified the value is 15 (`defaultValue: "15"`, profile `AZ9q0GTXpguOUvh2Rvhk`, no override) and § Requirements now cites that source. Replaced the Phase 2 preamble with two locally runnable checks per task — a hand count against the recorded method, and a per-file `cargo llvm-cov` figure that must not regress — and added a Cognitive complexity row to § Verification/Checklist. The Sonar issue total is explicitly not a Phase 2 completion criterion; its remediation loop lives in operator step O1.
- **Promotes to ADR:** no

### [plan-review] Group C's parallel lanes were not disjoint

- **Finding:** round 1, `[TASK_GRANULARITY]` BLOCKER. `src/commands/interactive.rs:7` imports `error_hint`, `split_statements`, `write_csv` and `write_json` from `sql.rs`, and tasks 2.3 and 2.4 refactor exactly those in a lane scheduled to run concurrently with the `interactive.rs` lane. Making the extracted renderers unit-testable requires a `write_csv`/`write_json` signature change that sat in task 2.4's lane while task 2.5 needed it. The claim that the four lanes touch disjoint files was false and would produce broken merges.
- **Direction change:** split Group C into sequential C1 (`profile.rs` 2.1/2.2/3.1, `sql.rs` 2.3/2.4, `export.rs` 2.7) and C2 (`interactive.rs` 2.5/2.6), and stated the C1-to-C2 dependency with the import as evidence. Task 2.4 now owns every signature change to `write_csv` and `write_json` and lands the `interactive.rs` call-site updates with it so the tree builds; tasks 2.5 and 2.6 consume `write_csv`, `write_json` and `split_statements` unchanged. Deleted the disjoint-files sentence and replaced it with the narrower true claim about the three C1 lanes.
- **Promotes to ADR:** no

### [plan-review] Tasks 4.2 and 4.3 could not run during `/speq:implement`

- **Finding:** round 1, `[TASK_GRANULARITY]` BLOCKER. Task 4.2 began "After the PR analysis completes" and task 4.3 "After merge", but neither a PR analysis nor a merge exists while `/speq:implement` runs. Both sat in § Implementation Tasks as plain checkboxes, so implementation would complete with them blocked or checked off without evidence. § Parallelization also listed Group D as parallel when 3.2, 4.1, 4.2 and 4.3 are strictly sequential.
- **Direction change:** moved both out of § Implementation Tasks into a new `## Post-Merge Operator Checklist` as operator steps O1 and O2, stated as actions on a live PR and on `main`, with an explicit prohibition on an implementer executing or claiming them. O1 gained the remediation loop for a non-zero `rust:S3776` total; O2 gained the concrete New Code setting to apply. Corrected Group D to 3.2 and 4.1 with a "run them in order" note. § Requirements gained a `Verified` column splitting in-loop criteria (local `cargo llvm-cov` figures, behavior, per-function complexity counts) from operator criteria, and states that the two headline Goals are operator criteria that an implementation report may not claim.
- **Promotes to ADR:** no

### [plan-review] Task 1.1b measured regions, not uncovered lines

- **Finding:** round 2, `[UNSTATED_ASSUMPTION]` BLOCKER. Task 1.1b's `jq` expression filtered function regions on `.[5] == 0`, which is the file id, not the execution count at `.[4]`. Every region of a single-file function has file id 0, so the filter matched all of them and R came back as a total region count. Corrected to `.[4]`, a region count is still not a line count, and 194 and the recomputation formula are both in line units. The gate that exists to test the 194 assumption would have passed unconditionally, leaving round 1's blocker untouched.
- **Direction change:** replaced the `jq` region filter in task 1.1b with a line-level measurement over `files[].segments`, stated as a `python3` snippet, and stated explicitly that R counts uncovered **source lines** in the same unit as 194 and as `(coverable - uncovered + R) / coverable`. Added a reconciliation self-check (whole-file zero-count lines must equal `summary.lines.count - summary.lines.covered`) and named `cargo llvm-cov --html` as the cross-check and acceptable substitute. Added the note that `llvm-cov` function names are v0-mangled and must be matched on the demangled final component, so `edit` is not confused with `edit_bucketfs`. Updated [7] § Gate to name the same unit. Both tuple layouts were re-verified against a probe crate on `cargo-llvm-cov` 0.8.7 (export version 3.0.1) rather than assumed. Applying the advisory at the same location, task 1.1b now excludes from R every line task 2.2 relocates into `TerminalPrompter`, which stays uncovered wherever it lives.
- **Operator input:** the operator reviewed `open-questions.md` and gave blanket approval to apply round 2's own fix directions rather than make a different product decision — "raise coverage on all code; reduce cognitive complexity; make Sonar green". This is a measurement-correctness fix, not a scope change.
- **Promotes to ADR:** no

### [plan-review] The recorded cognitive-complexity counting method undercounts

- **Finding:** round 2, `[AMBIGUOUS_REQUIREMENT]` BLOCKER. Calibrated against two functions SonarCloud already scored, task 1.7's method reproduced `profile::show` at 19 but yielded 17 for `profile::init` against Sonar's 22, because the rule list omitted the `+1` per bare `else`. This hand count is the only in-loop completion gate for all seven Phase 2 tasks, so undercounting lets a task be marked done while the function is still above the threshold — caught only at operator step O1, on a PR that can then no longer merge. Two further deviations pointed the other way and would cause false failures: the nesting increment was applied to `else if` and to `&&`/`||` sequences, which the SonarSource specification excludes. The method was also silent on closures and recursion.
- **Direction change:** rewrote task 1.7's method as five numbered rules — base `+1` including bare `else`, `+1` per recursive call cycle, `+1` per labeled `break`/`continue`, the nesting increment restricted to `if`, `match`, `loop`, `while`, `for` and closure bodies, and the nesting level raised by `if`, `else`, `match`, loops and closure bodies. Added round 2's two worked calibrations (`profile::show` = 19, `profile::init` = 22) as a mandatory self-check the implementer reproduces before counting anything in Phase 2, and stated that a method failing either calibration is wrong and must be corrected first. Repeated the calibration requirement in the Phase 2 preamble, which is where the per-task count is performed.
- **Operator input:** covered by the same blanket approval; round 2 supplied the corrected rule list and both calibration numbers.
- **Promotes to ADR:** no

### [plan-review] The Phase 1 coverage-feasibility gate was prose, not a task

- **Finding:** round 2, `[TRACEABILITY_GAP]` BLOCKER. The gate sat as a bold paragraph between checkbox tasks 1.1b and 1.2. It stated a hard precondition on seven tasks and assigned real work — recompute two percentages, rewrite two § Requirements rows, amend [7] — but carried no checkbox, so `/speq:implement` had nothing to schedule, assign or verify. The gate could be skipped silently, its enforcement clause was unenforceable, and an R below 194 would stall the plan on a rewrite no task authorized.
- **Direction change:** converted the paragraph into two checkbox tasks. Task 1.1c compares R against 194 lines and records `targets stand, R = <n> lines` or `targets revised, R = <n> lines` in the implementation notes. Task 1.1d, conditional on R below 194 lines, recomputes both figures from the stated formulas, rewrites the two § Requirements rows, appends the measured R and both revised figures to [7] § Gate, and records the revision. Added both to § Parallelization Group A after 1.1b as a strictly ordered chain, and added a Group A to Group C1 dependency stating that Phase 2 may not begin until 1.1c or 1.1d is checked off. Tasks 3.1 and 3.2 now name task 1.1d instead of "the Phase 1 gate".
- **Operator input:** covered by the same blanket approval; round 2 specified both task splits.
- **Promotes to ADR:** no

### [plan-review] Task 1.1b's self-check compared two incompatible line rules

- **Finding:** round 3, `[UNSTATED_ASSUMPTION]` BLOCKER. Task 1.1b measured uncovered lines from the JSON export's `files[].segments` and then required that count to equal `summary.lines.count - summary.lines.covered`. Round 3 ran the script verbatim on this repository: it reports 604 zero-count lines for `src/commands/profile.rs` against a JSON summary of 611, and mismatches on 13 of 14 source files. The script was correct — it reproduces llvm-cov's lcov line mapping exactly on all 14 files. The JSON export and the lcov export use different line rules, and the self-check compared one against the other, condemning a correct measurement. The named fallback failed the same way, because `cargo llvm-cov --html` renders the report-side rule.
- **Direction change:** task 1.1b now reads per-line counts from the lcov file at `/tmp/lcov-baseline.info`, bucketing each `DA:<line>,<hits>` record into the function whose `FN:<line>,<name>` start line is the greatest at or below it. This reuses llvm-cov's own line rule instead of reimplementing it. The self-check is now internal to that file: zero-hit `DA` records must equal `LF - LH`, and the `DA` count must equal `LF`. Task 1.1b states that `summary.lines.count` uses a different rule from the lcov export (611 against 604 on `cargo-llvm-cov` 0.8.7) and that the two are never compared. The `--html` report is demoted to an unenforced eyeball check. § Requirements gained a paragraph stating both rows are read on the report-side rule from `cargo llvm-cov --summary-only`, while Sonar computes from the lcov rule about one point apart on `profile.rs`.
- **Correction found while applying the fix:** task 1.1's command was `cargo llvm-cov --lcov --output-path /tmp/lcov-baseline.info --summary-only`. Measured on `cargo-llvm-cov` 0.8.7, `--summary-only` strips every `FN:` and `DA:` record and leaves only `LF`/`LH`, so the file the fix depends on would have carried neither record type. Task 1.1 now runs `cargo llvm-cov --no-report` once and reports from it twice, `report --summary-only` for the table and `report --lcov --output-path` for the full file. This also removes task 1.1b's second container run. The § Verification Checklist coverage command lost the same invalid flag pair.
- **Promotes to ADR:** no

### [plan-review] The coverage targets could be met by test code alone

- **Finding:** round 3, `[AMBIGUOUS_REQUIREMENT]` BLOCKER. `sonar.sources=src` makes a `#[cfg(test)]` module inside `src/` count as covered source, and `src/commands/profile.rs` has none today, so every test line this plan adds is net new inflation. Round 3 quantified it on files that already carry one: `sql.rs` reports 74.9% against 50.9% production, `config.rs` 94.9% against 76.3%. A 400-line test module alone moves `profile.rs` from 38.3% to about 63% and TOTAL to about 84.1% with zero production lines newly covered — roughly 120 of the 194 lines and 1.6 of the 2.5 points the plan demands, free. The plan's own anti-inflation sentence did not survive, because it required only that every figure come from the same tool over the same target set. Task 1.1d's `coverable` was ambiguous between its baseline value and an inflated verification-time value.
- **Direction change:** § Requirements now evaluates the `profile.rs coverage` row over that file's baseline coverable line set alone, with every line this plan's new `#[cfg(test)]` module adds excluded from both numerator and denominator, and evaluates `Overall coverage` the same way against the task 1.1 baseline `lines_to_cover`. Task 1.1d names `coverable`, `uncovered` and `lines_to_cover` as the task 1.1 baseline values and states the achieved figure is computed on that same denominator. Tasks 3.1 and 3.2 gained a check that cannot be inflated: re-run the task 1.1b measurement and require the uncovered-line sum over the nine functions R came from to have fallen by at least R, recording the baseline sum and the achieved sum. Test-module lines bucket to their own `FN` records, so they cannot move that sum.
- **Promotes to ADR:** no

### [plan-review] The shared BucketFS port helper would have changed CLI behavior

- **Finding:** round 3, `[REQUIREMENT_CONFLICT]` BLOCKER. Round 2's consolidation fix instructed extracting the port parse-and-validate step "including its retry `notice`", but neither BucketFS function retries. `prompt_bucketfs` (`profile.rs:585-588`) and `edit_bucketfs` (`:853-856`) both hard-fail with `anyhow::bail!("invalid BucketFS port: {}", port_raw)`. The retry-with-`notice` loops live in `inquire_port` (`:504-515`) and `edit`'s own port loop (`:702-711`), neither of which is BucketFS. Building the helper as written converts a hard failure into a re-prompt on `exapump profile init` and `exapump profile edit`, which § Non-Goals and [8] forbid, and which no test under `tests/` catches because both paths need a TTY.
- **Direction change:** deleted "(including its retry `notice`)" from task 2.2. Task 2.2 now states that both BucketFS functions keep their current `anyhow::bail!` with no retry and no `notice`, that the retry loops belong only to `inquire_port` and `edit`'s port loop, and that the shared helper is the parse-and-validate step alone with each caller owning its own failure behavior. This avoids the rejected alternative of one helper carrying a retry-or-bail mode flag.
- **Promotes to ADR:** no

### [plan-review] Round-3 advisory findings applied

- **Finding:** round 3 raised six advisories alongside its three blockers. The operator directed applying them in the same pass, so none would survive into implementation unaddressed.
- **Direction change:** (1) task 1.7's `profile::init` `else` citations corrected from `:417`, `:428`, `:441`, `:472`, `:478` to `:418`, `:429`, `:443`, `:456`, `:477`, with a note that `:456` is an inline `else` inside the `Profile` struct literal that a scan for block-shaped `else` keywords misses. All five were re-read from source. (2) Task 2.2 states that `profile.rs:480`, `:695` and `:804` stay bare `println!` as terminal status output, and that the `make_default` tests assert the `default` field of the returned `Profile` rather than the printed suffix. (3) Task 1.1b is tagged `[expert]` and split: 1.1b produces and self-checks the per-function table, 1.1b-ii subtracts the `TerminalPrompter`-relocated ranges and records R. § Dependencies gained `rustfilt` with `cargo install rustfilt`, confirmed absent on this machine. § Parallelization Group A carries the extended chain. (4) [3] § Rationale now names operator step O2 instead of the deleted task 4.3. (5) Task 2.2 states that label rendering and the `required` empty-check live in exactly one place, a default method on `ProfilePrompter` or a free function both implementations call, so `ScriptedPrompter` cannot diverge from `TerminalPrompter`. (6) The 48-word § Requirements sentence is replaced by sentences under 25 words each.
- **Operator input:** the operator approved applying round 3's fix directions as written and directed skipping a fourth review round, going straight to implementation after this pass. No product decision changed; all three blockers are measurement-correctness or behavior-preservation fixes.
- **Promotes to ADR:** no

### [plan-review] Round-2 advisory findings applied

- **Finding:** round 2 raised six advisories at the same locations as the three blockers. Left unapplied they would resurface in a later review round.
- **Direction change:** (1) task 1.1b excludes the lines task 2.2 relocates into `TerminalPrompter` from R. (2) § Requirements gained a paragraph stating that `sonar.sources=src` makes `#[cfg(test)]` modules inside `src/` count as covered source, that this plan does not exclude them, and that every figure compared against the two coverage rows is read on that same basis. (3) Task 2.2's `notice` enumeration now lists all five messages, adding `"  password cannot be empty"` (`profile.rs:542`) and the `validate_profile_name` passthrough `"  {}"`, whose line was re-verified as `:533` rather than round 2's `:535`. (4) Task 2.2 states that `remove` and `prompt_password_for` do not convert to the trait, being outside the `init`/`edit` flow, and the § Design/Patterns row now reads "every `inquire` and `rpassword` call in the `init` and `edit` flows". (5) The `prompt_bucketfs`/`edit_bucketfs` consolidation is replaced by extracting only the port parse-and-validate step and the blank-to-`None` mapping, keeping the two functions as separate callers owning their prompt strings, defaults and decline behavior, with a requirement that `ScriptedPrompter` tests assert the recorded label sequence for both flows. (6) The test count is corrected to 179 integration tests in § Design/Context and in [2], "lines" is stated at every coverage-figure occurrence, and § Summary is split into two sentences under 25 words each with "82.46% locally measured" and Sonar named as the source of 55.7%.
- **Promotes to ADR:** no
