# Plan: fix-json-import-review-findings

## Summary

Fix the three correctness defects a human reviewer found on `feat/add-json-import` (positional import, colliding table names, and a `DECIMAL(18,0)` column too narrow for a 64-bit integer), close three gaps in format and input handling, and correct the spec, ADR, and test-harness inaccuracies the same review reported. exapump takes over rendering the `CREATE TABLE` statements, which resolves the type-width defect, the stale DDL-ownership statement, and the SQL-rewrite complexity in one change.

## Design

### Context

`src/json_tables.rs` hands every table of a family to `exarrow_rs::Connection::import_from_record_batches` without naming the batch's columns, so Exasol maps CSV fields onto table columns by position. Tables are created with `CREATE TABLE IF NOT EXISTS`, so a second file with a different property set loads into the first file's columns and reports success. Two different JSON paths can also produce one table name, because the separator between two path segments and an underscore inside a JSON key both render as `_`. That family is created once and loaded twice, and the second load writes the wrong column.

`json_tables_core` maps every integer to `DECIMAL(18,0)` (`crates/json_tables_core/src/contract.rs:298`), but its own classifier admits every 64-bit signed integer into an integer column (`classify_value`, same file). A 19-digit value therefore fails with `ETL-3050 numeric value out of range` after the tables already exist. exapump cannot fix this inside `column_sql_type` without releasing a new upstream tag.

exapump already rewrites the upstream `CREATE TABLE` text by prefix match, so `json_tables_core` does not in fact own the DDL, contrary to `specs/_decision/008-add-json-import.md` and three spec Backgrounds. That rewrite carries its own error path and a unit test that parses SQL back out to check column order.

- **Goals**: load every value into the column of its own name. Reject a family exapump cannot load correctly, before it creates any table. Cover the full 64-bit signed integer range. Make the recorded specs, the ADR, and the mission match the code. Remove the SQL rewrite, the second schema mechanism, and the five-way early return from `load`.
- **Non-Goals**: streaming or spill-to-disk for a large file. Support for a single JSON object spread over several lines. A change to `json_tables_core`. Primary-key or foreign-key constraints. Renumbering `_id` across runs.

### Decision

exapump owns every SQL statement it runs. `json_tables_core` owns the plan: which tables the family holds, which columns each table carries, and which contract type each column has. exapump maps a contract type to an Exasol type itself and renders `CREATE TABLE IF NOT EXISTS` from the column plan.

#### Architecture

```
┌────────────────────────┐   PlannedTable   ┌──────────────────────────────┐
│ json_tables_core       │   (path, columns,│ exapump src/json_tables.rs   │
│  • which tables exist  │───  types) ─────▶│  • framing and file reading  │
│  • which columns       │                  │  • Exasol type mapping       │
│  • contract types      │                  │  • CREATE TABLE rendering    │
│  • row buffering       │                  │  • schema resolution         │
└────────────────────────┘                  │  • Arrow batch + IMPORT      │
                                            └──────────────────────────────┘
                                                         │
                                          qualified DDL + named columns
                                                         ▼
                                            ┌──────────────────────────────┐
                                            │ Exasol                       │
                                            └──────────────────────────────┘
```

#### Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| One owner per SQL statement | `src/json_tables.rs` renders the `CREATE TABLE` text from `physical_columns` | Column order becomes correct by construction, so the prefix rewrite and the SQL-parsing test both go |
| Import by column name | `import_batch` derives the `IMPORT` column list from the batch it sends | One source of truth, so the column list cannot drift from the CSV field order |
| Reject before the first statement | `plan_family` runs every family-level rejection | A `--dry-run` reports the same rejection, and no table exists when the run fails |
| One resolved schema, named everywhere | `load` resolves the schema, then passes it to the DDL renderer and to `qualified_name` | `OPEN SCHEMA` and the session state it depends on both go |
| Accumulator out-parameter | `load(.., loaded: &mut Vec<(String, u64)>) -> anyhow::Result<()>` | Each of the five early returns becomes `?`, and the caller still prints what loaded |
| Drop guard for a test schema | `tests/fixtures/mod.rs` `SchemaGuard` | Cleanup runs on a failed assertion, so a failing test stops leaving a schema behind |

### Assumptions

These beliefs carry the design. An implementer who finds one false MUST stop and report it.

| Assumption | Basis | If false |
|------------|-------|----------|
| Exasol fails `IMPORT INTO t ("_id", "a", "c")` when `t` holds no column `c` | Standard `IMPORT` column-list behavior. Not yet run against the container | The scenario "Second file carries a column the target table does not hold" needs a different mechanism, such as a catalog probe |
| `exarrow-rs` writes the `columns` list verbatim into the statement, so exapump MUST quote each name | `src/query/import.rs:402` joins the names with `, ` and adds no quoting | Double quoting would break every column name |
| `DECIMAL(19,0)` accepts every value `classify_value` routes to `SimpleType::Integer` | `classify_value` admits only `i64`-representable numbers and sends the rest to `SimpleType::Number` | A wider `DECIMAL` is needed, and the `DOUBLE` fallback claim in the Background is wrong |
| `NOT NULL` belongs on a column when `is_required \|\| is_null_mask` | `build_sql_schema` applies exactly that rule (`crates/json_tables_core/src/ddl.rs`) | The rendered DDL differs from the one the recorded scenarios were verified against |
| A `SchemaGuard` with `Display` and `Deref<Target = str>` compiles at all 26 existing call sites | The sites use `format!("{schema_name}...")`, `&schema_name`, and `schema_name.to_uppercase()`, which all work through those two traits | Each call site needs an explicit `.name()` call |
| `Drop` can run `DROP SCHEMA` from a dedicated thread with its own current-thread runtime | `tokio::runtime::Handle::block_on` panics inside a runtime thread, so the guard must not use it | The guard needs a blocking driver path, or the cleanup returns to the end of each test |

### Consequences

| Decision | Alternatives Considered | Rationale |
|----------|------------------------|-----------|
| exapump renders the DDL and maps `SimpleType::Integer` to `DECIMAL(19,0)` | Fix `column_sql_type` upstream and bump the pinned tag; document the 18-digit limit in the spec and change nothing | The upstream fix blocks this PR on another repository's release, which ADR `reuse-json-tables-core-via-git-dependency` already rejected for the dependency itself. Documenting the limit leaves a real defect in place. exapump already rewrites the DDL, so taking it over completes a split that exists |
| Pass quoted column names to `ArrowImportOptions::columns` | Compare the batch schema against `SYS.EXA_ALL_COLUMNS` before the import | `exarrow-rs` joins the names verbatim into `IMPORT INTO t (...)`, so Exasol enforces the match. A catalog probe adds a round trip and a second copy of the rule |
| Reject a family whose table names collide, in `plan_family` | Rely on the column-name import to fail later; rename one table automatically | The plan-time check fires before any table exists and under `--dry-run`. An automatic rename invents a name the user never wrote and cannot predict |
| Reject a single JSON object spread over several lines, with a message naming both accepted shapes | Support the shape by re-reading the file as one JSON value when line 1 fails to parse | The shape yields a one-row table. The fallback cannot tell a pretty-printed object from NDJSON whose first line is malformed, so it would report a misleading error for a real malformed file |
| Drop `OPEN SCHEMA` and qualify every statement | Keep `OPEN SCHEMA` as an existence probe | Two mechanisms select the schema today. The missing-schema failure moves from the open to the first `CREATE TABLE`, which still names the schema |
| Put the drop guard in `tests/fixtures/mod.rs` and delete the manual cleanup in all four test files | Add the guard only to `tests/json_test.rs` | `setup_exasol_schema` creates the schema, so it owns the cleanup. Fixing one of its four callers leaves the same leak in the other three |

## Features

| Feature | Status | Spec |
|---------|--------|------|
| upload/json-import | CHANGED | `specs/_plans/fix-json-import-review-findings/upload/json-import/spec.md` |
| upload/json-import-nesting | CHANGED | `specs/_plans/fix-json-import-review-findings/upload/json-import-nesting/spec.md` |
| upload/json-import-rejection | CHANGED | `specs/_plans/fix-json-import-review-findings/upload/json-import-rejection/spec.md` |
| upload/parquet-import | CHANGED | `specs/_plans/fix-json-import-review-findings/upload/parquet-import/spec.md` |

## Impact

`exapump upload data.jsonl` now works. It failed with "file format not supported" before.

A JSON integer column is created as `DECIMAL(19,0)` instead of `DECIMAL(18,0)`. A table an earlier exapump version created keeps `DECIMAL(18,0)`, because `CREATE TABLE IF NOT EXISTS` never alters an existing table. A 19-digit value still fails against such a table. Recreate the table to widen it.

A repeated upload into a table whose columns do not match the new file changes behavior in both directions. A file that carries a column the table does not hold now fails, where it loaded values into the wrong columns before. That direction is a breaking change for any caller that relied on the old behavior, and that behavior wrote wrong data. A file that omits a column the table does hold now loads NULL into that column and succeeds, where it failed with `ETL-6009` before.

An input whose JSON paths collide on one table name now fails before any table is created. It reported success before.

An input whose every planned table carries only generated columns, such as `{"a": {}}`, now fails. It created two empty tables before.

The failure for a schema Exasol does not hold moves from `OPEN SCHEMA` to the first `CREATE TABLE`. stderr still names the schema, and the exit code is unchanged.

## Dependencies

None. `json_tables_core` stays pinned at tag `v0.3`. `exarrow-rs` stays at 0.16.0, and `ArrowImportOptions::columns` is already present there (`src/import/arrow.rs:112`).

## Implementation Tasks

Three delta blocks change prose only and need no code task: the `upload/json-import` Background clauses on the `_id` warning and on memory, the `upload/json-import-nesting` Background, and the de-duplicated parts of the `upload/json-import-rejection` Background. `/speq:record` merges them. Every other delta block maps to a task below.

### 1. Test harness

- [ ] 1.1 Add `SchemaGuard` to `tests/fixtures/mod.rs`: it holds the schema name, implements `Display` and `Deref<Target = str>`, and on drop runs `DROP SCHEMA <name> CASCADE` from a fresh connection on a dedicated thread with its own current-thread runtime, so the cleanup also runs while a failed assertion unwinds. Change `setup_exasol_schema` to return `(Connection, SchemaGuard)`. The drop path MUST discard every error instead of unwrapping it, across the connection open, the `DROP SCHEMA` execution, and the `JoinHandle` result. `setup_exasol_schema` unwraps each of those steps today (`tests/fixtures/mod.rs:113-118`), and the existing manual cleanups use `let _ = conn.execute_update(...)`, which is the policy to follow. CAUTION: a panic inside `Drop` while a failed assertion unwinds aborts the test process and hides the original assertion.
- [ ] 1.2 Delete the trailing `DROP SCHEMA ... CASCADE` block from every test in `tests/json_test.rs`, `tests/csv_test.rs`, `tests/parquet_test.rs`, and `tests/cli_test.rs`. Where the deleted block was the connection's only use, change the binding to `let (_conn, schema_name)`. Four tests in `tests/csv_test.rs` need that change: `exasol_csv_import_prints_row_count`, `exasol_csv_import_with_custom_delimiter`, `exasol_csv_import_with_no_header`, and `exasol_csv_flags_ignored_for_parquet`. Without it `cargo clippy` reports `unused_variables` and `unused_mut`, and § Verification § Checklist fails.
- [ ] 1.3 Add `fn upload_json(path: &Path, table: &str, dsn: &str) -> Command` to `tests/json_test.rs`, returning a command preloaded with `upload <path> --table <table> --dsn <dsn>` so a caller can append `--dry-run` or a timeout. Route every upload invocation in the file through it.

### 2. JSON import core

- [ ] 2.1 Render the `CREATE TABLE` statements in exapump. [expert]
  Add `fn exasol_type(ty: SimpleType) -> Option<&'static str>` mapping `Bool` to `BOOLEAN`, `Integer` to `DECIMAL(19,0)`, `Number` to `DOUBLE`, `String` to `VARCHAR(2000000)`, and `Null`, `Object`, and `Array` to `None`. Its doc comment states why exapump owns the mapping. Point `physical_columns` at it. Replace `build_ddl` and `rewrite_create` with `fn create_table_ddl(plan: &PlannedTable, stem: &str, schema: Option<&str>) -> String`, which emits `CREATE TABLE IF NOT EXISTS <qualified> (\n  "<col>" <type>[ NOT NULL],\n...\n);` in `physical_columns` order, with `NOT NULL` when `is_required || is_null_mask`. Drop the `ddl` field from `TableFamily` and the `json_tables_core::ddl::build_sql_schema` import. Update the `DECIMAL(18,0)` assertion in `dry_run_shows_the_planned_table_family`.
- [ ] 2.2 Import by column name. [expert]
  In `import_batch`, build the column list from `batch.schema().fields()` through `sanitize_ident`, pass it to `ArrowImportOptions::new().columns(...)`, and name the columns in the failure context. The list names the batch columns only. A column the target table holds and the batch omits therefore stays unnamed, and Exasol loads NULL into it. Do not pad the list with the table's other columns, and do not probe `SYS.EXA_ALL_COLUMNS` for them. The NULL load needs no nullability change: `json_tables_core` marks only `_id`, `_parent`, `_pos`, and the `<name>|n` null-mask columns as required (`crates/json_tables_core/src/infer.rs:251-402`), and task 2.1 applies `NOT NULL` to those columns only. Write the three failing tests first: the reordered-properties test must place each value in its own column, the mismatched-column test must fail and leave the first file's row intact, and the omitted-column test must exit 0 and leave the omitted column NULL on the new row while the earlier rows keep their values.
- [ ] 2.3 Add `fn reject_duplicate_table_names(plans, stem, path) -> anyhow::Result<()>` and call it from `plan_family`. Compare `table_raw_name(&plan.path, stem)` across the family. The message names the table name and both `TablePath` values.
- [ ] 2.4 Extend `reject_unusable_plans` so a `<name>|object` or `<name>|array` column no longer counts as document data. Read the names to exclude from `plan.properties`, through `PropertyColumns::object_fk` and `PropertyColumns::array_count`, rather than by matching the column-name suffix. Restate the failure message to cover the link columns.
- [ ] 2.5 Resolve the target schema once and name it in every statement. [expert]
  Move the DDL rendering out of `plan_family` and into the two callers: `describe` renders with `self.schema.as_deref()`, and `create_tables` renders with `Some(resolved)`. Delete the `OPEN SCHEMA` statement and its error context. Update the assertion in `exasol_json_missing_target_schema_fails`.
- [ ] 2.6 Change `load` to `pub async fn load(family, path, conn, loaded: &mut Vec<(String, u64)>) -> anyhow::Result<()>`, replacing the five early returns with `?`. Update `json_import` in `src/commands/upload.rs` to own the vector, print it on both paths, and then propagate the error.
- [ ] 2.7 Accept `jsonl` in `detect_from_path` in `src/format.rs` and add it to `SUPPORTED_FORMATS`. Update two stale assertions to the full new list `.parquet, .csv, .json, .ndjson, .jsonl`: `unsupported_file_extension` in `tests/parquet_test.rs` and `unsupported_extension_returns_error_with_supported_formats` in `src/format.rs`. Both assert the old list as a substring, which survives the addition, so neither catches a later removal of `.jsonl` from `SUPPORTED_FORMATS`.
- [ ] 2.8 In `read_documents`, when the detected framing is `InputFormat::Lines` and `for_each_document` fails, add context stating that the file is neither a JSON array of objects nor one JSON object per line. Leave array-framing failures unchanged, because upstream already names the offending entry.

### 3. Documentation accuracy

- [ ] 3.1 Correct the Consequences line of `specs/_decision/008-add-json-import.md`. It must state that exapump owns file reading, framing, connection handling, the SQL it runs, the Arrow conversion, and the import, and that `json_tables_core` owns which tables exist and which columns they carry.
- [ ] 3.2 Correct the `json_tables_core` row of `specs/mission.md` § Tech Stack, which still claims the crate emits Exasol DDL.
- [ ] 3.3 Add a JSON and NDJSON section to `docs/file_exchange.md` § Upload: the three accepted extensions, content-based framing, the unsupported multi-line single object, subtable fan-out, and the memory note from `upload/json-import` § Background. State the mechanism, not an unmeasured size threshold.
- [ ] 3.4 Correct the seven doc comments in `src/json_tables.rs` that this plan makes false or narrow.
  The module doc comment at lines 3-4 still reads "`json_tables_core` owns every decision about which tables exist, which columns they carry, and which DDL describes them". Rewrite it to state that exapump owns the SQL it runs, and that `json_tables_core` owns which tables exist and which columns each table carries. Rewrite the doc comment of `qualified_name` and the doc comment of `create_tables` so neither explains itself through the removed `build_sql_schema`. The `create_tables` comment must also drop its `OPEN SCHEMA` sentence, which task 2.5 makes false.
  Rewrite the `TableFamily` doc comment at lines 29-34. It still names the `ddl` field that task 2.1 deletes. Name `plans` and the row counts as the ordered pair. Keep the rule that both follow the order `build_all_schema_plans` produced.
  Rewrite the `load` doc comment at lines 228-237. It still describes the `(Vec<(String, u64)>, Option<anyhow::Error>)` return that task 2.6 replaces. Describe the `loaded` accumulator parameter and the `anyhow::Result<()>` return. Keep the sentence that the import is not atomic across the family. Keep the sentence that a failure leaves the tables already loaded in place.
  Rewrite the `build_record_batch` doc comment at lines 313-318. It states that the import maps CSV fields onto table columns by position rather than by name. Task 2.2 removes that defect. State that the import names its columns, so field order no longer decides which column a value lands in. State that field order still follows `physical_columns`. Keep the sentence on resolving the buffer by table path.
  Restate the `GENERATED_KEY_COLUMNS` doc comment at lines 25-27. Task 2.4 narrows it rather than makes it false. State that the const holds the fixed generated names only. State that `reject_unusable_plans` adds the `<name>|object` and `<name>|array` link columns from `plan.properties`.

## Parallelization

| Group | Tasks | Depends on | Knowledge |
|-------|-------|------------|-----------|
| A: Test harness hygiene | 1.1-1.3 | — | no spec delta; `tests/fixtures/mod.rs`, `tests/json_test.rs`, `tests/csv_test.rs`, `tests/parquet_test.rs`, `tests/cli_test.rs` |
| B: JSON import core | 2.1-2.8, 3.4 | A (shares `tests/json_test.rs`, `tests/parquet_test.rs`, and `tests/fixtures/mod.rs`) | spec deltas `upload/json-import`, `upload/json-import-nesting`, `upload/json-import-rejection`, `upload/parquet-import`; `src/json_tables.rs`, `src/format.rs`, `src/commands/upload.rs`, `tests/json_test.rs`, `tests/parquet_test.rs`, `tests/fixtures/mod.rs` |
| C: Documentation accuracy | 3.1-3.3 | — | no spec delta; `specs/_decision/008-add-json-import.md`, `specs/mission.md`, `docs/file_exchange.md` |

Group B carries every source change. Eight of its nine tasks edit `src/json_tables.rs`, and eight add or change a test in `tests/json_test.rs`. Splitting them would hand two agents the same two files and the same spec deltas. Task 2.7 is the one task that touches no source file beyond `src/format.rs` and the two extension-list assertions, and it is too small to justify a group of its own. Task 3.4 rewrites doc comments inside `src/json_tables.rs`, so it belongs with the tasks that rewrite that file rather than with group C, which touches no source file.

## Dead Code Removal

| Type | Location | Reason |
|------|----------|--------|
| Function | `rewrite_create`, `src/json_tables.rs` | exapump renders the statement instead of rewriting upstream text |
| Function | `build_ddl`, `src/json_tables.rs` | Replaced by `create_table_ddl`, called per table at the point the schema is known |
| Field | `TableFamily::ddl`, `src/json_tables.rs` | The DDL now depends on the resolved schema, so it cannot be built at plan time |
| Test | `build_ddl_rejects_an_unrecognised_create_statement`, `src/json_tables.rs` | Tests the removed rewrite error path |
| Test | `ddl_column_order_matches_record_batch_field_order`, `src/json_tables.rs` | The import names its columns, so column order no longer decides correctness |
| Function | `declared_columns` and `leading_quoted_ident`, `src/json_tables.rs` tests | Only the removed order test parsed the CREATE statement back |
| Statement | `OPEN SCHEMA` in `create_tables`, `src/json_tables.rs` | Every statement names the resolved schema |
| Code | The trailing `DROP SCHEMA ... CASCADE` block in 26 tests | `SchemaGuard` drops the schema, including on a failed assertion |

## Verification

### Scenario Coverage

| Scenario | Test Type | Test Location | Test Name |
|----------|-----------|---------------|-----------|
| upload/json-import: Import an NDJSON file with the .jsonl extension | Integration | `tests/json_test.rs` | `exasol_jsonl_extension_imports_one_row_per_line` |
| upload/json-import: Integer column holds the full 64-bit signed range | Integration | `tests/json_test.rs` | `exasol_json_nineteen_digit_integer_imports_unchanged` |
| upload/json-import: Repeated run places each value in the column of its own name | Integration | `tests/json_test.rs` | `exasol_json_reordered_properties_keep_each_value_in_its_column` |
| upload/json-import: Second file omits a column the target table holds | Integration | `tests/json_test.rs` | `exasol_json_second_file_omitting_a_column_loads_null` |
| upload/json-import-rejection: Qualified table name whose schema does not exist | Integration | `tests/json_test.rs` | `exasol_json_missing_target_schema_fails` |
| upload/json-import-rejection: Documents with no properties | Integration | `tests/json_test.rs` | `documents_with_no_properties_are_rejected` |
| upload/json-import-rejection: Nested object that holds no properties | Integration | `tests/json_test.rs` | `nested_object_with_no_properties_is_rejected` |
| upload/json-import-rejection: Two JSON paths map to one table name | Integration | `tests/json_test.rs` | `colliding_table_names_are_rejected` |
| upload/json-import-rejection: Second file carries a column the target table does not hold | Integration | `tests/json_test.rs` | `exasol_json_second_file_with_an_unknown_column_fails` |
| upload/json-import-rejection: Single JSON object spread over several lines | Integration | `tests/json_test.rs` | `single_multi_line_json_object_is_rejected` |
| upload/parquet-import: Unsupported file extension | Integration | `tests/parquet_test.rs` | `unsupported_file_extension` |

The `.jsonl` extension also gets a unit test, `jsonl_extension_returns_json` in `src/format.rs`, matching the existing per-extension tests there. `detect_from_path` is pure computation with no I/O.

Every other scenario of these four features keeps the test it already has. Four of those tests change their assertions without changing their scenario. `dry_run_shows_the_planned_table_family` expects `DECIMAL(19,0)`. `exasol_json_missing_target_schema_fails` expects the create-table failure instead of the open-schema failure. `unsupported_file_extension` in `tests/parquet_test.rs` and `unsupported_extension_returns_error_with_supported_formats` in `src/format.rs` each expect the full list `.parquet, .csv, .json, .ndjson, .jsonl`, which is what verifies the changed "Unsupported file extension" scenario.

New fixtures in `tests/fixtures/mod.rs`: `create_jsonl`, `create_wide_integer_json`, `create_colliding_paths_json`, `create_hollow_object_json`, `create_multi_line_object_json`, and `create_json_file(dir, name, content)` for the three column-mapping scenarios, which each need a pair of files.

### Manual Testing

| Feature | Command | Expected Output |
|---------|---------|-----------------|
| upload/json-import | `printf '[{"id": 1234567890123456789}]' > /tmp/ids.json && exapump upload /tmp/ids.json --table sales.ids --dsn "$EXAPUMP_DSN" --dry-run` | Prints `id: DECIMAL(19,0)` and `CREATE TABLE IF NOT EXISTS "SALES"."IDS" (` |
| upload/json-import | `printf '{"a":1}\n{"a":2}\n' > /tmp/e.jsonl && exapump upload /tmp/e.jsonl --table sales.e --dsn "$EXAPUMP_DSN"` | Prints `Imported 2 rows into "SALES"."E"` and exits 0 |
| upload/json-import-rejection | `printf '[{"customer":{"address":{"city":"B"}},"customer_address":{"zip":"1"}}]' > /tmp/c.json && exapump upload /tmp/c.json --table sales.col --dsn "$EXAPUMP_DSN"` | Exits non-zero; stderr names `COL_customer_address`, `customer.address`, and `customer_address` |
| upload/json-import-rejection | `printf '[{"a":1,"b":"from_b"}]' > /tmp/1.json && printf '[{"a":2,"c":"from_c"}]' > /tmp/2.json && exapump upload /tmp/1.json --table sales.m --dsn "$EXAPUMP_DSN" && exapump upload /tmp/2.json --table sales.m --dsn "$EXAPUMP_DSN"` | The second run exits non-zero; stderr names `"SALES"."M"` and the column `c`; `SELECT * FROM SALES."M"` still returns the one row `from_b` |
| upload/json-import-rejection | `printf '{\n  "a": 1\n}\n' > /tmp/s.json && exapump upload /tmp/s.json --table sales.s --dsn "$EXAPUMP_DSN"` | Exits non-zero; stderr states the file is neither a JSON array of objects nor one JSON object per line |
| upload/json-import-rejection | `printf '[{"a":{}}]' > /tmp/h.json && exapump upload /tmp/h.json --table sales.h --dsn "$EXAPUMP_DSN"` | Exits non-zero; stderr states that no column could be derived; `SALES` holds no table `H` |
| upload/json-import-nesting | `printf '[{"id":1,"customer":{"tier":"gold"},"items":[{"sku":"S1"}]}]' > /tmp/n.json && exapump upload /tmp/n.json --table sales.n --dsn "$EXAPUMP_DSN" --dry-run` | Prints a `CREATE TABLE IF NOT EXISTS` statement for `"SALES"."N"`, `"SALES"."N_customer"`, and `"SALES"."N_items_arr"`, each with `"_id" DECIMAL(19,0) NOT NULL` |
| upload/parquet-import | `exapump upload /tmp/x.txt --table sales.x --dsn "$EXAPUMP_DSN"` | Exits non-zero; stderr lists `.parquet, .csv, .json, .ndjson, .jsonl` |

Every command above needs the Exasol container from `CLAUDE.md`, and a DSN carrying `tls=true&validateservercertificate=0`.

### Checklist

| Step | Command | Expected |
|------|---------|----------|
| Build | `cargo build` | Exit 0 |
| Test | `cargo test` | 0 failures |
| Lint | `cargo clippy && cargo fmt --check` | 0 errors/warnings |
| Format | `cargo fmt` | No changes |
| Specs | `speq plan validate fix-json-import-review-findings` | Validation passed |
