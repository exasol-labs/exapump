# Decisions: fix-json-import-review-findings

## ADR: exapump renders its own CREATE TABLE and maps an integer to DECIMAL(19,0)

**ID:** exapump-renders-create-table-decimal19
**Plan:** fix-json-import-review-findings
**Status:** Accepted

### Context

`classify_value` in `json_tables_core` admits every 64-bit signed integer into an integer column, but the upstream `column_sql_type` maps that integer type to `DECIMAL(18,0)`, which rejects a 19-digit value. No wider type is reachable through the upstream mapping, because a value outside the 64-bit signed range is already classified as a fractional number and lands in `DOUBLE`. exapump already rewrote the upstream `CREATE TABLE` statement by prefix match before this plan, so the DDL boundary had already moved away from `json_tables_core`.

### Decision

exapump stops calling `json_tables_core::ddl::build_sql_schema` and renders the `CREATE TABLE IF NOT EXISTS` text from `PlannedTable::columns`. exapump maps a contract type to an Exasol type itself, and maps `SimpleType::Integer` to `DECIMAL(19,0)`. `json_tables_core` keeps ownership of which tables exist, which columns each table carries, and which contract type each column has.

### Options Considered

| Option | Verdict |
|--------|---------|
| exapump renders its own DDL and maps integer to `DECIMAL(19,0)` | ✓ Chosen — removes the rewrite error path and makes column order correct by construction |
| Fix `column_sql_type` in `exasol-labs/exasol-json-tables` and bump the pinned tag | ✗ Rejected — blocks the fix on another repository's release, which the `reuse-json-tables-core-via-git-dependency` ADR already rejected as a dependency strategy |
| Document the 18-digit limit in the spec and change no code | ✗ Rejected — leaves a defect that produces `ETL-3050 numeric value out of range` after the tables already exist |

### Consequences

`rewrite_create`, `build_ddl`, the `TableFamily::ddl` field, and the two SQL-parsing test helpers are deleted. exapump now diverges from upstream on one type width. A future tag bump MUST NOT restore `DECIMAL(18,0)` by re-adopting `column_sql_type`. A table an earlier exapump version created keeps `DECIMAL(18,0)`, because `CREATE TABLE IF NOT EXISTS` alters nothing.
