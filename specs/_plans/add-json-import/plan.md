# Plan: add-json-import

## Summary

Add JSON and NDJSON ingest to `exapump upload`, turning one document set into a relational table family (root table plus one subtable per nested object or array path). exapump reuses the `json_tables_core` crate from `exasol-labs/exasol-json-tables` for normalization and keeps ownership of file reading, connection handling, and the Exasol import.

## Design

### Context

`exapump upload` today loads exactly one file into exactly one table. `src/commands/upload.rs` matches on `(FileFormat, dry_run)`, infers a single `InferredTableSchema` through exarrow-rs, emits one `CREATE TABLE IF NOT EXISTS`, and calls one import function. JSON does not fit that model. A document set with nested objects and arrays has no single relational shape, so an importer must derive a family of tables and the key columns that link them.

`exasol-labs/exasol-json-tables` already solves that derivation. Its `json_tables_core` crate is a pure normalization engine whose only dependency is `serde_json`. It reads JSON or NDJSON, collects property and type statistics, plans a table family, emits Exasol DDL, and writes rows into a caller-supplied sink. It carries no Exasol, Arrow, or CLI coupling by design.

The reuse question is which part of that repository exapump consumes. The sibling crate `json_tables_ingest` (package `json_to_parquet`) is the local-file CLI. It exposes only `run(Args)`, its `Args` fields are private, and it opens its own Exasol connection from a raw URL. Consuming it would bypass exapump's DSN, profile, certificate-fingerprint, and transport resolution, and would put two competing stdout formats in one binary.

- **Goals**
  - Accept `.json` and `.ndjson` files in `exapump upload`, with framing detected from file content.
  - Create the full table family and load every document, nested object, and array element.
  - Preview the planned family and its DDL under `--dry-run` without connecting.
  - Reuse `json_tables_core` rather than reimplement normalization inside exapump.
  - Keep exapump's existing connection, profile, and output conventions unchanged.
- **Non-Goals**
  - No `json_tables_udf` reuse. exapump is a client-side CLI, not a UDF host.
  - No primary-key or foreign-key constraint statements. See Consequences.
  - No provenance `COMMENT ON TABLE` statements and no source-manifest artifact.
  - No `--schema-sql` or `--manifest-output` file artifacts. `--dry-run` covers schema preview.
  - No atomicity across the table family. See Consequences.
  - No parallel per-table import. The existing single-connection import model stays.
  - No multi-file or glob JSON input. `upload` reads `args.files[0]` today and keeps doing so.
  - No `.jsonl` extension. See Consequences.
  - No chunking and no spill to disk. Peak memory scales with the input file size. See Consequences.

### Decision

Add a git dependency on `json_tables_core` pinned to the `v0.3` tag. Add a `FileFormat::Json` arm to `src/format.rs`. Add a new module `src/json_tables.rs` that owns the whole JSON path behind a three-item interface, and give `src/commands/upload.rs` two thin arms that call it.

`src/json_tables.rs` hides serde_json, the table family, the Arrow conversion, the DDL, and the schema qualification rule. Its interface is:

```rust
/// A planned table family derived from one JSON or NDJSON file.
pub struct TableFamily { /* plans, stem, schema, ddl */ }

/// Scans `path` and derives the table family for the target `table`.
/// Performs no database I/O.
pub fn plan_family(path: &Path, table: &str) -> anyhow::Result<TableFamily>;

impl TableFamily {
    /// Table names, their columns, and the planned CREATE statements, for --dry-run.
    pub fn describe(&self) -> String;
}

/// Creates the family's tables and loads every document from `path`.
///
/// The returned vector holds the row count loaded per table, in family order.
/// On a failure it holds every table loaded before the error. On success it
/// holds every table of the family. The returned option holds the failure.
pub async fn load(
    family: &TableFamily,
    path: &Path,
    conn: &mut exarrow_rs::Connection,
) -> (Vec<(String, u64)>, Option<anyhow::Error>);
```

Family order is the order of the `build_all_schema_plans` output, which `StatsCollector::finish` sorts by table path (`json_tables_core/src/infer.rs:81-85`). `TableFamily` stores that vector as `plans` and never reorders it. The sort key is `TablePath::to_string()`, which renders the root path as the literal `root` and a nested path as its dotted segments. The root table therefore sorts among the subtables by that word, not first. A family of root, `customer`, and `items[]` orders as `customer`, `items[]`, `root`. Every step that walks the family walks `family.plans`: the CREATE statements, the imports, and the row-count report. `ColumnBuffers` stores its tables in a `HashMap<TablePath, TableBuffer>`, so `ColumnBuffers::tables()` yields a per-process random order and no code path in this feature calls it. Buffers are resolved by key instead, through `ColumnBuffers::table(&plan.path)`.

#### Architecture

```
exapump upload data.json --table SALES.ORDERS
   │
   ▼
src/format.rs  detect_from_path  ──▶ FileFormat::Json
   │
   ▼
src/commands/upload.rs  (dispatch only)
   │
   ▼
┌──────────────────────── src/json_tables.rs ────────────────────────┐
│ plan_family: pass 1                                                │
│   detect_format ─▶ for_each_document ─▶ StatsCollector             │
│   build_all_schema_plans ─▶ Vec<PlannedTable>                      │
│   build_sql_schema ─▶ CREATE statements                            │
│                                                                    │
│ load: pass 2                                                       │
│   for_each_document ─▶ write_document ─▶ ColumnBuffers             │
│   TableBuffer ─▶ arrow RecordBatch  (exapump-owned bridge)         │
└────────────────────────────────────────────────────────────────────┘
   │                                    │
   ▼                                    ▼
OPEN SCHEMA + CREATE TABLE       import_from_record_batches
(exarrow_rs::Connection)         (exarrow_rs::Connection)
```

Every box inside `src/json_tables.rs` above the dashed line is a `json_tables_core` call. exapump adds the Arrow bridge and the two Exasol calls.

#### Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| Deep module behind three items | `src/json_tables.rs` | Callers never see serde_json, `PlannedTable`, `ColumnBuffers`, or the DDL rules |
| Dependency inversion onto a pure core | `json_tables_core` git dependency | Normalization is stable and I/O-free; exapump is the volatile delivery layer that depends on it |
| Single owner for the naming rule | `src/json_tables.rs` | The mapping from `--table` to schema, stem, and qualified table names exists in exactly one place |
| Arrow RecordBatch import | `load` | Reuses `exarrow_rs::Connection::import_from_record_batches`, so no temporary Parquet file is staged |
| Dispatch-only command arm | `src/commands/upload.rs` | Matches the existing `(FileFormat, dry_run)` shape, so the command file stays a router |

#### Identifier rules

`json_tables_core` quotes every table and column identifier, because JSON keys are case sensitive and can contain characters an unquoted Exasol identifier rejects. exapump's CSV and Parquet paths pass unquoted names, which Exasol folds to uppercase.

To keep one `--table` value produce the same root table across all three formats, `plan_family` uppercases the schema part and the table part of `--table`, then uses the uppercased table part as the `json_tables_core` stem. `--table sales.orders` therefore yields `"SALES"."ORDERS"` and `"SALES"."ORDERS_items_arr"`. The subtable suffix keeps the JSON key case, because two JSON keys may differ only by case.

`json_tables_core::ddl::build_sql_schema` emits unqualified table names. `load` resolves exactly one target schema before it runs any statement. When `--table` carries a schema prefix, the uppercased prefix is the target schema. When `--table` is unqualified, `load` reads the connection's default schema from `exarrow_rs::Connection::params().schema` (`exarrow-rs 0.16.0`, `src/adbc/connection.rs:1044` and `src/connection/params.rs:28`) and uppercases it. When neither supplies one, `load` fails before creating any table, and the error names both places a schema can come from.

`load` then runs `OPEN SCHEMA "<SCHEMA>"` before the CREATE statements, so the unqualified names that `build_sql_schema` emits land in the resolved schema. Every import target is qualified explicitly as `"<SCHEMA>"."<raw name>"`. Both the DDL and the imports therefore name a schema exapump resolved itself, so no statement depends on session state.

`--dry-run` opens no connection and cannot read a default schema. `describe` prints qualified names when `--table` carries a schema prefix, and the unqualified names that `build_sql_schema` emits when it does not.

### Consequences

| Decision | Alternatives Considered | Rationale |
|----------|------------------------|-----------|
| Git dependency on `json_tables_core`, pinned to tag `v0.3` | Vendor the normalization code into exapump; ask the upstream owners to publish to crates.io first | `json_tables_core` is not on crates.io. exapump ships as GitHub release binaries and runs no `cargo publish`, so a git dependency blocks no distribution path. Vendoring forks the logic the user asked to reuse. A crates.io release is another repository's decision and would block this plan on an external release. |
| Depend on `json_tables_core` only, not on `json_to_parquet` | Depend on `json_to_parquet` and call `run(Args)` | `json_to_parquet` exposes one entry point with private fields and opens its own connection from a raw URL, bypassing exapump's DSN, profile, fingerprint, and transport resolution. |
| Full multi-table fan-out in the first release | Ship flat JSON only, defer nesting | Flat-only JSON needs none of `json_tables_core` and does not match `exasol-json-tables`. The nesting is the capability the user asked for. |
| Import through `import_from_record_batches` | Stage Parquet files and call `import_parquet_from_files`, as upstream does | Removes the temporary directory, the Parquet writer, and its cleanup logic. exapump's mission defines the tool as a thin wrapper over exarrow-rs. |
| `CREATE TABLE IF NOT EXISTS` and no constraint statements | Emit the constraint statements and ignore failures; probe the catalog for pre-existing tables | `build_sql_schema` emits plain `CREATE TABLE` plus `ALTER TABLE ... ADD CONSTRAINT`, so a second run fails on both. exapump's upload contract is re-runnable. The constraints are emitted `DISABLE` upstream and enforce nothing. `TableBuffer` restarts `_id` at 1 on every run, so the linkage columns resolve only within one run, and every import warns about that on stderr. Ignoring errors or parsing error strings is worse. Recorded as a follow-up. |
| Not atomic across the family, with explicit partial-load reporting | Wrap the family in one transaction | Whether the HTTP-transport IMPORT participates in a surrounding transaction is unverified for this exarrow-rs version. Claiming atomicity without evidence is worse than reporting what loaded. |
| `.json` and `.ndjson` only | Also accept `.jsonl` | Framing is detected from file content, so `.jsonl` adds no capability, only a third spelling. Add it when a user asks. |
| One file per invocation | Expand `args.files` for JSON | `upload` reads `args.files[0]` for CSV and Parquet today. Changing that is a separate concern for all three formats. |
| Buffer the whole family in memory | Stream each table to Exasol in chunks; spill buffers to disk | `json_tables_core::buffer::ColumnBuffers` buffers the family by design, and `for_each_document` parses a top-level array as one value. Peak memory scales with the input size. A streaming `RowSink` is possible, because the trait exists for that purpose, but it needs a chunked import loop and a memory budget that this feature does not define. Documented in the spec Background, with NDJSON named as the shape for large input. |
| Reject an empty plan and a family of only generated key columns | Let Exasol reject the generated SQL; accept the key-only table and load it | A zero-document input yields no planned table. `json_tables_core/src/infer.rs:203-209` sets `include_id` unconditionally for every object table, so an all-empty-object input yields a root table holding `_id` alone. That DDL is valid, so Exasol accepts it and the run would silently create a table carrying no document data. `upload/csv-import` already requires a non-zero exit for an empty CSV, so a clear exapump-side error matches the existing contract. |

## Features

| Feature | Status | Spec |
|---------|--------|------|
| upload/json-import | NEW | `specs/_plans/add-json-import/upload/json-import/spec.md` |
| upload/parquet-import | CHANGED | `specs/_plans/add-json-import/upload/parquet-import/spec.md` |
| cli/upload-command-structure | CHANGED | `specs/_plans/add-json-import/cli/upload-command-structure/spec.md` |

## Impact

`exapump upload data.json --table <table>` changes behavior. It previously failed with "file format not supported" and now plans and loads a table family. Any script that relied on that failure breaks.

One `upload` invocation may now create more than one table. A user who targets `sales.orders` with nested JSON also gets `sales.orders_customer` and `sales.orders_items_arr`. `--dry-run` shows the full list before anything is created.

The supported-format list in the error message and in `--help` grows to `.parquet, .csv, .json, .ndjson`.

exapump gains a git dependency. Builds now need network access to `github.com/exasol-labs/exasol-json-tables`, which is a public repository. `Cargo.lock` pins the exact commit.

No existing CSV or Parquet behavior changes.

## Dependencies

| Dependency | Source | Version | Notes |
|------------|--------|---------|-------|
| `json_tables_core` | `https://github.com/exasol-labs/exasol-json-tables` | tag `v0.3` | MIT. Public repository. Its only dependency is `serde_json`, already in exapump's tree. |
| `serde_json` | crates.io | 1 | New direct dependency, already transitive. Needed for the `Value` types in the `json_tables_core` API. |

MIT is already listed in both `deny.toml` (`[licenses].allow`) and `about.toml` (`accepted`), so no license list change is expected. `deny.toml` declares no `[sources]` section, so `cargo deny check licenses` must be run against the new git source and a `[sources]` allow entry added if it rejects one.

## Implementation Tasks

### 1. Format surface and dependency

- [ ] 1.1 Add the `json_tables_core` git dependency pinned to tag `v0.3` and the `serde_json` dependency to `Cargo.toml`. Run `cargo deny check licenses` and `cargo deny check advisories`. Add a `[sources]` allow entry to `deny.toml` only if the git source is rejected.
- [ ] 1.2 Add `FileFormat::Json` to `src/format.rs`, map the `.json` and `.ndjson` extensions to it, and update `SUPPORTED_FORMATS` to `.parquet, .csv, .json, .ndjson`. Update the `src/format.rs` unit tests: the unsupported-extension test must use `.txt`, and new tests must cover both JSON extensions in lower and upper case.
- [ ] 1.2a Add the `(FileFormat::Json, true)` and `(FileFormat::Json, false)` arms to the `(format, args.dry_run)` match in `src/commands/upload.rs`. Both arms `anyhow::bail!` with a not-yet-implemented message. Task 1.2 makes this match non-exhaustive, so this task is what keeps the crate compiling until tasks 2.4 and 2.8 replace both arms.
- [ ] 1.3 Update `tests/parquet_test.rs::unsupported_file_extension` to use a `.txt` file and assert the new supported-format list.
- [ ] 1.4 Extend the `UploadArgs::table` doc comment in `src/cli.rs` to state that JSON input creates a root table plus one subtable per nested path. Add the `tests/cli_test.rs` assertion for it.
- [ ] 1.5 Update `README.md` and `specs/mission.md` for the new formats: the supported-format list, the core-capability list, and the `json_tables_core` entry in the Tech Stack table.

### 2. JSON table family

- [ ] 2.1 Add `tests/fixtures/mod.rs` helpers that create the JSON inputs the scenarios need: a flat array file, a nested array-and-object file, an NDJSON file, a `.json` file holding NDJSON framing, a mixed-scalar-type file, an explicit-null file, an empty file, an empty-array file, an all-empty-objects file, and a file whose third array entry is a number.
- [ ] 2.2 Write the failing `tests/json_test.rs` tests for the dry-run, file-not-found, empty-file, no-documents, no-properties, non-object-entry, and connection-failure scenarios, plus a test that `--delimiter` is accepted and ignored on a JSON dry run. These need no Exasol connection.
- [ ] 2.3 Implement `plan_family` and `TableFamily::describe` in the new `src/json_tables.rs`, over `json_tables_core`'s `detect_format`, `for_each_document`, `StatsCollector`, `build_all_schema_plans`, and `build_sql_schema`. Apply the identifier rules from the Design section and rewrite each `CREATE TABLE` to `CREATE TABLE IF NOT EXISTS`. Discard the constraint statements. Before any statement is built, reject when `build_all_schema_plans` returns no table, and reject when every planned table carries only the generated key columns `_id`, `_parent`, and `_pos`. A table planned from documents that carry properties always holds at least one column outside that set, so the second check fires only for an all-empty-object input.
- [ ] 2.4 Replace the task 1.2a `(FileFormat::Json, true)` bail arm in `src/commands/upload.rs` with the dry-run dispatch into `json_tables`, and declare the `json_tables` module in `src/main.rs`.
- [ ] 2.5 Write the failing `tests/json_test.rs` tests for the load scenarios against the Exasol Docker container: flat import, nested subtable creation, generated key columns, NDJSON, content-detected framing, mixed scalar types, explicit null mask, repeated run, unqualified table name, unqualified table name with no connection schema, and partial-family failure reporting.
- [ ] 2.6 Implement the `ColumnBuffers` to `arrow::RecordBatch` bridge in `src/json_tables.rs`. Iterate `PlannedTable::columns` in plan order, keep only the columns for which `column_sql_type` returns `Some`, and map `ColumnValues::{Bool, BoolMask, Int, Double, Str}` onto `BooleanArray`, non-null `BooleanArray`, `Int64Array`, `Float64Array`, and `StringArray`. The Arrow field order must match the CREATE statement column order exactly. [expert]
- [ ] 2.7 Implement `load` in `src/json_tables.rs`. Resolve the target schema first, per the Identifier rules: the uppercased `--table` prefix, else `conn.params().schema` uppercased, else fail before any statement runs. Run `OPEN SCHEMA` on the resolved schema, execute the CREATE statements in family order, run the second document pass into `ColumnBuffers`, then import each table with `import_from_record_batches` against its explicitly qualified name. Iterate `family.plans` for the CREATE statements, the imports, and the returned counts, and resolve each table's buffer through `ColumnBuffers::table(&plan.path)`. Never iterate `ColumnBuffers::tables()`, whose `HashMap` order is randomized per process. On success, return every table's count paired with `None`. On a failure, return the counts collected so far paired with `Some(error)`. Name the failed table in that error. Write nothing to stdout. [expert]
- [ ] 2.8 Replace the task 1.2a `(FileFormat::Json, false)` bail arm in `src/commands/upload.rs` with the import dispatch. Print one stdout line per entry of the vector `load` returns, with its row count. Print the family total after those lines. Print the same vector on the failure path, before you return the error that `load` paired with it. Keep every stdout write in `src/commands/upload.rs`. Print the `_id` warning to stderr on every JSON import, because the command does not probe the catalog for a pre-existing family.

## Parallelization

| Group | Tasks | Depends on | Knowledge |
|-------|-------|------------|-----------|
| A: Format surface and dependency | 1.1-1.5 | — | spec deltas `upload/parquet-import`, `cli/upload-command-structure`; `Cargo.toml`, `deny.toml`, `src/format.rs`, `src/cli.rs`, `src/commands/upload.rs`, `tests/parquet_test.rs`, `tests/cli_test.rs`, `README.md`, `specs/mission.md` |
| B: JSON table family | 2.1-2.8 | A (needs `FileFormat::Json` from `src/format.rs` and the `json_tables_core` dependency) | spec delta `upload/json-import`, plus the `CSV flags ignored for Parquet files` scenario of `cli/upload-command-structure`; `src/json_tables.rs`, `src/commands/upload.rs`, `src/main.rs`, `tests/json_test.rs`, `tests/fixtures/mod.rs` |

Group B carries the `[expert]` tasks and routes to `implementer-expert-agent`.

Group A must finish before Group B starts. The two groups are sequenced, not parallel. `src/commands/upload.rs` is the one file both touch, and the sequencing is what keeps them from contending. `src/commands/upload.rs:15-20` matches `(format, args.dry_run)` exhaustively, so task 1.2 makes that match non-exhaustive and the crate stops compiling with E0004. Task 1.2a restores compilation inside Group A with two bail arms, so Group A's own `cargo test` assertions in tasks 1.3 and 1.4 can build. Tasks 2.4 and 2.8 then replace those arms with the real dispatch.

## Dead Code Removal

| Type | Location | Reason |
|------|----------|--------|
| None | — | No code becomes obsolete. The `.json` assertions in `src/format.rs` and `tests/parquet_test.rs` move to `.txt` rather than disappear. |

## Verification

### Scenario Coverage

| Scenario | Test Type | Test Location | Test Name |
|----------|-----------|---------------|-----------|
| json-import: Import a flat JSON array into one table | Integration | `tests/json_test.rs` | `import_flat_json_array_creates_one_table` |
| json-import: Import nested JSON creates a subtable per nested path | Integration | `tests/json_test.rs` | `import_nested_json_creates_subtable_per_path` |
| json-import: Nested tables carry the generated key columns | Integration | `tests/json_test.rs` | `nested_tables_carry_generated_key_columns` |
| json-import: Import an NDJSON file | Integration | `tests/json_test.rs` | `import_ndjson_file` |
| json-import: Framing detected from file content, not extension | Integration | `tests/json_test.rs` | `json_extension_with_ndjson_framing_imports` |
| json-import: Dry-run shows the planned table family | Integration | `tests/json_test.rs` | `dry_run_shows_planned_table_family` |
| json-import: Property with mixed scalar types gets alternate columns | Integration | `tests/json_test.rs` | `mixed_scalar_types_create_alternate_columns` |
| json-import: Explicit JSON null stays distinct from an absent property | Integration | `tests/json_test.rs` | `explicit_null_sets_null_mask_column` |
| json-import: Repeated run loads into the existing family | Integration | `tests/json_test.rs` | `repeated_run_appends_to_existing_family` |
| json-import: Unqualified table name uses the connection schema | Integration | `tests/json_test.rs` | `unqualified_table_uses_connection_schema` |
| json-import: Unqualified table name with no connection schema | Integration | `tests/json_test.rs` | `unqualified_table_without_connection_schema_errors` |
| json-import: Import failure reports the tables already loaded | Integration | `tests/json_test.rs` | `partial_family_failure_reports_loaded_tables` |
| json-import: Empty JSON file | Integration | `tests/json_test.rs` | `empty_json_file_errors` |
| json-import: JSON file with no documents | Integration | `tests/json_test.rs` | `json_file_with_no_documents_errors` |
| json-import: Documents with no properties | Integration | `tests/json_test.rs` | `documents_with_no_properties_error` |
| json-import: Document that is not a JSON object | Integration | `tests/json_test.rs` | `non_object_entry_errors_with_position` |
| json-import: JSON file not found | Integration | `tests/json_test.rs` | `json_file_not_found` |
| json-import: Connection failure | Integration | `tests/json_test.rs` | `json_connection_failure` |
| parquet-import: Unsupported file extension | Integration | `tests/parquet_test.rs` | `unsupported_file_extension` |
| upload-command-structure: CSV flags ignored for Parquet files | Integration | `tests/json_test.rs` | `csv_flags_ignored_for_json_file` |
| upload-command-structure: Upload help describes the table family for JSON input | Integration | `tests/cli_test.rs` | `upload_help_describes_json_table_family` |
| Extension-to-format mapping (supporting) | Unit | `src/format.rs` | `json_extension_returns_json`, `ndjson_extension_returns_json`, `uppercase_json_extension_returns_json`, `unsupported_extension_returns_error_with_supported_formats` |

`partial_family_failure_reports_loaded_tables` forces its failure by creating the root table `"SALES"."ORDERS"` up front with an incompatible column list, so `"SALES"."ORDERS_items_arr"` loads and the root import fails. The sabotaged table must be the root, not a subtable, because family order sorts the root last (see § Decision). Sabotaging the first table in the order would leave nothing loaded before the failure and the scenario's stdout assertion would have nothing to assert.

`repeated_run_appends_to_existing_family` runs the same import twice and asserts that the row counts double. It also asserts that the second run prints the `_id` warning to stderr. The warning is unconditional, so the first run prints it too.

### Manual Testing

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| upload/json-import | `exapump upload orders.json --table sales.orders --dsn 'exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0' --dry-run` | Lists `"SALES"."ORDERS"`, `"SALES"."ORDERS_customer"`, and `"SALES"."ORDERS_items_arr"` with their columns and `CREATE TABLE IF NOT EXISTS` statements. Exit code 0. No table created. |
| upload/json-import | `exapump upload orders.json --table sales.orders --dsn 'exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0'` | One row-count line per table in the family, then the total. Exit code 0. `SELECT COUNT(*)` on each table matches the printed counts. |
| upload/json-import | `exapump upload events.ndjson --table raw.events --dsn 'exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0'` | One row per non-empty line loaded into `"RAW"."EVENTS"`. Exit code 0. |
| upload/parquet-import | `exapump upload notes.txt --table sales.orders --dsn 'exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0'` | Non-zero exit. stderr names the unsupported format and lists `.parquet, .csv, .json, .ndjson`. |
| cli/upload-command-structure | `exapump upload --help` | The `--table` description states that JSON input creates a root table plus one subtable per nested path. |

### Checklist

| Step | Command | Expected |
|------|---------|----------|
| Build | `cargo build` | Exit 0 |
| Test | `cargo test` | 0 failures, with the Exasol container running on port 8563 |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | 0 errors, 0 warnings |
| Format | `cargo fmt --check` | No changes |
| Licenses | `cargo deny check licenses` | Exit 0 |
| Advisories | `cargo deny check advisories` | Exit 0 |
