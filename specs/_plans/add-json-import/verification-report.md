# Verification Report: add-json-import

## Verdict

| Result | Details |
|--------|---------|
| **PASS** | JSON and NDJSON import, plus the two prior review rounds' fixes, are implemented, tested, and verified against a live Exasol container. |
| Code review | 7 findings — 7 fixed |

| Check | Status |
|-------|--------|
| Build | ✓ |
| Tests | ✓ |
| Lint | ✓ |
| Format | ✓ |
| Scenario Coverage | ✓ (one pre-flagged advisory gap, see Notes) |
| Manual Tests | ✓ |

## Test Evidence

### Test Results

| Type | Run | Passed | Failed | Ignored |
|------|-----|--------|--------|---------|
| Unit + Integration (all binaries, `cargo test`) | 581 | 581 | 0 | 1 (pre-existing, manual-only: `wait_dumps_logs_when_container_crashes`) |

Full run: `target/speq-test.log`. Suite breakdown (`test result:` lines): 363, 17, 54, 17, 4, 37, 23, 8, 47, 2, 9 across the 11 test binaries.

### Manual Tests

| Test | Command | Result |
|------|---------|--------|
| Dry-run shows planned family | `exapump upload orders.json --table sales.orders --dsn '...' --dry-run` | ✓ Listed `"SALES"."ORDERS"`, `"SALES"."ORDERS_customer"`, `"SALES"."ORDERS_items_arr"` with columns and `CREATE TABLE IF NOT EXISTS` statements. Exit 0. |
| Real JSON import | Same file, no `--dry-run`, against a container-created `SALES` schema | ✓ `Imported 2 rows into "SALES"."ORDERS_customer"`, `Imported 3 rows into "SALES"."ORDERS_items_arr"`, `Imported 2 rows into "SALES"."ORDERS"`, `Imported 7 rows in total`. `SELECT COUNT(*)` on each table matched. Unconditional `_id`-repeat warning printed to stderr. |
| NDJSON import | `exapump upload events.ndjson --table raw.events --dsn '...'` | ✓ `Imported 2 rows into "RAW"."EVENTS"`. `SELECT COUNT(*)` matched. |
| Unsupported format | `exapump upload notes.txt --table sales.orders --dsn '...'` | ✓ `Error: file format "txt" is not supported. Supported formats: .parquet, .csv, .json, .ndjson` |
| `--help` documents JSON subtables | `exapump upload --help` | ✓ `--table` description states JSON/NDJSON input creates a root table plus one subtable per nested path. |

One deviation from the plan's literal manual-test transcript, not a defect: `OPEN SCHEMA` does not create the schema (matches exapump's existing CSV/Parquet behavior), so the target schema had to exist before either manual import ran. The plan's own Design section already documents this ("`load` resolves exactly one target schema... `load` then runs `OPEN SCHEMA`"); the Manual Testing table just didn't spell out the schema-creation precondition.

## Tool Evidence

### Linter

```
cargo clippy --all-targets --all-features -- -D warnings
(clean, exit 0 — target/speq-clippy.log)
```

### Formatter

```
cargo fmt --check
(clean, exit 0 — target/speq-fmt.log)
```

### License / Advisory Gates

```
cargo deny check licenses    → exit 0 (target/speq-deny-licenses.log)
cargo deny check advisories  → exit 0 (target/speq-deny-advisories.log)
```

`cargo deny check advisories` initially failed on RUSTSEC-2026-0285 (rustls 0.23.37). Code review confirmed this was pre-existing on `origin/main` and separately confirmed it was fixable in-range; the review-fix pass ran `cargo update -p rustls`, resolving to 0.23.45 inside the `Cargo.toml` `0.23` constraint. No `deny.toml`/`about.toml` change was needed.

## Scenario Coverage

| Domain/Feature | Scenario | Test Location | Test Name | Passes |
|---|---|---|---|---|
| upload/json-import | Import a flat JSON array into one table | `tests/json_test.rs` | `exasol_json_flat_import_creates_one_table` | Pass |
| upload/json-import | Import nested JSON creates a subtable per nested path | `tests/json_test.rs` | `exasol_json_nested_import_creates_a_subtable_per_path` | Pass |
| upload/json-import | Nested tables carry the generated key columns | `tests/json_test.rs` | `exasol_json_nested_tables_carry_the_generated_key_columns` | Pass |
| upload/json-import | Import an NDJSON file | `tests/json_test.rs` | `exasol_ndjson_import_skips_blank_lines` | Pass |
| upload/json-import | Framing detected from file content, not extension | `tests/json_test.rs` | `exasol_json_framing_is_detected_from_content_not_extension` | Pass |
| upload/json-import | Dry-run shows the planned table family | `tests/json_test.rs` | `dry_run_shows_the_planned_table_family` | Pass |
| upload/json-import | Property with mixed scalar types gets alternate columns | `tests/json_test.rs` | `exasol_json_mixed_scalar_types_get_alternate_columns` | Pass |
| upload/json-import | Explicit JSON null stays distinct from an absent property | `tests/json_test.rs` | `exasol_json_explicit_null_stays_distinct_from_an_absent_property` | Pass |
| upload/json-import | Repeated run loads into the existing family | `tests/json_test.rs` | `exasol_json_repeated_run_appends_to_existing_family` | Pass |
| upload/json-import | Unqualified table name uses the connection schema | `tests/json_test.rs` | `exasol_json_unqualified_table_uses_the_connection_schema` | Pass |
| upload/json-import | Unqualified table name with no connection schema | `tests/json_test.rs` | `exasol_json_unqualified_table_without_a_connection_schema_fails` | Pass |
| upload/json-import | Import failure reports the tables already loaded | `tests/json_test.rs` | `exasol_json_partial_family_failure_reports_loaded_tables` | Pass |
| upload/json-import | Empty JSON file | `tests/json_test.rs` | `empty_json_file_is_rejected` | Pass |
| upload/json-import | JSON file with no documents | `tests/json_test.rs` | `json_file_with_no_documents_is_rejected` | Pass |
| upload/json-import | Documents with no properties | `tests/json_test.rs` | `documents_with_no_properties_are_rejected` | Pass |
| upload/json-import | Document that is not a JSON object | `tests/json_test.rs` | `non_object_array_entry_is_rejected_with_its_position` | Pass |
| upload/json-import | JSON file not found | `tests/json_test.rs` | `json_file_not_found` | Pass |
| upload/json-import | Connection failure | `tests/json_test.rs` | `json_connection_failure` | Pass |
| parquet-import | Unsupported file extension | `tests/parquet_test.rs` | `unsupported_file_extension` | Pass |
| upload-command-structure | CSV flags ignored for Parquet files (JSON/NDJSON extension) | `tests/json_test.rs` | `dry_run_accepts_and_ignores_the_delimiter_flag` | Pass (dry-run only, see Notes) |
| upload-command-structure | Upload help describes the table family for JSON input | `tests/cli_test.rs` | `upload_help_documents_json_subtable_creation` | Pass |
| Extension-to-format mapping (supporting) | — | `src/format.rs` | `json_extension_returns_json`, `ndjson_extension_returns_json`, `uppercase_json_extension_returns_json`, `unsupported_extension_returns_error_with_supported_formats` | Pass |

Plus 4 tests added by the review-fix pass, beyond the plan's original list: `exasol_json_missing_target_schema_fails`, `build_ddl_rejects_an_unrecognised_create_statement`, `exasol_json_array_empty_in_every_document_creates_an_empty_subtable`, `ddl_column_order_matches_record_batch_field_order`.

## Notes

- **Test names differ from the plan's literal `tests/json_test.rs` names.** The implementers used a more descriptive, consistently `exasol_json_`/`exasol_ndjson_`-prefixed convention for the container-backed tests (for example `exasol_json_flat_import_creates_one_table` instead of the plan's `import_flat_json_array_creates_one_table`) and slightly reworded the dry-run and rejection test names. Every scenario in the plan's Scenario Coverage table has a corresponding passing test; only the literal identifier changed. Not treated as a defect.
- **Known, pre-accepted advisory gap (round-2 review, not fixed, not blocking):** the "CSV flags ignored for Parquet files" scenario's second clause, "the command MUST proceed with the import for the detected format as normal," is exercised only via `--dry-run` (`dry_run_accepts_and_ignores_the_delimiter_flag`), not via a real import against the Exasol container. This was flagged as a round-2 `[TRACEABILITY_GAP]` ADVISORY finding (posted to PR #48's review comment) and was explicitly out of scope for the open-questions fix pass, which touched only the two round-2 BLOCKERs. Neither `/speq:implement`'s code-reviewer nor the fix pass caught it, since it is a plan-review-level test-mapping gap rather than a code-quality defect. Recommend a follow-up test if this scenario's second clause needs container-backed coverage.
- **`_id` restart warning is unconditional by design**, printed on every JSON/NDJSON import regardless of whether the target family already exists, per decision-log.md [7] and the round-2 open-questions resolution. Confirmed live in manual testing.
- The review-fix pass bumped `rustls` 0.23.37 → 0.23.45 (plus transitive `aws-lc-rs`/`aws-lc-sys`/`rustls-webpki`) to clear a pre-existing security advisory unrelated to this feature's own new dependencies (`json_tables_core`, `serde_json`). License-exception churn: none: the fix agent confirmed `aws-lc-sys`'s license field is unchanged between the old and new pinned version.
