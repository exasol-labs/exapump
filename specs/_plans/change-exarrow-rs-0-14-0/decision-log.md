# Decision Log: change-exarrow-rs-0-14-0

## Interview

Headless plan (`speq-plan-pr`). No live interview. The brief is GitHub issue exasol-labs/exapump#34; the planner made the conventional calls below and documented them.

## Design Decisions

### [1] No feature spec for the dependency bump

- **Decision:** Add no feature spec and no domain. `Features: (none)`.
- **Alternatives:** Add a `connection/` scenario asserting query behavior under 0.14.0. Rejected — the bump introduces no CLI-observable behavior and no new cargo-testable behavior; a scenario would restate existing coverage.
- **Rationale:** Matches the recorded `change-exarrow-rs-bump-0.12.3` precedent and decision 004. The spec library stays scoped to CLI/library behavior. The existing suite is the regression proof.
- **Promotes to ADR:** no

### [2] Patch version bump: 0.11.3 → 0.11.4

- **Decision:** Bump exapump to 0.11.4 (patch).
- **Alternatives:** Minor bump (0.12.0). Rejected — a minor bump signals a new exapump capability; 0.14.0 adds none that exapump exposes.
- **Rationale:** exapump exposes no query-timeout surface (no CLI flag, no config key), so 0.14.0's timeout change is invisible at the exapump boundary. Per Conventional Commits, a behavior-preserving internal dependency bump is a patch. The one library-level behavioral delta — exarrow-rs no longer imposes a hidden client-side ~300s/120s query timeout, deferring to the server's `QUERY_TIMEOUT` — removes an undocumented ceiling rather than adding an exapump feature.
- **Promotes to ADR:** no

### [3] Breaking-change assessment: exapump consumes none of the changed API

- **Decision:** Apply the bump with no exapump source changes.
- **Alternatives:** Migrate exapump call sites for the new `Option<Duration>`/`Option<u64>` signatures. Rejected — exapump has no such call sites.
- **Rationale:** 0.14.0's sole breaking change is `ConnectionParams::query_timeout: Option<Duration>` and `Statement::timeout_ms() -> Option<u64>`. exapump connects only via the DSN-string path (`Driver::new`, `driver.open`, `db.connect` in `src/connection.rs`) and never constructs `ConnectionParams`, calls `create_statement`, or reads `timeout_ms()`. All other consumed symbols — `ParquetImportOptions`, `CsvImportOptions`, `infer_schema_from_csv`/`infer_schema_from_parquet`, `ColumnNameMode`, `InferredTableSchema`, `CsvInferenceOptions`, `ParquetCompression`, and the `QueryError::SyntaxError`/`ExecutionFailed` variants — are unchanged in the 0.14.0 source. Compilation against 0.14.0 is the enforcing check (Task 2).
- **Promotes to ADR:** no

### [4] Keep the `["native", "websocket"]` feature set

- **Decision:** Retain `features = ["native", "websocket"]`.
- **Alternatives:** Drop `websocket`. Rejected — out of scope; exapump's `--transport websocket` flag depends on it, and 0.14.0 does not change exarrow-rs feature flags.
- **Rationale:** Preserve existing capability; a feature-flag change is a separate decision.
- **Promotes to ADR:** no

## Review Findings

<!-- Populated in Revision Mode after plan-reviewer blockers, and by speq-implement after code review. -->
