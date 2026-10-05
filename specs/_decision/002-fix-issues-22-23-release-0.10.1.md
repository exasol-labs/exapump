# Decisions: fix-issues-22-23-release-0.10.1

## ADR: Auto-detect CREATE … SCRIPT bodies in the statement splitter via a ScriptBody state

**ID:** auto-detect-script-bodies-scriptbody-state
**Plan:** fix-issues-22-23-release-0.10.1
**Status:** Accepted

### Context

A `CREATE … SCRIPT … AS` body contains semicolons that must not split the statement.

### Decision

The splitter detects `CREATE … SCRIPT … AS` and treats everything up to a line containing only `/` as one statement. `FUNCTION … END` blocks are not handled.

### Options Considered

| Option | Verdict |
|--------|---------|
| Detect script bodies automatically | ✓ Chosen — needs no user action |
| Add a `--no-split` flag | ✗ Rejected — every invocation must change |
| Require a configurable delimiter | ✗ Rejected — the exaplus `/` convention is standard |
