# Decisions: fix-sonar-quality-gate

## ADR: Extract decision logic instead of suppressing rust:S3776

**ID:** extract-logic-over-suppress-s3776
**Plan:** fix-sonar-quality-gate
**Status:** Accepted

### Context

Functions that mix branching with terminal I/O exceed the `rust:S3776` complexity threshold and cannot be tested without a terminal.

### Decision

Move branching logic into pure functions. Put interactive prompting behind a narrow trait that tests replace with a scripted implementation. Add no suppressions and do not change the threshold.

### Options Considered

| Option | Verdict |
|--------|---------|
| Extract pure functions and inject a prompter | ✓ Chosen — puts the whole interactive flow under test |
| Extract only the decision helpers | ✗ Rejected — covers only the extracted part |
| Suppress `rust:S3776` per function | ✗ Rejected — leaves the code untestable |
| Raise the threshold | ✗ Rejected — hides the defect |
