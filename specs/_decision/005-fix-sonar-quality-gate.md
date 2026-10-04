# Decisions: fix-sonar-quality-gate

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
