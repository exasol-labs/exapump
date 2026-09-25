# Decisions: add-json-import

## ADR: Reuse json_tables_core via a git dependency, not vendoring or a crates.io release

**ID:** reuse-json-tables-core-via-git-dependency
**Plan:** add-json-import
**Status:** Accepted

### Context

`json_tables_core` is a pure normalization engine with no Exasol, Arrow, or CLI coupling. It is not published to crates.io. exapump ships as GitHub release binaries and never runs `cargo publish`, so a git dependency imposes no distribution constraint.

### Decision

Depend on `json_tables_core` as a git dependency, pinned to a tag, and bump that pin as a routine maintenance action rather than an architectural one.

### Options Considered

| Option | Verdict |
|--------|---------|
| Git dependency, pinned to a tag | ✓ Chosen — no distribution constraint; a normal, updatable pin |
| Vendor the normalization logic into exapump | ✗ Rejected — forks the logic the plan asked to reuse and guarantees drift |
| Block on an upstream crates.io release | ✗ Rejected — makes exapump wait on another repository's release |

### Consequences

exapump's build depends on the upstream repository's availability. Getting `json_tables_core` published to crates.io stays a worthwhile follow-up outside this plan.

## ADR: Reuse json_tables_core only, not json_to_parquet

**ID:** reuse-json-tables-core-not-json-to-parquet
**Plan:** add-json-import
**Status:** Accepted

### Context

`json_to_parquet` opens its own Exasol connection from a raw URL, bypassing exapump's DSN, profile, certificate-fingerprint, and transport resolution, and prints its own progress format. Consuming it would put two connection layers and two output conventions in one binary.

### Decision

Depend on `json_tables_core` only. Reimplement the file reading, connection handling, and Exasol import inside exapump.

### Options Considered

| Option | Verdict |
|--------|---------|
| Depend on `json_tables_core` and reimplement I/O inside exapump | ✓ Chosen — keeps one connection layer and one output convention in the binary |
| Add `json_to_parquet` as a second dependency and call its `run(Args)` | ✗ Rejected — its private `Args` fields, raw-URL connection, and own progress format bypass exapump's connection and output conventions |

### Consequences

exapump owns file reading, connection handling, the Arrow conversion, and the import. `json_tables_core` owns every decision about which tables exist, which columns they carry, and which DDL describes them.
