# Plan: fix-glibc-compatibility

## Summary

Build the two Linux release binaries inside an AlmaLinux 8 container so they link against glibc 2.28 instead of the Ubuntu 24.04 runner's glibc 2.39. This lets `exapump` run on SLES 15 SP7 and other enterprise distributions with older glibc, closing issue #32.

## Design

### Context

Issue #32 reports `exapump: /lib64/libc.so.6: version 'GLIBC_2.39' not found (required by exapump)` on SLES 15 SP7. Root cause: `.github/workflows/auto-tag.yml`'s `build-binaries` job builds both Linux targets (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`) natively on Ubuntu 24.04 runners with plain `cargo build --release`. Ubuntu 24.04 ships glibc 2.39, so the resulting binaries reference glibc 2.39 symbols and refuse to start on any host with an older glibc. Most enterprise and LTS distributions ship glibc older than 2.39.

A dynamically linked glibc binary runs on any host whose glibc is at least as new as the glibc it was built against. Lowering the build-time glibc to 2.28 widens the supported host range to cover SLES 15 SP7 and equivalents.

- **Goals** — shipped Linux binaries reference glibc symbols no newer than 2.28; a release-time guard fails the build if that ceiling regresses; macOS and Windows builds stay unchanged.
- **Non-Goals** — bumping `exarrow-rs` (stays 0.13.0; see Decision 2 in the decision log); changing `ci.yml`; supporting glibc older than 2.28; switching to fully static musl binaries.

### Decision

Wrap only the two Linux matrix entries in an `almalinux:8` job container. AlmaLinux 8.10 ships glibc 2.28, the same baseline the Python ecosystem standardized as `manylinux_2_28`. The build stays a native `cargo build --release --target <triple>` on the matching runner architecture (x86_64 on `ubuntu-latest`, aarch64 on `ubuntu-24.04-arm`); the container only changes which glibc the linker resolves against. No cross-compilation, no QEMU, no target-triple change.

#### Architecture

```
build-binaries matrix
├─ x86_64-unknown-linux-gnu   runs-on ubuntu-latest      container almalinux:8  → glibc 2.28
├─ aarch64-unknown-linux-gnu  runs-on ubuntu-24.04-arm   container almalinux:8  → glibc 2.28
├─ x86_64-apple-darwin        runs-on macos-latest        container "" (none)   → unchanged
├─ aarch64-apple-darwin       runs-on macos-latest        container "" (none)   → unchanged
└─ x86_64-pc-windows-msvc     runs-on windows-latest      container "" (none)   → unchanged
```

The job sets `container: ${{ matrix.container }}`. An empty string disables the container, so macOS and Windows entries run directly on their runners. Toolchain-install and the glibc guard are gated per entry with `if: matrix.container != ''` (container path) and `if: matrix.container == ''` (native path).

#### Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| Conditional job container via `matrix.container` | `build-binaries` job | One matrix builds Linux-in-container and macOS/Windows-on-runner without splitting the job |
| rustup install inside container | Linux container step | `dtolnay/rust-toolchain` is unreliable on non-Ubuntu minimal images; rustup is the documented, robust path |
| glibc-ceiling guard at build time | Linux container step | Fails the release the moment a future change lifts the glibc requirement above 2.28 — the durable regression guard |

### Consequences

| Decision | Alternatives Considered | Rationale |
|----------|------------------------|-----------|
| Build Linux targets in `almalinux:8` (glibc 2.28) | Static musl target (`*-linux-musl`) | musl changes the shipped ABI and target triple, needs musl-specific handling for TLS/crypto crates, and alters what users download. AlmaLinux 8 delivers an older glibc floor with zero target or ABI change. |
| Plain `almalinux:8` image | `manylinux_2_28` image; `cargo-zigbuild` with `gnu.2.28` target | `manylinux_2_28` is the same AlmaLinux 8 glibc baseline but heavier and Python-oriented; `cargo-zigbuild` adds a zig toolchain dependency. Plain `almalinux:8` is the minimal, self-documenting expression of the glibc 2.28 floor. |
| rustup inside the container | `dtolnay/rust-toolchain@stable` inside the container | The action targets Ubuntu runners and is unreliable on minimal non-Ubuntu images; rustup is the supported install path there. macOS and Windows keep the action. |
| Keep `gnu` target and native arch | Cross-compile from one runner | Native per-arch builds need no QEMU or cross linker and keep the artifact identical to today apart from the glibc floor. |

## Features

| Feature | Status | Spec |
|---------|--------|------|
| (none) | n/a | No spec scenarios added or modified — release/build-pipeline change only |

This plan changes no user-observable CLI or library behavior, so it adds no feature spec. This follows the recorded precedent of `change-exarrow-rs-bump-0.12.3`, which also carried `Features: (none)`. The durable distribution constraint (glibc 2.28 Linux floor) is recorded in `specs/mission.md` § Constraints (task 3.1) and enforced by the in-workflow guard (task 1.4), not by a feature scenario. See decision log Decision 1 for why no `release/` spec domain is created.

## Dependencies

No dependency changes. `exarrow-rs` stays pinned at `0.13.0` with features `["native", "websocket"]`. The bump to `0.14.0` is an explicit follow-up (decision log Decision 2).

## Implementation Tasks

### Group A — Rework the Linux build path in `auto-tag.yml`

- [ ] 1.1 Add a `container` key to each `build-binaries` matrix entry: `almalinux:8` for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`; `""` for the two macOS entries and the Windows entry. Add `container: ${{ matrix.container }}` at the job level (below `runs-on`).
- [ ] 1.2 Gate the existing `dtolnay/rust-toolchain@stable` step with `if: matrix.container == ''`. Add a container-only step (`if: matrix.container != ''`) that runs before `actions/checkout` is relied upon for the toolchain: install build prerequisites with `dnf install -y gcc git binutils curl ca-certificates` (add any `-devel` package the build turns out to need), install the Rust stable toolchain via rustup for `${{ matrix.target }}`, and append `$HOME/.cargo/bin` to `$GITHUB_PATH`.
- [ ] 1.3 Remove the dead `cross` matrix field and its two gated steps ("Install cross", "Build (cross)"), leaving one shared `cargo build --release --target ${{ matrix.target }}` step used by every entry. See Dead Code Removal.
- [ ] 1.4 Add a glibc-ceiling guard step after the build, gated `if: matrix.container != ''`: read the maximum required GLIBC symbol version from the built binary (`objdump -T` or `readelf -V`), and `exit 1` if it exceeds `2.28`. Echo the detected ceiling so the log records it.

### Group B — Record the distribution constraint

- [ ] 3.1 Update `specs/mission.md` § Constraints to state the Linux glibc floor: Linux release binaries target glibc 2.28 (AlmaLinux 8 / manylinux_2_28 baseline) so they run on enterprise distributions such as SLES 15 SP7.

No task warrants an `[expert]` tag. All work is CI-workflow configuration and a documentation edit — no concurrency, algorithm, or cross-file behavioral coupling. Per the planning guardrails, config changes are not expert-tagged.

## Parallelization

| Parallel Group | Tasks |
|----------------|-------|
| Group A | 1.1, 1.2, 1.3, 1.4 (same file, apply as one edit sequence) |
| Group B | 3.1 |

Group A and Group B are independent (different files) and MAY run concurrently.

## Dead Code Removal

| Type | Location | Reason |
|------|----------|--------|
| Matrix field | `.github/workflows/auto-tag.yml` `build-binaries` `matrix.cross` | Every entry sets `cross: false`; the field is never true |
| Step | `.github/workflows/auto-tag.yml` "Install cross" (`if: matrix.cross`) | Guard is always false — dead |
| Step | `.github/workflows/auto-tag.yml` "Build (cross)" (`if: matrix.cross`) | Guard is always false — dead; native build step covers all entries |

## Verification

### Scenario Coverage

| Scenario | Test Type | Test Location | Test Name |
|----------|-----------|---------------|-----------|
| (none) | n/a | n/a | No spec scenarios — see Features table |

The change is verified two ways: the in-workflow glibc-ceiling guard (task 1.4) fails any release whose Linux binary references glibc newer than 2.28, and the existing integration suite under `tests/` re-runs unchanged to confirm behavior is untouched.

### Manual Testing

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| Linux build in glibc-2.28 userspace | `docker run --rm -v "$PWD":/w -w /w almalinux:8 bash -c 'dnf install -y gcc git binutils curl ca-certificates >/dev/null 2>&1 && curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --target x86_64-unknown-linux-gnu >/dev/null && source "$HOME/.cargo/env" && cargo build --release --target x86_64-unknown-linux-gnu'` | Build exits 0 |
| glibc ceiling of the built binary | `objdump -T target/x86_64-unknown-linux-gnu/release/exapump \| grep -oE 'GLIBC_[0-9.]+' \| sort -V \| tail -1` | `GLIBC_2.28` or lower |
| Runs under old-glibc userspace | `docker run --rm -v "$PWD/target/x86_64-unknown-linux-gnu/release":/b almalinux:8 /b/exapump --version` | Prints version, exit 0, no `GLIBC_2.39 not found` |
| Workflow syntax | `actionlint .github/workflows/auto-tag.yml` | 0 errors |

### Checklist

| Step | Command | Expected |
|------|---------|----------|
| Build | `cargo build` | Exit 0 |
| Test | `cargo test` (against Exasol Docker DB on `localhost:8563`, run with `dangerouslyDisableSandbox: true`) | 0 failures |
| Lint | `cargo clippy` | 0 errors / 0 warnings |
| Format | `cargo fmt --check` | No changes |
