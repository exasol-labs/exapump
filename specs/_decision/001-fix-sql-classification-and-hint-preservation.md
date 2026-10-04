# Decisions: fix-sql-classification-and-hint-preservation

## ADR: Preserve original SQL byte-for-byte to Exasol; strip comments only for classification

**ID:** preserve-sql-strip-comments-classification-only
**Plan:** fix-sql-classification-and-hint-preservation
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
