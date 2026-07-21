# Decisions: fix-glibc-compatibility

## ADR: No spec domain; record the constraint in mission.md and enforce it in the workflow

**ID:** no-spec-domain-for-glibc-floor
**Plan:** fix-glibc-compatibility
**Status:** Accepted

### Context

Issue #32 reported `GLIBC_2.39 not found` on SLES 15 SP7. `auto-tag.yml`'s `build-binaries` job built both Linux gnu targets natively on Ubuntu 24.04 runners, which ship glibc 2.39. Binaries linked against glibc 2.39 symbols refuse to run on older glibc. The fix required a durable glibc-floor constraint on released Linux binaries. The plan needed to decide where that constraint lives: a new spec domain, or plan tasks plus decision-log rationale.

### Decision

Add no feature spec and no `release/` or `packaging/` domain. Record the glibc 2.28 Linux floor in `specs/mission.md` § Constraints and enforce it with a build-time guard step in `auto-tag.yml`.

### Options Considered

| Option | Verdict |
|--------|---------|
| Record in mission.md + enforce via CI guard | ✓ Chosen — matches the non-functional, non-CLI-observable nature of the constraint |
| Create a `release/` domain with a Gherkin scenario asserting glibc ≤ 2.28 linkage | ✗ Rejected — no CLI-observable behavior and no natural cargo test; verification is a shell inspection of the artifact, which would pollute the behavior-focused spec library |

### Consequences

The spec library stays scoped to CLI/library behavior verified by cargo tests. The glibc 2.28 floor is durable and discoverable in `specs/mission.md` § Constraints, and the `auto-tag.yml` build-time guard enforces it at release time instead of via a spec scenario. This follows the precedent set by the recorded `change-exarrow-rs-bump-0.12.3` plan (`Features: (none)`).
