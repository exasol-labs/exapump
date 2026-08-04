# Plan: fix-sonar-quality-gate

## Summary

Wire the integration test suite into the coverage report Sonar consumes, lifting line coverage from Sonar's 55.7% to 82.46% locally measured. Then refactor the nine `rust:S3776` functions and cover the extracted logic to reach 85% lines; no spec scenario changes, test and code-quality work only.

## Design

### Context

The SonarCloud project `exasol-labs_exapump` was provisioned by PR #36 (commit `6073f22`, merged as `c131a99`). The first main-branch analysis ran on 2026-08-03T09:17:55 and the CI `Sonar Analysis` job failed with exit code 3 and the log line `ERROR QUALITY GATE STATUS: FAILED`. Planning research established the following facts by reading the CI job log and querying the SonarCloud public API. `decision-log.md` records the exact commands.

**The gate is `Sonar way` (built-in, Clean as You Code).** Every one of its six conditions measures new code only:

| Condition | Fails when |
|-----------|-----------|
| `new_coverage` | < 80 |
| `new_duplicated_lines_density` | > 3 |
| `new_maintainability_rating` | worse than A |
| `new_reliability_rating` | worse than A |
| `new_security_rating` | worse than A |
| `new_security_hotspots_reviewed` | < 100 |

**No condition measures overall coverage.** The 55.7% project coverage did not and cannot fail this gate directly.

**The observed failure is an empty new-code period, not a threshold breach.** The stored status for that analysis is `NONE` with zero evaluated conditions, and `new_lines`, `new_coverage`, `new_maintainability_rating` and `quality_gate_details` are all empty. Only one analysis exists on `main`, with `projectVersion` reported as `not provided`. The project has no version history, so the new-code baseline resolved to the first analysis itself and left nothing to measure. `sonar.qualitygate.wait=true` treats any status other than `OK` as a failure, so the scanner exited non-zero. The same commit analysed as PR #36 returned `qualityGateStatus: OK`, because a pull-request analysis always has a well-defined new-code period: the PR diff.

**Coverage measurement excludes the entire integration suite.** The `unit-tests` job runs `cargo llvm-cov --bin exapump`, which executes only the `#[cfg(test)]` modules inside `src/`. The 179 integration tests under `tests/` never contribute. Running the same tool over all test targets locally, with no Exasol container so the Exasol-gated tests fail, already reports 82.46% line coverage against 54.96% for the current command:

| File | `--bin exapump` | all targets | Lines still missed |
|------|-----------------|-------------|--------------------|
| `src/commands/profile.rs` | 0.00% | 38.30% | 377 |
| `src/commands/sql.rs` | 74.56% | 86.94% | 96 |
| `src/commands/interactive.rs` | 64.58% | 81.20% | 69 |
| `src/commands/bucketfs.rs` | 11.72% | 84.83% | 44 |
| `src/commands/export.rs` | 0.00% | 82.93% | 28 |
| `src/commands/wait.rs` | 33.80% | 87.50% | 27 |
| `src/split.rs` | 89.80% | 91.78% | 25 |
| `src/config.rs` | 94.94% | 98.61% | 11 |
| `src/commands/upload.rs` | 0.00% | 96.77% | 3 |
| `src/connection.rs` | 81.41% | 99.36% | 1 |
| `src/main.rs` | 0.00% | 100.00% | 0 |
| **TOTAL** | **54.96%** | **82.46%** | **684** |

**Nine open issues, all `rust:S3776`.** Bugs, vulnerabilities and security hotspots are zero, and every rating is A.

| Function | Location | Cognitive complexity |
|----------|----------|----------------------|
| `execute_statement` | `src/commands/interactive.rs:226` | 66 |
| `split_statements` | `src/commands/sql.rs:80` | 60 |
| `run` | `src/commands/sql.rs:395` | 54 |
| `run` | `src/commands/export.rs:67` | 38 |
| `strip_comments` | `src/commands/sql.rs:13` | 35 |
| `run` | `src/commands/interactive.rs:153` | 26 |
| `init` | `src/commands/profile.rs:370` | 22 |
| `edit` | `src/commands/profile.rs:681` | 22 |
| `show` | `src/commands/profile.rs:220` | 19 |

**`profile.rs` resists CLI-driven testing.** Its 377 uncovered lines are the interactive flows. `init`, `edit`, `prompt_bucketfs` and `edit_bucketfs` guard on `std::io::stdin().is_terminal()` and then call `inquire` and `rpassword`. Tests driving the binary through `assert_cmd` get a non-TTY stdin, so they stop at the first guard. The same functions carry three of the nine complexity issues.

- **Goals** - Sonar `coverage` at or above 85%; zero open `rust:S3776` issues; gate status `OK` on this plan's PR analysis and on the following `main` analysis.
- **Non-Goals** - No change to user-visible CLI behavior, no new features, no spec or mission edits, no change to the gate definition or its thresholds, no attempt to make Exasol-gated tests skip instead of fail.

### Decision

Fix the measurement first, then the code. Three levers, applied in order.

**Lever 1: measure coverage where Exasol already runs.** The `integration-tests` job starts the container and runs the full suite. Replace its `cargo test` with `cargo llvm-cov --lcov --output-path lcov.info` and publish the artifact from there. Delete the `unit-tests` job: `scripts/check.sh` in the `Check` job still runs `cargo test --bin exapump`, so the unit-test pass/fail signal survives.

The coverage signal does not survive intact. Deleting `unit-tests` removes the repository's only container-free coverage producer, so after this change every coverage number depends on a container start whose readiness step alone reserves `timeout-minutes: 30`, and a container flake leaves `Sonar Analysis` skipped rather than evaluated. Accepted because unit-test-only coverage is the defect being fixed and `integration-tests` already blocks the merge on container failure. Reinstate a container-free coverage job when either trigger fires: `integration-tests` flakes on container start in more than one run out of ten over a calendar month, or a contributor needs a coverage number on a machine that cannot run the Exasol image. The reinstated job publishes a second lcov file and `sonar-project.properties` consumes both through a comma-separated `sonar.rust.lcov.reportPaths`.

**Lever 2: give the new-code period a baseline.** Pass `sonar.projectVersion` from the `Cargo.toml` version. The `Sonar way` default new-code definition is the previous version, and the project currently has none. `speq-implement-pr` bumps the version on this plan's branch, so the next `main` analysis will carry a version different from `not provided` and will resolve a non-empty period.

**Lever 3: separate decision logic from prompting and printing.** Every flagged function mixes branching decisions with I/O. Extracting the decision into a pure function that returns a value cuts cognitive complexity below 15 and makes the logic reachable from unit tests. `profile::show` is the clearest case: replace the cascade of `if let Some(...) { println!(...) }` with a function returning the rows to print, then print them.

#### Architecture

```
 integration-tests job (Exasol container)
   cargo llvm-cov  ->  lcov.info  ->  artifact
                                         |
                                         v
                              sonar job (needs: integration-tests)
                                sonar-scanner + sonar.projectVersion
                                         |
                                         v
                                  Quality Gate (new code)

 src/commands/<cmd>.rs
   pub fn/async fn run(...)      thin: parse args, call logic, do I/O
        |
        +--> pure helper(s)      branching decisions, no I/O, unit-tested
```

#### Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| Extract pure decision function | `profile.rs`, `export.rs`, `sql.rs`, `interactive.rs` | Cuts S3776 complexity and makes the branch logic reachable without a TTY or a database |
| Dependency inversion on prompting | `profile.rs` `init`, `edit`, `prompt_bucketfs`, `edit_bucketfs` | A `ProfilePrompter` trait replaces every `inquire` and `rpassword` call in the `init` and `edit` flows. The caller supplies the implementation, so the whole flow runs under test against canned answers instead of only the extracted helpers |
| Table-driven rendering | `profile::show` | Replaces a 13-branch print cascade with data plus one loop |

### Consequences

| Decision | Alternatives Considered | Rationale |
|----------|------------------------|-----------|
| Measure coverage in `integration-tests` | Keep `unit-tests` and merge two lcov files via a comma-separated `sonar.rust.lcov.reportPaths`; or list Exasol-free test targets explicitly in `unit-tests` | One report from one run is simpler. Listing Exasol-free targets is brittle: tests panic rather than skip when Exasol is absent, so the list breaks whenever a test file gains a database case |
| Accept coupling the Sonar job to the container job | Keep a fast container-free coverage job | Coverage from unit tests alone is the defect being fixed. If the container fails, `integration-tests` already blocks the merge, so no new class of failure appears |
| Set `sonar.projectVersion` from `Cargo.toml` | Leave it unset and rely on the second analysis resolving a baseline on its own | Cheap insurance against a repeat of status `NONE`, and it gives Sonar useful release history. The baseline behavior without a version is inferred, not documented, so making it explicit removes the guess |
| Extract logic rather than split functions mechanically | Add `#[allow]`-style suppressions, or raise the S3776 threshold | Suppression leaves the 377 untestable lines untestable. The extraction fixes complexity and coverage with one change |
| Cover before refactoring each file | Refactor everything, then add tests | Refactored lines count as new code and need 80% coverage on this PR's own analysis. Pairing each refactor with its tests keeps the gate green while the work lands |

## Features

| Feature | Status | Spec |
|---------|--------|------|
| (none) | n/a | No spec scenarios added or modified; test coverage and code quality only |

No user-observable behavior described in the spec library changes. `specs/mission.md` and every `specs/<domain>/<feature>/spec.md` are untouched. The existing suite under `tests/` is the regression net that proves the refactors preserve behavior.

## Impact

None for users and operators. The CLI keeps its current commands, flags, output and exit codes, and no configuration file or profile format changes. Contributors see two changes: the `unit-tests` CI job is gone, and coverage now comes from the `integration-tests` job, so a coverage report requires a running Exasol container.

One file enters the spec library at record time: `/speq:record` promotes `decision-log.md` [2] (where coverage is measured) and [4] (extract logic rather than suppress `rust:S3776`) into `specs/_decision/005-fix-sonar-quality-gate.md`. Those two decisions are the only ones in the log flagged for promotion. That file is the only write outside `specs/_recorded/`; no `specs/<domain>/**/spec.md` and no `specs/mission.md` edit.

## Requirements

`Verified` states where each criterion is decided. **In-loop** criteria are measurable during `/speq:implement` from a local `cargo llvm-cov` run or a local command, and gate the implementation report. **Operator** criteria need a SonarCloud analysis that does not exist while the implementation runs; they move to the Post-Merge Operator Checklist and must not be checked off by an implementer.

| Requirement | Details | Verified |
|-------------|---------|----------|
| Overall coverage | `cargo llvm-cov --summary-only` TOTAL line coverage at or above 85% of lines against a running Exasol container. Baseline: task 1.1 records it; 82.46% is the container-free floor | In-loop |
| `profile.rs` coverage | `cargo llvm-cov --summary-only` reports `commands/profile.rs` at or above 70% of lines. Baseline 38.30% from the same command. If the task 1.1b per-function measurement shows the target is unreachable, tasks 1.1c and 1.1d revise this number before Phase 2 starts, and the revised number is the requirement | In-loop |
| Cognitive complexity | Zero open `rust:S3776` issues. Baseline 9. Every listed function at or below the rule's configured `threshold` parameter. Planning measured that threshold as **15** via `api/rules/show?key=rust:S3776&organization=exasol-labs` (`defaultValue: "15"`, not overridden in the project's Rust `Sonar way` profile `AZ9q0GTXpguOUvh2Rvhk`). Task 1.7 re-confirms it and replaces this number if it has changed | In-loop for the per-function count (task 1.7's manual count method); Operator for the issue total |
| New-code duplication | `new_duplicated_lines_density` at or below 3 on this plan's PR analysis. This is the gate condition; the extracted functions are the new code it measures, so they must consolidate duplicated blocks rather than copy them | Operator |
| Overall duplication | Secondary check: `duplicated_lines_density` stays at or below 3%. Baseline 2.1%, concentrated in `interactive.rs` (8.9%), `bucketfs.rs` (7.9%) and `sql.rs` (3.7%) | Operator |
| New-code hotspots | `new_security_hotspots_reviewed` at 100 on this plan's PR analysis. This is a gate condition and it fails at anything below 100 | Operator |
| Ratings | `reliability_rating`, `security_rating` and `sqale_rating` stay at A. Bugs, vulnerabilities and security hotspots stay at 0 | Operator |
| Gate status | `api/qualitygates/project_status` returns `OK` for this plan's PR analysis and for the next `main` analysis, not `NONE` and not `ERROR` | Operator |
| Behavior | Every existing test under `tests/` passes unchanged. No test is deleted or weakened to accommodate a refactor | In-loop |

**Both coverage rows count test code as source.** `sonar-project.properties` sets `sonar.sources=src`, so a `#[cfg(test)]` module inside `src/` is source that `cargo llvm-cov` reports as covered by definition. A large `ScriptedPrompter` test module therefore lifts both figures without covering one production line. This plan does not exclude those modules from the report. It evaluates both rows over baseline line sets instead. The `profile.rs coverage` row is evaluated over that file's baseline coverable line set alone — the lines task 1.1 records. Every line this plan's new `#[cfg(test)]` module adds is excluded from that row's numerator and from its denominator. The `Overall coverage` row is evaluated the same way, against the task 1.1 baseline `lines_to_cover`. Four measurements bind to those sets: the task 1.1 baseline, the task 1.1b counts, each Phase 2 per-file check, and the task 3.2 re-measurement. Tasks 3.1 and 3.2 also re-run the task 1.1b measurement, which no test line can inflate. Task 3.2's ban on assertion-free tests is what keeps the number honest.

**Both rows are read on the report-side line rule.** `cargo llvm-cov --summary-only` produces both figures. Sonar computes its own coverage from the lcov export, which uses a different line rule: 604 coverable lines for `profile.rs` against the report's 611. The two run about one point apart on that file, so a Sonar percentage is never compared directly against a row here.

The two headline Goals (gate `OK` on the PR analysis and on the following `main` analysis) are Operator criteria by construction. An implementation report may claim only the In-loop rows.

## Dependencies

| Dependency | Purpose | Status |
|------------|---------|--------|
| `cargo-llvm-cov` | Coverage instrumentation | Installed in CI via `taiki-e/install-action@cargo-llvm-cov`; move the step into `integration-tests` |
| `llvm-tools-preview` | Rust component `cargo-llvm-cov` requires | Add to the `integration-tests` toolchain step |
| `rustfilt` | Demangles the v0-mangled `FN:` names task 1.1b reads from the lcov file | Absent on this machine; install with `cargo install rustfilt`. Local only, not needed in CI |
| Exasol `exasol/docker-db:2025.2.0` | Coverage run needs the full suite to pass | Already started by `integration-tests` |
| SonarCloud `exasol-labs_exapump` | Gate evaluation | Provisioned; `SONAR_TOKEN` present for same-repo runs |

## Implementation Tasks

### Phase 1: Record the baseline and fix coverage measurement

- [ ] 1.1 Start `exasol/docker-db:2025.2.0` locally per `CLAUDE.md`, wait with `exapump wait`, then run the suite once and report from it twice:

  ```sh
  cargo llvm-cov --no-report
  cargo llvm-cov report --summary-only
  cargo llvm-cov report --lcov --output-path /tmp/lcov-baseline.info
  ```

  Record the TOTAL and per-file line coverage from the `--summary-only` table. This is the true full-suite baseline; the 82.46% figure in Design was measured without a container and is a floor. Record `commands/profile.rs` coverable and uncovered line counts separately: tasks 1.1c and 1.1d name them as the baseline `coverable` and `uncovered`.

  `--summary-only` MUST NOT be combined with `--lcov`. That combination strips every `FN:` and `DA:` record from the file and leaves only `LF`/`LH` totals, and task 1.1b needs both record types. Verified on `cargo-llvm-cov` 0.8.7.

  Produce no JSON export. Task 1.1b reads the lcov file alone, and a JSON export sitting beside it only invites the cross-format comparison task 1.1b forbids.
- [ ] 1.1b Emit a per-function **uncovered source line** count for `src/commands/profile.rs` from the `/tmp/lcov-baseline.info` that task 1.1 wrote, one row per function, sorted descending. Task 1.1b-ii turns this table into R. R is a count of uncovered source lines. That is the same unit as the 194 lines task 1.1c compares it against and as the `(coverable - uncovered + R) / coverable` formula in task 1.1d. Do not count regions: `llvm-cov` emits several regions per line, so a region count overstates the figure by a large and variable factor and cannot be compared against 194. [expert]

  Read the counts from the lcov file, not from the JSON export's `files[].segments`. Inside the `SF:` block whose path ends `commands/profile.rs`, every `DA:<line>,<hits>` record is one measured line under llvm-cov's own line rule, and every `FN:<line>,<mangled-name>` record gives one function's start line. Bucket each `DA` line into the function whose `FN` start line is the greatest start line at or below it. This reuses llvm-cov's line mapping and reimplements nothing.

  ```python
  # python3 - /tmp/lcov-baseline.info
  import bisect, sys
  block, keep = [], False
  for ln in open(sys.argv[1]).read().splitlines():
      if ln.startswith("SF:"):
          keep, block = ln[3:].endswith("commands/profile.rs"), []
      elif ln == "end_of_record":
          if keep:
              break
      elif keep:
          block.append(ln)
  fns = sorted((int(l[3:].split(",", 1)[0]), l[3:].split(",", 1)[1]) for l in block if l.startswith("FN:"))
  da = [tuple(int(v) for v in l[3:].split(",")[:2]) for l in block if l.startswith("DA:")]
  lf = int(next(l[3:] for l in block if l.startswith("LF:")))
  lh = int(next(l[3:] for l in block if l.startswith("LH:")))
  assert len(da) == lf, f"DA records {len(da)} != LF {lf}"
  assert sum(1 for _, h in da if h == 0) == lf - lh, "zero-hit DA count != LF - LH"
  starts = [s for s, _ in fns]
  rows = {}
  for line, hits in da:
      i = bisect.bisect_right(starts, line) - 1
      if i < 0:
          continue
      r = rows.setdefault(fns[i][1], [0, 0, starts[i]])
      r[1] += 1
      if hits == 0:
          r[0] += 1
  for name, (unc, total, start) in sorted(rows.items(), key=lambda kv: -kv[1][0]):
      print(f"{unc:5d} uncovered /{total:5d} measured  @{start}  {name}")
  ```

  Self-check, internal to the lcov file and to nothing else: the count of zero-hit `DA` records in that `SF:` block MUST equal `LF - LH` for the same block, and the `DA` record count MUST equal `LF`. Both are the two `assert` lines above. A failure means the file was parsed wrong, not that llvm-cov is wrong.

  **Never compare an lcov figure against a JSON-export figure.** `summary.lines.count` in the JSON export uses a different line rule from the lcov export: 611 lines for `profile.rs` against lcov's 604, on `cargo-llvm-cov` 0.8.7. The two disagree by construction on almost every file, so a mismatch between them proves nothing.

  Eyeball check only, not a gate and not a completion criterion: `cargo llvm-cov report --html --output-dir /tmp/cov-baseline-html` renders `src/commands/profile.rs` with a hit count beside every line. It renders the report-side rule, so its per-function counts do not match this table exactly and are not required to.

  `FN:` names are v0-mangled (`_RNvCs..._7exapump...`), not `init`. Demangle with `rustfilt` and match the final `::` component exactly, or match the length-prefixed trailing component; a substring match on `edit` also matches `edit_bucketfs`. A closure carries its own `FN` record, so its lines bucket to the closure rather than to the function around it. Fold each closure row into the function whose line range encloses the closure's printed start line.

  Record the table in the implementation notes.
- [ ] 1.1b-ii Sum the task 1.1b table into R. Add the counts for `init`, `edit`, `prompt_bucketfs`, `edit_bucketfs`, `prompt_profile_name`, `prompt_new_password`, `inquire_text`, `inquire_confirm` and `inquire_port` — the functions the task 2.2 design makes reachable. [expert]

  **Then subtract from R every line task 2.2 relocates into `TerminalPrompter`.** Those lines move to the production implementation and stay uncovered there, so they are not coverage this design yields: the `inquire_text` body (`profile.rs:484-496`), the `inquire_confirm` body (`:497-503`), `map_inquire_err` (`:626-635`), and the raw `inquire::Text` call bodies inside `inquire_port` (`:506-509`), `prompt_bucketfs` (`:581-584`, `:590-593`) and `edit_bucketfs` (`:849-852`, `:862-865`). Subtract them by hand from the per-function counts, counting only lines that carry a `DA` record. R counts only the branching and validation lines that remain in the trait-taking functions.

  Record R and the nine per-function counts it was summed from in the implementation notes. Tasks 3.1 and 3.2 re-run this measurement and compare against those nine counts.
- [ ] 1.1c Compare R against 194 lines. That is the number of additional covered lines the § Requirements `profile.rs coverage` row's 70% target needs from the 38.30% baseline over roughly 611 coverable lines (`611 * (0.70 - 0.383) = 194`), and the overall 85% target depends on it. If R is at or above 194 lines, record `targets stand, R = <n> lines` in the implementation notes; the § Requirements figures stand unchanged and task 1.1d is skipped. If R is below 194 lines, record `targets revised, R = <n> lines` and do task 1.1d.
- [ ] 1.1d Conditional on R below 194 lines; skip it when task 1.1c recorded `targets stand`. Recompute both targets from the measured R, in line units throughout: set the `profile.rs` target to `(coverable - uncovered + R) / coverable` rounded down to the nearest whole percent, and set the overall target to the TOTAL that the same R implies against `lines_to_cover`, also rounded down to the nearest whole percent.

  `coverable` and `uncovered` in that formula are the task 1.1 baseline values for `src/commands/profile.rs`, and `lines_to_cover` is the task 1.1 baseline total. They are never re-measured after this plan's `#[cfg(test)]` module exists. The achieved figure that tasks 3.1 and 3.2 check is computed on that same baseline denominator, so a test module cannot move either side of the comparison. Rewrite the § Requirements `profile.rs coverage` and `Overall coverage` rows with the two revised figures, append the measured R and both revised figures to `decision-log.md` [7] § Gate, and record the revision in the implementation notes. The revised numbers are then the requirement: a 70% target that R contradicts is not carried forward, and it is not closed with assertion-free tests. No Phase 2 or Phase 3 task may be marked done against an unrevised target.
- [ ] 1.2 In `.github/workflows/ci.yml`, add `with: components: llvm-tools-preview` to the `integration-tests` toolchain step and add the `taiki-e/install-action@cargo-llvm-cov` step after it.
- [ ] 1.3 In the `integration-tests` job, replace `run: cargo test --verbose` with `run: cargo llvm-cov --lcov --output-path lcov.info`, keeping `env: REQUIRE_EXASOL: "1"`. Add the `actions/upload-artifact@v4` step for `lcov.info` (name `lcov`, `if-no-files-found: error`) after the test step and before the container teardown steps.
- [ ] 1.4 Delete the `unit-tests` job and change the `sonar` job's `needs: [unit-tests]` to `needs: [integration-tests]`.
- [ ] 1.5 In the `sonar` job, add a step that reads the crate version from `Cargo.toml` into a step output, then pass `args: -Dsonar.projectVersion=<version>` to `SonarSource/sonarqube-scan-action@v8.2.1`.
- [ ] 1.6 Update the comment above `sonar.rust.lcov.reportPaths` in `sonar-project.properties`: the report now covers the full suite, not unit tests.
- [ ] 1.7 Run `curl -s "https://sonarcloud.io/api/rules/show?key=rust:S3776&organization=exasol-labs"` and read the `threshold` parameter, then run `curl -s "https://sonarcloud.io/api/rules/search?organization=exasol-labs&qprofile=AZ9q0GTXpguOUvh2Rvhk&activation=true&rule_key=rust:S3776&f=params"` to confirm the project's Rust `Sonar way` profile does not override it. Write the value into the § Requirements `Cognitive complexity` row. Planning measured 15; if the value differs, that value replaces 15 everywhere in Phase 2 and the Phase 2 tasks target the new number. Record this counting method verbatim in the implementation notes; it is the only in-loop check on all seven Phase 2 tasks, and undercounting lets a task be marked done while Sonar still scores the function above the threshold.

  1. **Base increment, +1 each:** `if`, `else if`, **`else`**, `match`, `loop`, `while`, `for`, and each sequence of `&&`/`||` operators (one increment per sequence of the same operator, not per operator). `?` does not count.
  2. **Recursion, +1** per recursive call cycle.
  3. **Labeled jumps, +1** per `break` or `continue` that carries a label.
  4. **Nesting increment:** add the current nesting level as an extra increment, and add it **only** to `if`, `match`, `loop`, `while`, `for` and closure bodies. Never add it to `else`, to `else if`, to a boolean-operator sequence, or to a labeled jump — those take their flat +1 wherever they sit.
  5. **Nesting level** starts at 0 and is raised by one inside the body of an `if`, an `else`, a `match`, a loop, or a closure.

  **Calibration, mandatory before any Phase 2 counting.** Hand-count these two functions with the method above and reproduce both numbers, which are SonarCloud's own scores for this codebase and match the § Design/Context table:

  - `profile::show` (`profile.rs:220`) must count to **19**: one `match` at level 0 (+1), nine `if let`/`if` at level 1 (+2 each, 18). It contains no `else`.
  - `profile::init` (`profile.rs:370`) must count to **22**: eight `if` (seven at level 0 and one nested, 9), five `match` at level 0 (5), five bare `else` at `:418`, `:429`, `:443`, `:456` and `:477` (+1 each with no nesting increment, 5), one `||` sequence (+1, no nesting increment), one `for` nested inside an `if` (+2). `:456` is an inline `else` inside the `Profile` struct literal — `default: if make_default { Some(true) } else { None },` — so a scan for block-shaped `else` keywords misses it and lands on 21.

  A method that fails either calibration is wrong. Correct it and re-run both before counting any refactored function, and do not mark a Phase 2 task done against a count produced by an uncalibrated method.

### Phase 2: Cut cognitive complexity and expose the logic to tests

Each task drives the named function to cognitive complexity at or below the task 1.7 threshold and covers the extracted logic with unit tests in the same change.

**Local verification, per task, before the task is marked done.** Both steps run without SonarCloud:

1. Count cognitive complexity by hand for the refactored function and for every function extracted from it, applying the counting method task 1.7 recorded. Reproduce task 1.7's two calibration counts (`profile::show` = 19, `profile::init` = 22) before counting anything in this phase; a method that misses either number undercounts and must be corrected first. Write the count for each function into the implementation notes next to its name. A function above the threshold fails the task.
2. Run `cargo llvm-cov --summary-only` and read the line-coverage figure for the file the task touched. It must not fall below the figure task 1.1 recorded for that file.

The Sonar issue total is not verifiable in-loop and is not a completion criterion for any Phase 2 task; it is checked in the Post-Merge Operator Checklist, which also defines the remediation loop when it is above zero.

- [ ] 2.1 `src/commands/profile.rs:220` `show` (complexity 19): extract a pure function that maps a `Profile` to the ordered label/value rows, masking `password`, `bfs_write_password` and `bfs_read_password` as `****`. Reduce `show` to loading the config and printing the rows. Unit-test a fully populated profile, a minimal profile and the not-found error.
- [ ] 2.2 `src/commands/profile.rs:370` `init` (22) and `:681` `edit` (22): invert the dependency on prompting. Introduce this trait in `profile.rs` and route every `inquire::*` and `rpassword::*` call through it:

  ```rust
  trait ProfilePrompter {
      fn text(&mut self, label: &str, default: Option<&str>, required: bool) -> anyhow::Result<String>;
      fn confirm(&mut self, label: &str, default: bool) -> anyhow::Result<bool>;
      fn password(&mut self, label: &str) -> anyhow::Result<String>;
      fn notice(&mut self, message: &str);
  }
  ```

  `text` renders `"{label}:"` and applies the `required` empty-check, reproducing today's `inquire_text` exactly; the raw `inquire::Text` calls in `inquire_port`, `prompt_bucketfs` and `edit_bucketfs` become `text` calls with the same rendered label and `required: false`. Label rendering and the `required` empty-check with its `"{} is required"` failure live in exactly one place — either a default method on `ProfilePrompter` that both implementations inherit, or a free function both call. Each trait method implementation then carries only the prompting itself, so `ScriptedPrompter` cannot diverge from `TerminalPrompter` on a label or on the empty-check.

  `password` wraps `rpassword::prompt_password` and takes the full prompt string. `notice` carries the retry feedback that is `println!` today. All five messages move to `notice`, so the retry loops in `inquire_port`, `prompt_profile_name` and `prompt_new_password` become testable: `"  not a valid port — enter 1..65535"` (`profile.rs:512`, and the same string again at `:709` inside `edit`'s own port loop), `"  '{}' already exists — choose another name"` (`:528`), the `config::validate_profile_name` error passthrough `"  {}"` (`:533`), `"  password cannot be empty"` (`:542`), and `"  passwords did not match — try again"` (`:549`). Leaving any of them as a bare `println!` leaves its branch unassertable.

  Three other `println!` calls stay bare `println!` and do **not** route through `notice`: `"Profile '{}' created{}"` (`profile.rs:480`), `"Editing profile '{}' — press Enter to keep current value."` (`:695`) and `"Profile '{}' updated{}"` (`:804`). They are terminal status output, not retry feedback, and routing them through `notice` would put status text into the recorded label sequence the tests assert on. The `make_default` tests therefore assert the `default` field of the `Profile` the inner function returns to its wrapper, not the printed suffix. That return value is what the wrapper hands to `config::save_config`, so it is already on the seam.

  Change `init`, `edit`, `prompt_bucketfs`, `edit_bucketfs`, `prompt_profile_name`, `prompt_new_password`, `inquire_port`, `inquire_text` and `inquire_confirm` to take `&mut dyn ProfilePrompter`. Ship one production implementation, `TerminalPrompter`, holding the current `inquire`/`rpassword` bodies and the `map_inquire_err` mapping. Construct it, and keep the `!stdin().is_terminal()` guard with its unchanged error message, in a thin outer `init`/`edit` wrapper that also does `config::load_config` and `config::save_config`; the wrapper is the only part that stays uncovered.

  `remove` (`profile.rs:652`) and `prompt_password_for` (`:636`) do **not** convert to `&mut dyn ProfilePrompter` in this plan. Both hold direct calls (`inquire::Confirm` at `:665`, `rpassword::prompt_password` at `:645`) and both carry their own `is_terminal` guard, but neither is part of the `init`/`edit` flow this trait targets, so both stay as they are and neither contributes to R.

  Do not merge `prompt_bucketfs` (`profile.rs:554`) and `edit_bucketfs` (`:809`) into one function. They diverge on four axes — opening prompt (`"Configure BucketFS? (needed for \`exapump bucketfs\` commands)"` versus `"Edit BucketFS settings?"`), decline behavior (all-`None` versus the current values), defaults (`Some("")`, `DEFAULT_BFS_PORT`, `"default"` versus the profile's current values), and password prompt wording — and decision [8] plus § Verification/Manual Testing require all four to survive byte-identical. One function carrying four mode flags is a shallower abstraction than the two it replaces. Extract only what is genuinely shared: the port parse-and-validate step and the blank-to-`None` mapping, as helpers over the trait. `prompt_bucketfs` and `edit_bucketfs` stay separate callers owning their own prompt strings, defaults and decline behavior.

  **The shared port helper is the parse-and-validate step alone, and it does not retry.** Each caller keeps its own failure behavior. `prompt_bucketfs` (`profile.rs:585-588`) and `edit_bucketfs` (`:853-856`) KEEP their current hard failure, `anyhow::bail!("invalid BucketFS port: {}", port_raw)`, with no retry and no `notice`. The retry-with-`notice` loops belong only to `inquire_port` (`:504-515`) and to `edit`'s own port loop (`:702-711`), neither of which is BucketFS. Turning either BucketFS bail into a re-prompt is a user-visible CLI behavior change, which § Non-Goals and `decision-log.md` [8] both forbid, and no test under `tests/` would catch it. `ScriptedPrompter` tests MUST assert the recorded label sequence for both flows; `grep -n "blank to skip\|blank = clear\|Configure BucketFS\|Edit BucketFS" tests/profile_test.rs` returns nothing today, so no existing test would catch a prompt-string regression.

  Unit-test with a test-only `ScriptedPrompter` that answers from a queue and records the labels it was asked, covering: `init` with and without each `args` field pre-supplied, `make_default` both ways including the default-clearing loop, `--no-bucketfs`, the BucketFS blank-to-skip paths, `edit`'s keep-current-value paths, its change-password and blank-to-clear password paths, and the port and profile-name retry loops. Assert on the recorded labels so the prompt sequence stays byte-identical. Do not add a public API: `ProfilePrompter` and both implementations stay private to the module. [expert]
- [ ] 2.3 `src/commands/sql.rs:13` `strip_comments` (35) and `:80` `split_statements` (60): reduce both scanners below 15 without changing their output for any input. Both are hand-written state machines over quoting, line and block comments, and the `CREATE ... SCRIPT ... AS` header that switches to script-body scanning. Extract the per-state transition handling; do not rewrite the algorithm. The existing 78 unit tests in this file are the correctness net; add cases for any branch the extraction leaves uncovered. [expert]
- [ ] 2.4 `src/commands/sql.rs:395` `run` (54): extract the input-source selection (argument, file, stdin), the per-`OutputFormat` rendering, and the status-line construction into separate functions. Unit-test each extracted function; leave the database call in `run`. This task owns every signature change to `write_csv` (`sql.rs:364`) and `write_json` (`sql.rs:377`) — making the rendering unit-testable means giving both a `&mut impl Write` parameter instead of writing to stdout directly, and no other task may change those signatures. Land the new signatures and update the two call sites in `interactive.rs` (`:244`, `:249`, `:292`, `:297`) in this task so the tree builds, without extracting anything in `interactive.rs`.
- [ ] 2.5 `src/commands/interactive.rs:226` `execute_statement` (66): extract the result-rendering and error-reporting branches from the execution path. Runs after task 2.4 is merged and consumes `write_csv`, `write_json` and `split_statements` at whatever signature 2.4 left them at — this task must not change any of those three. This is the highest-complexity function in the codebase and sits inside the REPL loop, so preserve the exact prompt, output and error text that `tests/cli_test.rs` asserts on. Consolidate the 50 duplicated lines Sonar reports in this file rather than copying them into the new functions. Unit-test the extracted renderers. [expert]
- [ ] 2.6 `src/commands/interactive.rs:153` `run` (26): extract the REPL setup and the per-line dispatch of meta-commands versus SQL. Unit-test the dispatch classification. Runs after task 2.3 is merged and consumes `split_statements` (`interactive.rs:195`, `:369`) unchanged.
- [ ] 2.7 `src/commands/export.rs:67` `run` (38): extract the export-source resolution (`--table` versus `--query`, including the mutual-exclusion errors), the CSV option assembly, and the split-writer selection. Unit-test each extracted function, including the `--compression` with CSV rejection.

### Phase 3: Close the remaining coverage gaps

- [ ] 3.1 Raise `src/commands/profile.rs` to the § Requirements `profile.rs coverage` figure (70% of lines, or the figure task 1.1d substituted) by unit-testing through the `ProfilePrompter` seam task 2.2 introduced, targeting the 377 lines currently unreachable through `assert_cmd`.

  **The file percentage alone does not complete this task.** Re-run the task 1.1b measurement against a fresh `cargo llvm-cov report --lcov` and sum the uncovered-line counts over the same nine functions task 1.1b-ii summed R from. That sum MUST have fallen by at least the R task 1.1c or task 1.1d recorded. Record the task 1.1b-ii baseline sum and the achieved sum in the implementation notes. Lines added by the new `#[cfg(test)]` module bucket to their own `FN` records, so they cannot move this number.
- [ ] 3.2 Re-measure with `cargo llvm-cov --summary-only`. If TOTAL is below the § Requirements `Overall coverage` figure (85% of lines, or the figure task 1.1d substituted), add tests for the next-largest gaps in order: `sql.rs` (96 lines), `interactive.rs` (69), `bucketfs.rs` (44), `export.rs` (28), `wait.rs` (27), `split.rs` (25). Stop at 85%; do not write tests that assert nothing in order to move the number.

  Repeat task 3.1's uninflatable check as the final gate: re-run the task 1.1b measurement, sum the uncovered-line counts over the same nine functions, and require that sum to sit at least R below the task 1.1b-ii baseline sum. Record both counts in the implementation notes. A TOTAL that reaches the § Requirements figure while this sum has not fallen by R means test-module lines carried it, and the plan's targets are not met.

### Phase 4: Verify the gate

- [ ] 4.1 Run the full local checklist in the Verification section against a running Exasol container. All of build, test, clippy, fmt, `cargo deny` and coverage must pass.

Phase 4 ends here. Every remaining acceptance criterion needs a SonarCloud analysis that does not exist while `/speq:implement` runs, and is owned by the Post-Merge Operator Checklist below.

## Post-Merge Operator Checklist

Not implementation tasks. These are operator actions on a live pull request and on `main`; no implementer may execute or check them off, and the implementation report must not claim them. Run them in order.

**O1 — after the PR analysis completes.** Confirm `api/qualitygates/project_status?projectKey=exasol-labs_exapump&pullRequest=<n>` returns `OK`, and that `new_duplicated_lines_density` is at or below 3 and `new_security_hotspots_reviewed` is 100 in its `conditions` array. Confirm `api/issues/search?componentKeys=exasol-labs_exapump&rules=rust:S3776&resolved=false&pullRequest=<n>` returns `"total":0`.

Remediation when that total is above zero: read each remaining issue's `component` and `line`, which name the function still above the threshold. If the function is one this plan refactored, reopen the owning Phase 2 task and extract further; the hand count recorded for that function in the implementation notes was wrong, so correct the count and the notes with it. If the function is new — introduced by an extraction in this plan — the extraction split the work unevenly and the owning task is reopened to rebalance it. Push the fix to the same PR and repeat O1. Do not merge on a non-`OK` PR gate, do not suppress the rule, and do not change the threshold.

**O2 — after merge.** Confirm the `main` analysis returns `OK` and `coverage` is at or above the § Requirements overall figure. If the status is `NONE` again, `sonar.projectVersion` did not resolve a new-code baseline. The remaining remedy is the SonarCloud project's New Code definition, which lives in project settings and not in this repository: in SonarCloud, open **exasol-labs_exapump → Administration → New Code**, set it to **Previous version**, and if that still yields `NONE`, set it to **Number of days = 30**, then re-run the `main` analysis. Do not change the gate definition and do not remove `sonar.qualitygate.wait=true`.

## Parallelization

| Parallel Group | Tasks |
|----------------|-------|
| Group A | 1.1, 1.1b, 1.1b-ii, 1.1c, 1.1d (strictly in order — 1.1 is the only container run and writes both report files, 1.1b reads its lcov file, 1.1b-ii sums R, 1.1c reads R, 1.1d runs only if 1.1c says so), 1.7 |
| Group B | 1.2 to 1.5 (all edit `.github/workflows/ci.yml`, so run them in order), 1.6 |
| Group C1 | 2.1 + 2.2 + 3.1 (`profile.rs`), 2.3 + 2.4 (`sql.rs`), 2.7 (`export.rs`) |
| Group C2 | 2.5 + 2.6 (`interactive.rs`) |
| Group D | 3.2, 4.1 (run them in order) |

Sequential dependencies:

- Group A to Group B: the baseline must be recorded before the CI change alters what gets measured.
- **Group A to Group C1: Phase 2 may not begin until task 1.1c or task 1.1d is checked off.** One of the two always applies: 1.1c when R is at or above 194 lines, 1.1d when it is below. Until one carries a check, no Phase 2 or Phase 3 task may start and none may be marked done, because the coverage target it would be measured against is unconfirmed.
- Group B to Group C1: refactored lines count as new code, so the corrected coverage pipeline must exist before the refactors land.
- **Group C1 to Group C2: `interactive.rs` depends on `sql.rs`.** `src/commands/interactive.rs:7` imports `error_hint`, `split_statements`, `write_csv` and `write_json` from `sql.rs`, and tasks 2.3 and 2.4 change exactly those. Running the two files' lanes concurrently produces either a broken build or one lane silently reverting the other's signature change, which the "no test may be edited" rule makes expensive to resolve. Task 2.4 lands the `write_csv`/`write_json` signature change together with the `interactive.rs` call-site updates; tasks 2.5 and 2.6 then start from a building tree and change nothing in `sql.rs`.
- Group C2 to Group D: the final measurement and local checklist need every refactor and its tests in place.

Inside a lane the tasks are ordered: extract, then test, then close the gap. The `profile.rs`, `sql.rs` and `export.rs` lanes in Group C1 touch no file in common and run concurrently.

## Dead Code Removal

| Type | Location | Reason |
|------|----------|--------|
| CI job | `unit-tests` in `.github/workflows/ci.yml` | Coverage moves to `integration-tests`; `scripts/check.sh` in the `Check` job already runs `cargo test --bin exapump` |
| Function | Private helpers in `profile.rs`, `sql.rs`, `interactive.rs` and `export.rs` left unreferenced after extraction | Superseded by the extracted pure functions. `cargo clippy -- -D warnings` fails on `dead_code`, so any leftover surfaces in the `Check` job |

No source file is deleted. No test is deleted.

## Verification

### Scenario Coverage

| Scenario | Test Type | Test Location | Test Name |
|----------|-----------|---------------|-----------|
| (none) | n/a | n/a | No spec scenarios added or modified; see the Features table |

The refactors must preserve behavior, and the existing suite proves it. Every test under `tests/` (`bucketfs_test.rs`, `cli_test.rs`, `csv_test.rs`, `env_test.rs`, `export_test.rs`, `parquet_test.rs`, `profile_test.rs`, `transport_test.rs`, `wait_test.rs`) and every `#[cfg(test)]` module in `src/` must pass unchanged. A refactor that requires editing an existing assertion has changed behavior and must be reverted.

New unit tests added by this plan cover extracted pure functions, which is the case `/speq:planning` reserves unit tests for: computation with no I/O and no side effects.

### Manual Testing

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| Coverage measurement | `cargo llvm-cov --summary-only` with Exasol on `localhost:8563`, run with `dangerouslyDisableSandbox: true` | TOTAL and `commands/profile.rs` at or above their § Requirements figures |
| Cognitive complexity cleared (operator, O1) | `curl -s "https://sonarcloud.io/api/issues/search?componentKeys=exasol-labs_exapump&rules=rust:S3776&resolved=false"` | Response contains `"total":0` |
| Gate status on main (operator, O2) | `curl -s "https://sonarcloud.io/api/qualitygates/project_status?projectKey=exasol-labs_exapump&branch=main"` | `"status":"OK"` with a non-empty `conditions` array |
| Project measures on main (operator, O2) | `curl -s "https://sonarcloud.io/api/measures/component?component=exasol-labs_exapump&metricKeys=coverage,duplicated_lines_density,code_smells"` | `coverage` at or above the § Requirements figure, `duplicated_lines_density` at or below 3. For `code_smells`: the `rust:S3776` count must be 0, checked by the issue-search row above. `code_smells` counts every rule, so it is not required to be 0 — any non-`rust:S3776` smell this PR introduces is acceptable at up to 3 and is logged as a follow-up, and a non-`rust:S3776` smell that pushes `sqale_rating` off A is not acceptable |
| Profile display unchanged | `exapump profile show <name>` against an existing profile | Byte-identical output to the pre-refactor binary for the same profile |
| SQL splitting unchanged | `exapump sql --file <script.sql> --dsn 'exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0'` with a script containing comments, quoted semicolons and a `CREATE SCRIPT ... AS` block, run with `dangerouslyDisableSandbox: true` | Same statement split and same results as the pre-refactor binary |
| CI pipeline | Push the branch and inspect the run | `Check`, `Integration Tests` and `Sonar Analysis` all pass; no `unit-tests` job appears |

### Checklist

| Step | Command | Expected |
|------|---------|----------|
| Build | `cargo build` | Exit 0 |
| Test | `cargo test` against Exasol Docker DB on `localhost:8563`, run with `dangerouslyDisableSandbox: true` | 0 failures |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | 0 errors, 0 warnings |
| Format | `cargo fmt --all -- --check` | No changes |
| Licenses | `cargo deny check licenses && cargo deny check advisories` | Pass |
| Coverage | `cargo llvm-cov --summary-only` with Exasol running | TOTAL at or above the § Requirements `Overall coverage` figure |
| Cognitive complexity | Hand count per the task 1.7 method for every function Phase 2 refactored or extracted, read back from the implementation notes | Every function at or below the task 1.7 threshold; a count is recorded for each |
| Full gate | `./scripts/check.sh` | Exit 0 |
