# Verification Report: fix-json-import-review-findings

## Verdict

| Result | Details |
|--------|---------|
| **PASS** | 3 real bugs fixed (positional-column corruption, table-name collision, 19-digit integer overflow), 3 gaps closed (`.jsonl` support, a clearer multi-line-object error, the still-open empty-object-with-link-columns check), spec/ADR/doc accuracy corrected, and the test harness's DRY/resource-leak problem fixed. Verified against a live Exasol container. |
| Code review | 4 findings — 4 fixed |

| Check | Status |
|-------|--------|
| Build | ✓ |
| Tests | ✓ |
| Lint | ✓ |
| Format | ✓ |
| Scenario Coverage | ✓ |
| Manual Tests | ✓ |

## Test Evidence

### Test Results

| Type | Run | Passed | Failed | Ignored |
|------|-----|--------|--------|---------|
| Unit + Integration (all binaries, `cargo test`) | 593 | 593 | 0 | 1 (pre-existing, manual-only: `wait_dumps_logs_when_container_crashes`) |

Full run: `target/speq-fix-test.log`. Up from 583 on the prior recorded baseline: +10 net (new tests added by this plan, minus the 1 tautological test deleted as dead-per-refactor, see Notes).

### Manual Tests

| Test | Command | Result |
|------|---------|--------|
| 19-digit integer, dry-run | `upload ids.json --table sales.ids --dry-run` | ✓ `"id": DECIMAL(19,0)` |
| `.jsonl` import | `upload e.jsonl --table sales.e` | ✓ `Imported 2 rows into "SALES"."E"` |
| Colliding table names | `upload c.json --table sales.col` (paths `customer.address` and `customer_address`) | ✓ Rejected before any table created: `two JSON paths ... produce the table name COL_customer_address: customer.address and customer_address`. |
| Second file, extra column | Two uploads into `sales.m`, second carries column `c` the table lacks | ✓ Second upload fails: `failed to import into "SALES"."M", naming the columns "_id", "a", "c"` / `object "c" not found`. First row (`from_b`) intact and unchanged. |
| Multi-line pretty-printed object | `upload s.json --table sales.s` | ✓ `the file is neither a JSON array of objects nor one JSON object per line`, chained under the real parser error. |
| Hollow object `{"a": {}}` | `upload h.json --table sales.h` | ✓ Rejected: `every planned table carries only generated columns ... and the "<name>|object" and "<name>|array" columns`. No table created (confirmed via failed `SELECT`). |
| Second file, missing column (NULL-load) | Two uploads into `sales.om`, second omits column `b` | ✓ Both succeed, exit 0. Second row: `b` is NULL. First row's `b = 'x'` untouched. |
| Nested dry-run, `_id` presence | `upload n.json --table sales.n --dry-run` | ✓ `N` and `N_customer` carry `"_id" DECIMAL(19,0) NOT NULL`; `N_items_arr` carries no `_id` (its items hold no further nesting). |
| Unsupported extension | `upload x.txt --table sales.x` | ✓ `Supported formats: .parquet, .csv, .json, .ndjson, .jsonl` |

## Tool Evidence

### Linter

```
cargo clippy --all-targets --all-features -- -D warnings
(clean, exit 0 — target/speq-fix-clippy.log)
```

### Formatter

```
cargo fmt --check
(clean, exit 0 — target/speq-fix-fmt.log)
```

### Spec Validation

```
speq plan validate fix-json-import-review-findings
Validated 4 delta spec(s): upload/json-import, upload/json-import-nesting, upload/json-import-rejection, upload/parquet-import
```

## Scenario Coverage

| Domain/Feature | Scenario | Test Name | Passes |
|---|---|---|---|
| upload/json-import | Import an NDJSON file with the .jsonl extension | `exasol_jsonl_extension_imports_one_row_per_line` | Pass |
| upload/json-import | Integer column holds the full 64-bit signed range | `exasol_json_nineteen_digit_integer_imports_unchanged` | Pass |
| upload/json-import | Repeated run places each value in the column of its own name | `exasol_json_reordered_properties_keep_each_value_in_its_column` | Pass |
| upload/json-import | Second file omits a column the target table holds (loads NULL) | `exasol_json_second_file_omitting_a_column_loads_null` | Pass |
| upload/json-import-rejection | Qualified table name whose schema does not exist | `exasol_json_missing_target_schema_fails` | Pass |
| upload/json-import-rejection | Documents with no properties | `documents_with_no_properties_are_rejected` | Pass |
| upload/json-import-rejection | Nested object that holds no properties | `nested_object_with_no_properties_is_rejected` | Pass |
| upload/json-import-rejection | Two JSON paths map to one table name | `colliding_table_names_are_rejected` | Pass |
| upload/json-import-rejection | Second file carries a column the target table does not hold | `exasol_json_second_file_with_an_unknown_column_fails` | Pass |
| upload/json-import-rejection | Single JSON object spread over several lines | `single_multi_line_json_object_is_rejected` | Pass |
| upload/parquet-import | Unsupported file extension | `unsupported_file_extension` | Pass |

Plus `jsonl_extension_returns_json` (unit, `src/format.rs`) and 3 tests added during the review-fix pass beyond the plan's original list: `create_table_ddl_marks_required_and_null_mask_columns_not_null`, `malformed_ndjson_line_is_reported_without_the_framing_diagnosis`, and the `colliding_table_names_are_rejected` predicate tightening (same test, stronger assertion).

## Notes

- **The self-rendered-DDL refactor (task 2.1) made one prior test tautological**, so the implementer deleted it rather than update it: `ddl_column_order_matches_record_batch_field_order` and its helpers (`declared_columns`, `leading_quoted_ident`, ~80 lines). Both the DDL and the Arrow batch now derive from a single `physical_columns()` call, so a column-order mismatch between them is no longer constructible — the plan's own Patterns table already named this test as something the refactor makes obsolete. Code review confirmed this reasoning holds.
- **One API-signature correction found during implementation**: the plan's task 2.2 described `ArrowImportOptions::columns` loosely; the implementer confirmed against `exarrow-rs` source that it takes `Vec<String>`, not `Vec<&str>`, and implemented accordingly.
- **The NULL-load decision for the omitted-column case was made by a human** (this session), after round-2 adversarial review flagged it as a genuine behavior choice the plan hadn't settled (`Escalation: HUMAN`). Both directions were explained; NULL-load was chosen to match normal JSON optional-property semantics.
- **Code review found 4 issues, all fixed**: an unused import left over from the DDL refactor, a missing unit test for the `NOT NULL` rule in the new `create_table_ddl` (added, verified load-bearing by a mutation check), a duplicate-predicate simplification in the collision test, and a fix to the multi-line-object error context so it no longer misfires on a genuinely malformed NDJSON line partway through a file (added a document-count gate, verified with a new RED/GREEN test).
- **9 advisory findings from the plan's own round-2 review remain open** (prose/completeness notes, e.g. the `ALTER TABLE ... MODIFY COLUMN` widening note for existing `DECIMAL(18,0)` tables, a manual-testing quote-formatting nit). None block recording; they'll be noted on the PR per this branch's established convention.
