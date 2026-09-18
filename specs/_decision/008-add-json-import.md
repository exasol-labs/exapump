# Decisions: add-json-import

## ADR: Reuse json_tables_core through a git dependency pinned to tag v0.3

**ID:** reuse-json-tables-core-via-git-dependency
**Plan:** add-json-import
**Status:** Accepted

### Context

`json_tables_core` is a pure normalization engine whose only dependency is `serde_json`, with no Exasol, Arrow, or CLI coupling. Neither `json_tables_core` nor `json_to_parquet` is published to crates.io. No Cargo.toml in the upstream repository sets `publish`, and no workflow publishes to crates.io, so reuse needs a git dependency, an upstream crates.io release, or vendoring. `.github/workflows/auto-tag.yml` builds exapump release binaries and uploads them to a GitHub release; no workflow runs `cargo publish`, so the crates.io restriction on git dependencies does not apply to exapump. The upstream repository is public, so CI fetches it anonymously and no confidentiality rule applies.

### Decision

Add `json_tables_core = { git = "https://github.com/exasol-labs/exasol-json-tables", tag = "v0.3" }` to exapump's `Cargo.toml`.

### Options Considered

| Option | Verdict |
|--------|---------|
| Add a git dependency pinned to tag `v0.3` | ✓ Chosen — blocks no exapump distribution path; task 1.1 confirms the dependency resolves before anything else is built |
| Vendor the normalization logic into exapump | ✗ Rejected — forks the exact logic the user asked to reuse and guarantees drift |
| Block the plan until `json_tables_core` is published to crates.io | ✗ Rejected — makes this plan wait on an external repository's release |

### Consequences

exapump's build depends on the availability of `github.com/exasol-labs/exasol-json-tables` at tag `v0.3`. Getting `json_tables_core` published to crates.io stays a worthwhile follow-up outside this plan.

## ADR: Reuse json_tables_core only, not json_to_parquet

**ID:** reuse-json-tables-core-not-json-to-parquet
**Plan:** add-json-import
**Status:** Accepted

### Context

`json_to_parquet` exposes one public function whose `Args` fields are private, so a caller cannot construct them. It opens its own connection with `Driver::open(url)` from a raw URL, which bypasses exapump's DSN, profile, certificate-fingerprint, and transport resolution in `src/connection.rs`. It prints its own progress format. Consuming it would put two connection layers and two output conventions in one binary. The upstream crate's own documentation states that anything an alternative loader also needs belongs in the core crate, which is the boundary this decision follows.

### Decision

Depend on `json_tables_core` only. Reimplement the file reading, connection handling, and Exasol import inside exapump.

### Options Considered

| Option | Verdict |
|--------|---------|
| Depend on `json_tables_core` and reimplement I/O inside exapump | ✓ Chosen — keeps one connection layer and one output convention in the binary |
| Add `json_to_parquet` as a second dependency and call its `run(Args)` | ✗ Rejected — its private `Args` fields, raw-URL connection, and own progress format bypass exapump's connection and output conventions |

### Consequences

exapump owns file reading, connection handling, the Arrow conversion, and the import. `json_tables_core` owns every decision about which tables exist, which columns they carry, and which DDL describes them.
