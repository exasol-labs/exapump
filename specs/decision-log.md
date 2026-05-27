# Architecture Decision Records

<!-- ADRs are numbered sequentially starting from ADR-001. Never renumber. -->
<!-- recorder-agent appends new ADRs from plan decision logs. -->

---

## ADR-001: Preserve original SQL byte-for-byte to Exasol; strip comments only for classification

**Date:** 2026-05-21
**Plan:** `fix-sql-classification-and-hint-preservation`
**Status:** Accepted

### Context

SQL statements containing leading or inline comments (e.g. `/*snapshot execution*/`, `-- hint`) were being silently rewritten before reaching Exasol. The `strip_comments` pre-pass in `sql.rs::run` removed comments to enable statement-type classification, but this also dropped Exasol optimizer hints that users intentionally include in their SQL. The same pre-pass prevented the comment-aware splitter from being needed, but meant the split result was already mutated.

### Decision

Remove the `strip_comments(&sql_input)` pre-pass in `sql.rs::run`. Make `split_statements` comment-aware so it can find top-level semicolons without mutating the input. Call `strip_comments` inside `StatementType::from_sql` only, keeping comment stripping as a local, side-effect-free transformation used solely for keyword extraction.

### Options Considered

| Option | Verdict |
|--------|---------|
| Strip comments inside `StatementType::from_sql` only; pass original SQL to Exasol | ✓ Chosen — preserves hints, fixes the root cause |
| Keep `strip_comments` pre-pass in `sql.rs::run` | ✗ Rejected — silently drops `/*snapshot execution*/` and similar Exasol hints, defeating user intent |
| Detect hints heuristically and preserve only those | ✗ Rejected — fragile; a hint catalog cannot anticipate every Exasol-side directive |

### Consequences

The principle "what the user wrote is what Exasol sees" is enforced by the architecture. `split_statements` now requires a comment-aware four-state scanner to correctly identify top-level semicolons without rewriting input. All downstream consumers of `split_statements` receive the original statement text.

---

## ADR-002: Add `StatementType::Execute` variant and dispatch at runtime on `result_set.row_count()`

**Date:** 2026-05-21
**Plan:** `fix-sql-classification-and-hint-preservation`
**Status:** Accepted

### Context

`EXECUTE SCRIPT` statements were falling into the `Ddl` arm of the statement dispatcher, which calls `execute_update()`. When the script is defined with `RETURNS TABLE`, exarrow-rs's `execute_update` requires the server to return a row count — which fails because the script returns a result set instead. The statement type cannot be determined statically: whether a script returns rows depends on its server-side definition.

### Decision

Introduce a new `Execute` variant in `StatementType` mapped from the `EXECUTE` keyword. In both `sql.rs::run` and `interactive.rs::execute_statement`, call `conn.execute(stmt)` for the `Execute` arm and branch on `result_set.row_count().is_some()`: if `Some`, print `OK`; otherwise `fetch_all` and render via the same code path as the `Query` arm.

### Options Considered

| Option | Verdict |
|--------|---------|
| Add `StatementType::Execute` variant; runtime-branch on `row_count()` | ✓ Chosen — minimal-blast-radius change; existing Dml/Ddl paths stay intact |
| Always use `conn.execute` for every statement and branch on `row_count()` globally | ✗ Rejected — Dml/Ddl rely on the existing row-count contract for status-line formatting; a wholesale switch risks regressing those messages |
| Inspect SQL for `RETURNS TABLE` at the call site to determine dispatch | ✗ Rejected — script body is server-side; cannot know what it returns without executing |

### Consequences

`EXECUTE SCRIPT` is modelled as its own polymorphic variant, reflecting that it is genuinely the ambiguous case in Exasol SQL. The existing `Query`, `Dml`, and `Ddl` paths are unchanged. Both runners (`sql.rs` and `interactive.rs`) share the same branching logic.
