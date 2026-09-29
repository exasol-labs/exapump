# Code Review Findings: fix-json-import-review-findings

## Summary
- Files reviewed: 11
- Total findings: 4 (standard: 3, expert: 1)

Evidence gathered during this review: `cargo clippy --all-targets --all-features -- -D warnings` exits 0, `cargo fmt --check` exits 0, and `cargo test --test json_test` passes 33/33 against the local `exasol-test` container. `SYS.EXA_ALL_SCHEMAS` holds zero `EXAPUMP%` schemas after the run, so `SchemaGuard` cleans up as designed.

Two questions the brief raised, both checked and both clear, so neither produces a finding:

1. `SchemaGuard::drop` (`tests/fixtures/mod.rs:133-156`) contains no `unwrap`, `expect`, `panic!`, or indexing. The runtime build, `driver.open`, and `db.connect()` each use `let ... else { return; }`, `execute_update` uses `let _ =`, and the `JoinHandle` uses `let _ = handle.join();`, so a join failure is discarded rather than resumed as a panic. `runtime.block_on` runs on a thread created by `std::thread::spawn`, which carries no Tokio thread-local context, so the "cannot start a runtime from within a runtime" panic the plan warned about cannot fire.
2. Deleting `ddl_column_order_matches_record_batch_field_order` is sound. `create_table_ddl` (line 164) and `build_record_batch` (line 379) both iterate `physical_columns(plan)` over the same `PlannedTable` value, and `physical_columns` (lines 184-188) is a deterministic filter over `plan.columns`, so the two orders are identical by construction with no code path between them. `import_batch` derives its `IMPORT` column list from `batch.schema().fields()`, so a hypothetical order difference would no longer decide which column a value lands in. No remaining path can produce a silent order-driven mismatch.

## Standard fixes

### src/json_tables.rs

#### [UNUSED_IMPORT] Test module re-imports a name the glob already provides
- Location: line 448
- Issue: `mod tests` opens with `use super::*;` (line 447), which already brings `SimpleType` into scope from the module-level import at line 20, and then adds `use json_tables_core::contract::SimpleType;` on line 448. rustc does not report an explicit import that shadows a glob import, which is why `cargo clippy -- -D warnings` stays green on it.
- Fix: In src/json_tables.rs, delete line 448 (`use json_tables_core::contract::SimpleType;`) from `mod tests`, leaving `use super::*;` as the only import, and confirm `cargo test --lib` still compiles.

#### [MISSING_BOUNDARY_TEST] The NOT NULL branch of create_table_ddl is asserted nowhere
- Location: lines 163-180
- Issue: `create_table_ddl` is the new pure function that emits every `CREATE TABLE` statement exapump runs, and its `column.is_required || column.is_null_mask` branch (lines 166-170) has no test. `grep -rn "NOT NULL" tests/ src/` returns only the code at line 167 and its doc comment at line 161, plus one unrelated `IS NOT NULL` inside a `WHERE` clause in tests/json_test.rs:201. Deleting the `" NOT NULL"` arm entirely would leave all 33 json integration tests and the full unit suite green. The plan's § Dead Code Removal took `mixed_family`, `declared_columns`, and `leading_quoted_ident` with the deleted order test, so no unit test exercises DDL rendering at all any more, and the rendered statement's column order, identifier quoting, and schema qualification are likewise only covered indirectly by `--dry-run` substring assertions.
- Fix: In src/json_tables.rs `mod tests`, add `fn create_table_ddl_marks_required_and_null_mask_columns_not_null()`. Build `let documents: Vec<serde_json::Map<String, serde_json::Value>>` from `serde_json::json!({"id": 1, "note": null})` and `serde_json::json!({"id": 2, "extra": "x"})`, unwrapping each `serde_json::Value::Object`. Call `let plans = StatsCollector::plan_from_documents(&documents);` and assert that `create_table_ddl(&plans[0], "ORDERS", Some("SALES"))` equals exactly:
  ```
  CREATE TABLE IF NOT EXISTS "SALES"."ORDERS" (
    "_id" DECIMAL(19,0) NOT NULL,
    "id" DECIMAL(19,0),
    "note|n" BOOLEAN NOT NULL,
    "extra" VARCHAR(2000000)
  );
  ```
  Give the assertion the message that a required column and a null-mask column each carry `NOT NULL` while an ordinary document column does not.

### tests/json_test.rs

#### [DUPLICATE_TEST] Collision test's third assertion is implied by its first
- Location: lines 1007-1009, in `colliding_table_names_are_rejected`
- Issue: the test asserts `stderr` contains `"COL_customer_address"`, then `"customer.address"`, then `"customer_address"`. The third string is a substring of the first, so it can never fail independently and verifies nothing. The assertion was meant to check that the message names both colliding `TablePath` values, but as written it cannot distinguish the second path from the table name. The real message is `two JSON paths in <path> produce the table name COL_customer_address: customer.address and customer_address. Rename one of the properties, or import the two paths under different --table values.`
- Fix: In tests/json_test.rs `colliding_table_names_are_rejected`, replace the two predicates `.stderr(predicate::str::contains("customer.address"))` and `.stderr(predicate::str::contains("customer_address"))` with the single predicate `.stderr(predicate::str::contains("customer.address and customer_address"))`, keeping the `"COL_customer_address"` predicate unchanged.

## Expert fixes

### src/json_tables.rs

#### [BROAD_CATCH] Every NDJSON parse failure is blamed on the multi-line-object shape
- Location: lines 287-295, in `read_documents`
- Issue: the `map_err` attaches the multi-line-object diagnosis to *every* `InputFormat::Lines` failure, not only to the shape it names. Verified by running `printf '{"a":1}\n{"a":2\n{"a":3}\n' > bad.ndjson` through `upload`, which prints:
  ```
  Error: failed to read /tmp/.../bad.ndjson

  Caused by:
      0: the file is neither a JSON array of objects nor one JSON object per line. A single JSON object spread over several lines matches neither framing
      1: Line 2: EOF while parsing an object at line 1 column 6
  ```
  That file *is* one JSON object per line, with a typo on line 2, so the outer context states the opposite of the truth and buries the line number that identifies the real defect. plan.md § Consequences rejected the re-read fallback for precisely this failure mode ("it would report a misleading error for a real malformed file"); the chosen implementation reintroduces it on the other path. The existing test `single_multi_line_json_object_is_rejected` cannot catch this, because it only checks the two substrings that both a correct and an over-broad message contain.
- Fix: In src/json_tables.rs `read_documents`, add `let mut documents_seen = 0usize;` beside `visit_failure`, and increment it inside the `for_each_document` closure before calling `visit`. Change the `map_err` arm so the multi-line-object context is attached only when `matches!(format, InputFormat::Lines) && documents_seen == 0`, and every other framing failure is wrapped with `anyhow::Error::new(error)` alone. Keep the message text unchanged, because `specs/_plans/fix-json-import-review-findings/upload/json-import-rejection/spec.md:84` requires stderr to state that the file is neither a JSON array of objects nor one JSON object per line for the multi-line-object scenario. Then add `fn malformed_ndjson_line_is_reported_without_the_framing_diagnosis()` to tests/json_test.rs: write a fixture holding `{"a":1}\n{"a":2\n{"a":3}\n` via a new `create_malformed_ndjson` helper in tests/fixtures/mod.rs, run it through `upload_json` against `fixtures::DUMMY_DSN`, and assert the command fails, that stderr contains `"Line 2"`, and that stderr does not contain `"neither a JSON array of objects"`. Confirm `single_multi_line_json_object_is_rejected` still passes.
