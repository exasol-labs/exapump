# Decisions: fix-sql-classification-and-hint-preservation

## ADR: Preserve original SQL byte-for-byte to Exasol; strip comments only for classification

**ID:** preserve-sql-strip-comments-classification-only
**Plan:** fix-sql-classification-and-hint-preservation
**Status:** Accepted

### Context

Exasol optimizer hints such as `/*snapshot execution*/` live in SQL comments. Rewriting the SQL before execution drops them.

### Decision

exapump sends the user's SQL to Exasol unchanged. It strips comments only on a private copy, to classify the statement type.

### Options Considered

| Option | Verdict |
|--------|---------|
| Strip comments on a copy for classification only | ✓ Chosen — preserves hints |
| Strip comments before execution | ✗ Rejected — drops Exasol hints |
| Detect hints heuristically and keep only those | ✗ Rejected — no hint catalog can cover every directive |
