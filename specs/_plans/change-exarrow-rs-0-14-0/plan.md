# Plan: change-exarrow-rs-0-14-0

## Summary

Bump the `exarrow-rs` dependency from 0.13.0 to 0.14.0 and raise exapump to 0.11.4. The bump is dependency-only: exapump consumes none of the API that 0.14.0 changed, so no exapump code changes and no user-facing behavior change.

## Context

Issue #34 tracks the workspace release-chain rule: a new exarrow-rs release triggers a new exapump release. exarrow-rs 0.14.0 (published 2026-07-16) carries a single breaking change — `ConnectionParams::query_timeout` becomes `Option<Duration>` and `Statement::timeout_ms()` returns `Option<u64>`, and the client-side query-timeout timer is removed in favor of a server-enforced `queryTimeout` session attribute.

exapump does not touch that surface. It connects only through the DSN-string path (`Driver::new` → `driver.open(dsn)` → `db.connect()` in `src/connection.rs`) and never constructs `ConnectionParams`, calls `create_statement`, or reads `timeout_ms()`. Its other consumed types — `ParquetImportOptions`, `CsvImportOptions`, `types::infer_schema_from_csv`/`infer_schema_from_parquet`, `ColumnNameMode`, `InferredTableSchema`, `CsvInferenceOptions`, `ParquetCompression`, and the `QueryError::SyntaxError`/`ExecutionFailed` variants — are all unchanged in 0.14.0. The bump therefore compiles and behaves identically with zero code changes.

This change follows the recorded `change-exarrow-rs-bump-0.12.3` precedent and decision 004 (`specs/_decision/004-fix-glibc-compatibility.md`): a non-CLI-observable dependency bump adds no feature spec.

## Features

No feature specs. This is a dependency bump with no CLI-observable behavior change; there is no new scenario to specify and no existing scenario to alter.

| Feature | Status | Spec |
|---------|--------|------|
| (none) | — | — |

## Dependencies

| Dependency | Current | New |
|------------|---------|-----|
| exarrow-rs (crates.io) | 0.13.0, features `["native", "websocket"]` | 0.14.0, features `["native", "websocket"]` |

The feature set stays `["native", "websocket"]`; 0.14.0 does not change exarrow-rs feature flags.

## Migration

None. No configuration, DSN parameter, CLI flag, or on-disk format changes.

## Implementation Tasks

1. Set `exarrow-rs = { version = "0.14.0", features = ["native", "websocket"] }` in `Cargo.toml` and update `Cargo.lock` to resolve exarrow-rs 0.14.0.
2. Build against 0.14.0 and confirm exapump compiles with no source changes; confirm exapump consumes none of the changed `query_timeout`/`timeout_ms` API. Escalate to API migration only if compilation fails. [expert]
3. Run `cargo clippy` and `cargo fmt --check`; resolve any new warnings.
4. Run the full test suite (`cargo test`) against a live Exasol Docker container; confirm zero failures.
5. Bump the exapump package `version` in `Cargo.toml` from 0.11.3 to 0.11.4.

## Parallelization

| Parallel Group | Tasks |
|----------------|-------|
| Group A | Task 1 |
| Group B | Task 2 |
| Group C | Task 3, Task 4 |
| Group D | Task 5 |

Sequential dependencies:
- Group A → Group B → Group C → Group D

## Dead Code Removal

None. No exapump code is added, changed, or made obsolete by this bump.

## Verification

### Scenario Coverage

This bump adds no scenarios and changes no scenario. The existing integration suite (`tests/*.rs`, 45+ tests spanning upload, export, sql, bucketfs, interactive, profile, wait, transport) is the regression proof: every scenario that passed against exarrow-rs 0.13.0 MUST still pass against 0.14.0. A green run of the full suite against a live Exasol database confirms the bump is behavior-preserving.

### Manual Testing

Prerequisite: a running Exasol container (`exasol/docker-db:2025.2.0`, port 8563).

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| SQL execution | `exapump sql 'SELECT 1 AS n' --dsn 'exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0'` | Result table printing `n` = 1, exit 0 |
| Upload | `exapump upload tests/fixtures/*.csv --table TEST.SMOKE --dsn 'exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0'` | Rows loaded, exit 0 |
| Export | `exapump export --query 'SELECT 1' --output /tmp/out.csv --format csv --dsn 'exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0'` | `/tmp/out.csv` written, exit 0 |

### Checklist

| Step | Command | Expected |
|------|---------|----------|
| Build | `cargo build` | Exit 0 |
| Test | `cargo test` | 0 failures (fails, not skips, if Exasol is unavailable) |
| Lint | `cargo clippy` | 0 warnings |
| Format | `cargo fmt --check` | No changes |
