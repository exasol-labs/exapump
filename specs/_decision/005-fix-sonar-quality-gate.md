# Decisions: fix-sonar-quality-gate

## ADR: Measure coverage in the job that has a database

**ID:** measure-coverage-in-integration-tests-job
**Plan:** fix-sonar-quality-gate
**Status:** Accepted

### Context

`cargo llvm-cov --bin exapump` in the `unit-tests` CI job measured only the `#[cfg(test)]` modules in `src/`, so the 179 integration tests under `tests/` contributed nothing to the coverage Sonar reads. That held measured coverage to 54.96% while the full suite, run without a container, already reached 82.46%. The `integration-tests` job already starts `exasol/docker-db:2025.2.0` for the same suite. Keeping `unit-tests` and merging two lcov reports through a comma-separated `sonar.rust.lcov.reportPaths` was rejected as two runs and two artifacts for one number. Enumerating the Exasol-free test targets inside `unit-tests` was rejected as brittle: this project's tests panic rather than skip when Exasol is absent, so the list breaks the moment a test file gains a database case.

### Decision

Run `cargo llvm-cov` inside `integration-tests`, and delete the separate `unit-tests` job. `scripts/check.sh` in the `Check` job already runs `cargo test --bin exapump`, so the unit-test pass/fail signal survives the deletion.

### Options Considered

| Option | Verdict |
|--------|---------|
| Run `cargo llvm-cov` inside `integration-tests`; delete `unit-tests` | ✓ Chosen — one run, one artifact, and the container `integration-tests` already starts covers the full suite |
| Keep `unit-tests`; merge two lcov reports via comma-separated `sonar.rust.lcov.reportPaths` | ✗ Rejected — two runs and two artifacts for one number |
| Enumerate Exasol-free test targets inside `unit-tests` | ✗ Rejected — brittle; tests panic rather than skip when Exasol is absent, so the list breaks whenever a test file gains a database case |

### Consequences

The repository loses its only container-free coverage producer: every coverage number now depends on a container start whose readiness step alone reserves `timeout-minutes: 30`, and a container flake leaves `Sonar Analysis` skipped rather than evaluated. Accepted because unit-test-only coverage was the defect being fixed, and `integration-tests` already blocks the merge when the container fails. Reinstate a container-free coverage job if `integration-tests` flakes on container start in more than one run out of ten over a calendar month, or a contributor needs a coverage number on a machine that cannot run the Exasol image; the reinstated job would publish a second lcov file consumed through a comma-separated `sonar.rust.lcov.reportPaths`.

## ADR: Extract decision logic instead of suppressing rust:S3776

**ID:** extract-logic-over-suppress-s3776
**Plan:** fix-sonar-quality-gate
**Status:** Accepted

### Context

Nine functions across `profile.rs`, `sql.rs`, `interactive.rs` and `export.rs` exceeded SonarCloud's `rust:S3776` cognitive-complexity threshold. In `profile.rs`, `init`, `edit`, `prompt_bucketfs` and `edit_bucketfs` bail on `!stdin().is_terminal()` and then call `inquire` and `rpassword`, so a CLI-driven test stops at the first guard, leaving those functions both over-threshold and uncovered. Complexity and low coverage share one root cause: branching decisions are mixed with I/O.

### Decision

Extract every flagged function's branching logic into pure functions that take values and return values; apply no suppressions and no threshold changes. For `profile.rs`'s `init`, `edit`, `prompt_bucketfs` and `edit_bucketfs`, use an injected-prompter design: a private `ProfilePrompter` trait (`text`, `confirm`, `password`, `notice`) taken as `&mut dyn ProfilePrompter`, with a `TerminalPrompter` production implementation and a `ScriptedPrompter` test implementation.

### Options Considered

| Option | Verdict |
|--------|---------|
| Injected prompter: `ProfilePrompter` trait with `TerminalPrompter`/`ScriptedPrompter` | ✓ Chosen — puts the whole `init`/`edit` flow under test, not only an extracted subset |
| Pure-helper design: leave `init`/`edit` prompting for themselves, extract decision helpers beside them | ✗ Rejected — covers only the extracted subset; the coverage arithmetic this plan needed required the larger yield the injected prompter supplies |
| Suppress `rust:S3776` per function | ✗ Rejected — leaves the untestable code untestable and hides the reason coverage is stuck |
| Raise the `rust:S3776` threshold in the quality profile | ✗ Rejected — same defect, hidden rather than fixed |

### Consequences

Business logic performs no I/O and the consumer defines the abstraction it needs, following this project's design-philosophy guidance. Suppressing or raising the threshold was rejected as the commonly expected shortcut; this ADR records that rejection so a future planner does not re-litigate it. The pattern generalizes: an interactive CLI function that mixes branching with prompting or printing should have its decision extracted behind a narrow trait or pure function rather than have its complexity suppressed.
