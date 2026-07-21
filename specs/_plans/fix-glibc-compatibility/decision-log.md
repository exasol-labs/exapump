# Decision Log: fix-glibc-compatibility

## Interview

Headless mode — no live interview. The orchestrator supplied confirmed context; the exchanges below capture it as Q&A.

**Q:** What is the root cause of issue #32 (`GLIBC_2.39 not found` on SLES 15 SP7)?
**A:** `auto-tag.yml`'s `build-binaries` job builds both Linux gnu targets natively on Ubuntu 24.04 runners, which ship glibc 2.39. The binaries reference glibc 2.39 symbols and refuse to run on older glibc.

**Q:** Which glibc baseline should the Linux binaries target?
**A:** glibc 2.28, confirmed live as the version in AlmaLinux 8.10 (`docker run --rm almalinux/8-base:latest ldd --version`). It matches the Python ecosystem's `manylinux_2_28` standard — a proven, widely used baseline.

**Q:** How should the binaries be built against glibc 2.28?
**A:** Build the two Linux targets inside an AlmaLinux 8 container. Pick the cleanest concrete mechanism yourself. macOS and Windows are unaffected and build unchanged.

**Q:** Should this add a new spec domain, or be captured as plan tasks plus decision-log rationale?
**A:** Decide and document either way (see Decision 1).

**Q:** Should the `exarrow-rs` bump from 0.13.0 to 0.14.0 be in scope?
**A:** Decide and document either way, so it is not silently dropped (see Decision 2).

## Design Decisions

### [1] No spec domain; record the constraint in mission.md and enforce it in the workflow

- **Decision:** Add no feature spec and no `release/` or `packaging/` domain. Record the glibc 2.28 Linux floor in `specs/mission.md` § Constraints and enforce it with a build-time guard step in `auto-tag.yml`.
- **Alternatives:** Create a `release/` domain with a Gherkin scenario asserting the shipped binaries link against glibc ≤ 2.28.
- **Rationale:** The spec library describes observable CLI and library behavior verified by cargo integration tests. A glibc-linkage ceiling on release artifacts is a non-functional build-toolchain property with no CLI-observable behavior and no natural cargo test — its only honest verification is a shell inspection of the artifact in CI. A scenario that maps only to a CI shell check would pollute the behavior-focused library. The recorded plan `change-exarrow-rs-bump-0.12.3` set the precedent of `Features: (none)` for a pure infra change. mission.md § Constraints already owns distribution and platform constraints, making it the correct home for the durable requirement; the in-workflow guard is the runtime enforcement that a spec test would otherwise provide.
- **Promotes to ADR:** yes

### [2] Bump exarrow-rs to 0.14.0 as a separate follow-up, not in this plan

- **Decision:** Keep `exarrow-rs` pinned at 0.13.0. The 0.13.0 → 0.14.0 bump is an explicit non-goal of this plan and is left to its own plan.
- **Alternatives:** Fold the 0.14.0 bump into this plan so both ship in the next release.
- **Rationale:** This plan is a build-infra refactor with no behavior change (issue labeled `refactoring`). A minor dependency bump (0.13 → 0.14) can carry behavior changes that need their own test-validation pass and possibly spec review; coupling it here muddies the no-behavior-change framing. The repo already plans exarrow bumps as dedicated plans (`change-exarrow-rs-bump-0.12.3`). The workspace rule "a new exarrow-rs release must trigger a new exapump release" is satisfied by a separate follow-up release; it does not require the same release as the glibc fix.
- **Promotes to ADR:** no

### [3] Conditional job container over splitting the build job

- **Decision:** Keep one `build-binaries` matrix and set `container: ${{ matrix.container }}`, using an empty string to disable the container on macOS and Windows. Gate toolchain-install and the glibc guard per entry with `if: matrix.container ...`.
- **Alternatives:** Split into a Linux-in-container job and a separate macOS/Windows job.
- **Rationale:** The conditional-container pattern keeps one job definition, one artifact-upload path, and the smallest diff. An empty `container` string is treated by GitHub Actions as no container, which is the documented way to vary containers across a matrix.
- **Promotes to ADR:** no

### [4] rustup inside the container; native per-arch build; no cross-compilation

- **Decision:** Install the Rust toolchain via rustup inside the AlmaLinux 8 container and build natively on the matching runner architecture (`ubuntu-latest` for x86_64, `ubuntu-24.04-arm` for aarch64). Keep the `gnu` target triples unchanged.
- **Alternatives:** `dtolnay/rust-toolchain` inside the container; cross-compilation with QEMU or a cross linker; static musl target.
- **Rationale:** The toolchain action targets Ubuntu runners and is unreliable on minimal non-Ubuntu images. Native per-arch builds avoid QEMU and cross linkers and keep the artifact identical to today apart from the glibc floor. musl would change the shipped ABI and target triple — a larger change than the issue requires.
- **Promotes to ADR:** no

## Review Findings

<!-- Populated in Revision Mode after plan-reviewer blockers, and by speq-implement after code review. -->
