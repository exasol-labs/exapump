# Plan Review Findings: fix-json-import-review-findings (round 1)

## Summary
- Axes checked: 6/6
- Total findings: 11 (Blockers: 4, Advisory: 7)
- Intent Fidelity blockers: 0
- Human-escalation blockers: 0

### Premortem

Three failure stories drove this pass.

1. `.jsonl` shipped, then a later refactor dropped it from `SUPPORTED_FORMATS`. No test caught the regression, because the only test of that scenario asserts a substring that survives the removal. See BLOCKER 1.
2. A maintainer bumped the `json_tables_core` tag, re-adopted `column_sql_type` to "follow the library", and silently restored `DECIMAL(18,0)`. The module doc comment in `src/json_tables.rs` still told that maintainer the library owns the DDL. See BLOCKER 4 and ADVISORY 6.
3. `SchemaGuard::drop` panicked while a failed assertion was unwinding. The double panic aborted the test process and hid the original assertion, which is the exact leak task 1.1 exists to close. See BLOCKER 3.

### Checks that passed

These claims were verified against source or against the running Exasol container at `localhost:8563`. None produced a finding.

- Bug 1 mechanism. `build_schema_plan` orders columns by first-seen property order (`infer.rs:281-291`), so a second file listing properties in another order does produce a reordered batch. The reorder scenario reproduces the defect rather than testing a no-op.
- Bug 1 fix, assumption row 1. Verified on the container: `IMPORT INTO <t> ("_id","a","c")` against a table without column `c` fails with `object "c" not found [line 1, column 43] (SQL state: 42000)`, at compile time, before the source endpoint is contacted. The assumption holds. Task 2.2's failing-test-first instruction also covers it.
- Bug 1 fix, plumbing. `ArrowImportOptions::columns` reaches `target_clause` through `csv_import_options` (`exarrow-rs-0.16.0/src/import/arrow.rs:650`), and `target_clause` joins the names with `, ` and adds no quoting (`src/query/import.rs:403-406`). Assumption row 2 holds.
- Bug 2 mechanism. `table_raw_name` maps `file_suffix` through `.replace("[]", "_arr").replace('.', "_")` (`contract.rs:285-295`). Path `customer.address` and path `customer_address` both render `COL_customer_address`. The collision scenario is accurate.
- Bug 3 fix, type-map fidelity. `exasol_type` as specified in task 2.1 reproduces `column_sql_type` (`contract.rs:298-306`) exactly for `Bool`, `Number`, `String`, `Null`, `Object`, and `Array`, and diverges only for `Integer`. The `Option` return preserves the physical-column filter, so no column is added or dropped.
- Bug 3 fix, range. Verified on the container: `DECIMAL(19,0)` stores both `9223372036854775807` and `-9223372036854775808` unchanged. `classify_value` routes every other number to `SimpleType::Number` (`contract.rs:56-69`). Assumption row 3 holds.
- DDL rendering fidelity. `build_sql_schema` applies `NOT NULL` when `is_required || is_null_mask` and emits `  {ident} {type}{nn}` joined with `,\n` (`ddl.rs:43-52`). Task 2.1 reproduces both. Assumption row 4 holds.
- `OPEN SCHEMA` removal. Of the nine recorded `upload/json-import-rejection` scenarios, only "Qualified table name whose schema does not exist" depends on the open. "Unqualified table name with no connection schema" still fails inside `target_schema` before any statement. The `IMPORT` statements were already fully qualified (`src/json_tables.rs:260`), so no import loses its target. The rewritten scenario text matches the `failed to create {qualified}` context at `src/json_tables.rs:292-297`.
- `OPEN SCHEMA` removal, side effect. `exasol_json_partial_family_failure_reports_loaded_tables` pre-creates `ORDERS ("_id" DATE)`. Under named import the root table now fails on `object not found` instead of a type error, and the test asserts only the table name, so it keeps passing.
- Task 2.4 mechanism. `PropertyColumns::object_fk` and `PropertyColumns::array_count` hold the literal column names, including the `_value|array` form for an array inside an array (`infer.rs:272-347`). Reading the exclusion set from `plan.properties` covers every link column.
- Task 1.1 feasibility. `exarrow-rs` defaults autocommit to true (`src/adbc_ffi.rs:2885`), so a second connection running `DROP SCHEMA` cannot deadlock against the test's own connection. The 26 call sites bind the schema without cloning it, and `tests/export_test.rs` uses its own helper, so the guard reaches every caller of `setup_exasol_schema` and no other.
- ADR promotion gate. Decision [1] moves the DDL boundary between two crates and changes a shipped column type. It passes the gate. The other 13 entries are marked `no` and belong there.
- Prose guardrails. `plan.md` and `decision-log.md` carry no em dash, no semicolon, and no contraction outside tables. `speq plan validate fix-json-import-review-findings` passes on all four deltas.
- Comment coverage. PR #48 carries 14 review comments from `antonireus`. All 14 map to a decision-log entry. The 15th comment on the PR belongs to `marconae` and was already fixed in `4938f8a`, so no ask is dropped. See ADVISORY 7.

## Intent Fidelity

#### [SCOPE_REDUCTION] ADVISORY
- Location: plan.md § Implementation Tasks, task 3.3, and decision-log.md § [9]
- Issue: review comment `4102827216` asks for two things in `docs/file_exchange.md`, a recorded trade-off and "a rough size note". Task 3.3 delivers the first and declines the second: "State the mechanism, not an unmeasured size threshold." The declination is defensible and decision-log.md § [9] records it, but plan.md never names the reviewer's ask it declines, and no follow-up measurement is scheduled anywhere.
- Fix: Add one sentence to plan.md § Impact stating that `docs/file_exchange.md` carries no size threshold and that measuring the peak-memory-to-file-size ratio is a follow-up outside this plan, so the reviewer sees the declined ask without opening the decision log.

## Feasibility

#### [EFFORT_MISESTIMATION] BLOCKER
- Location: plan.md § Implementation Tasks, task 1.2
- Issue: task 1.2 says only "Delete the trailing `DROP SCHEMA ... CASCADE` block from every test". In four tests the deleted block is the connection's only use: `exasol_csv_import_prints_row_count`, `exasol_csv_import_with_custom_delimiter`, `exasol_csv_import_with_no_header`, and `exasol_csv_flags_ignored_for_parquet`, all in `tests/csv_test.rs`. After the deletion each leaves `let (mut conn, schema_name) = ...` with `conn` unread, which raises `unused_variables` and `unused_mut`. plan.md § Verification § Checklist requires `cargo clippy` to report 0 warnings, so the plan as written fails its own gate.
- Fix: Extend task 1.2 with a second sentence: where the deleted block was the only use of the connection, change the binding to `let (_conn, schema_name)`. Name the four `tests/csv_test.rs` tests above in the task text.
- Escalation: MECHANICAL. The affected tests are listed by name and the change is a binding rename.

#### [UNSTATED_ASSUMPTION] BLOCKER
- Location: plan.md § Implementation Tasks, task 1.1, and decision-log.md § [12]
- Issue: task 1.1 specifies the drop path as "a fresh connection on a dedicated thread with its own current-thread runtime", but states no failure policy for that path. Three steps there can fail: opening the connection, executing `DROP SCHEMA`, and joining the spawned thread. `setup_exasol_schema` calls `.unwrap()` on every fallible step today (`tests/fixtures/mod.rs:113-118`), so an implementer copying that style panics inside `Drop`. A panic inside `Drop` during an unwinding assertion aborts the test process and hides the original assertion, which is the leak task 1.1 exists to close.
- Fix: Add to task 1.1: the drop path MUST discard every error rather than unwrap it, covering the connection open, the `DROP SCHEMA` execution, and the `JoinHandle` result, so `Drop` never panics while a failed assertion unwinds. Add a matching sentence to decision-log.md § [12] Consequences.
- Escalation: MECHANICAL. The existing manual cleanups already use `let _ = conn.execute_update(...)`, so the policy is readable from the codebase.

#### [NFR_IGNORED] ADVISORY
- Location: plan.md § Impact, paragraph 2
- Issue: the paragraph tells a user with an existing `DECIMAL(18,0)` table to "Recreate the table to widen it." Recreating discards every row the table holds. Verified on the container at `localhost:8563`: `ALTER TABLE <t> MODIFY COLUMN "id" DECIMAL(19,0)` widens the column in place and a 19-digit insert then succeeds, with the existing rows intact.
- Fix: Replace "Recreate the table to widen it." in plan.md § Impact with "Widen it with `ALTER TABLE <table> MODIFY COLUMN <column> DECIMAL(19,0)`, which keeps the rows already loaded." Carry the same sentence into task 3.3's `docs/file_exchange.md` section.

## Requirement Quality

#### [COMPLETENESS_GAP] BLOCKER
- Location: specs/_plans/fix-json-import-review-findings/upload/parquet-import/spec.md, and plan.md § Verification § Scenario Coverage
- Issue: the delta changes the "Unsupported file extension" scenario to list `.jsonl` among the accepted extensions, and plan.md maps it to the existing test `unsupported_file_extension`. That test asserts `stderr` contains `".parquet, .csv, .json, .ndjson"` (`tests/parquet_test.rs:63`), which stays a substring of the new list, so the test passes whether or not `.jsonl` reaches `SUPPORTED_FORMATS`. The unit test `unsupported_extension_returns_error_with_supported_formats` carries the identical stale substring (`src/format.rs:53`). plan.md § Verification then states the opposite of the truth: "Every other scenario of these four features keeps the test it already has. Two of those tests change their assertions without changing their scenario." The changed scenario has no verification.
- Fix: Extend task 2.7 to update both assertions to the full new list `.parquet, .csv, .json, .ndjson, .jsonl`, naming `unsupported_file_extension` in `tests/parquet_test.rs` and `unsupported_extension_returns_error_with_supported_formats` in `src/format.rs`. Change the count in plan.md § Verification from two tests to four, and add both test names to the sentence that lists the tests whose assertions change.
- Escalation: MECHANICAL. Both assertions and the scenario text are in the repository.

#### [IMPLEMENTATION_LEAKAGE] ADVISORY
- Location: specs/_plans/fix-json-import-review-findings/upload/json-import/spec.md § Background, paragraphs at lines 39 and 41
- Issue: two Background paragraphs state facts that no GIVEN, WHEN, or THEN step of the merged feature depends on. Line 39 gives three reasons why exapump warns rather than refusing or recreating. Line 41 states the memory behavior and that the feature adds no chunking. The "Repeated run loads into the existing family" scenario asserts that the warning is printed, not why it won. No scenario asserts memory behavior. The reviewer asked for both statements, so this is flagged rather than blocked, but a rationale belongs in an ADR, which outlives a spec Background.
- Fix: Keep both paragraphs, and record in decision-log.md § [10] and § [11] that the Background rule was knowingly relaxed at the reviewer's request, naming the two paragraphs. This keeps `/speq:audit` from re-reporting them as leakage later.

#### [AMBIGUOUS_REQUIREMENT] ADVISORY
- Location: plan.md § Verification § Manual Testing, the first `upload/json-import` row
- Issue: the row expects the dry run to print ``id: DECIMAL(19,0)``. `describe` prints the column name through `sanitize_ident` (`src/json_tables.rs:82-88`), and `sanitize_ident` wraps the name in double quotes (`contract.rs:259-261`), so the real output is `  "id": DECIMAL(19,0)`. A human checking the stated string finds no match and reports a failure.
- Fix: Change the expected output in that row to `"id": DECIMAL(19,0)` with the double quotes included, matching what `sanitize_ident` emits.

## Task Breakdown

#### [TRACEABILITY_GAP] BLOCKER
- Location: plan.md § Implementation Tasks, section 3, and § Dead Code Removal
- Issue: comment 10 reported that the DDL-ownership statement is false. Tasks 3.1 and 3.2 correct it in the ADR and in `specs/mission.md`, and the three spec deltas correct it in the Backgrounds. The same false sentence stays in the source file the plan rewrites. `src/json_tables.rs:3-4` reads "`json_tables_core` owns every decision about which tables exist, which columns they carry, and which DDL describes them." Two more doc comments reference the function the plan deletes: `qualified_name` at line 117 and `create_tables` at line 279 both explain themselves in terms of `build_sql_schema`. § Dead Code Removal lists the code to delete and names none of these comments. No task covers them.
- Fix: Add task 3.4 to plan.md § Implementation Tasks: rewrite the `src/json_tables.rs` module doc comment so it states that exapump owns the SQL it runs and `json_tables_core` owns which tables exist and which columns each carries, and rewrite the doc comments of `qualified_name` and `create_tables` so neither explains itself through the removed `build_sql_schema`. Place task 3.4 in group B, because it edits a file group B owns.
- Escalation: MECHANICAL. All three comments are in one file and their replacement text is fixed by decision [1].

#### [TASK_GRANULARITY] ADVISORY
- Location: plan.md § Implementation Tasks, tasks 2.1 and 2.5
- Issue: task 2.1 requires dropping the `TableFamily::ddl` field. Nothing can render the DDL at plan time once that field is gone, so task 2.1 already forces the move of rendering into `describe` and `create_tables`, which is the body of task 2.5. Neither task can be completed or verified alone.
- Fix: Merge task 2.5 into task 2.1 under one `[expert]` heading, or move the "Drop the `ddl` field from `TableFamily`" clause from task 2.1 into task 2.5 so each task compiles on its own.

## Design Depth

#### [INFORMATION_LEAKAGE] ADVISORY
- Location: decision-log.md § [1] Consequences, and plan.md § Implementation Tasks, task 2.1
- Issue: after this change two functions encode the same contract-type-to-Exasol-type decision. `json_tables_core::contract::column_sql_type` keeps `DECIMAL(18,0)` and the new `exasol_type` uses `DECIMAL(19,0)`. Nothing enforces the divergence. Decision [1] states the rule as prose, "A future tag bump MUST NOT restore `DECIMAL(18,0)` by re-adopting `column_sql_type`", and no test, type, or compile-time check holds it. A maintainer bumping the pinned tag reads the module doc comment, which still credits the library with the DDL, and re-adopts the library function.
- Fix: Add to task 2.1 a unit test named `exasol_type_maps_every_contract_type` that asserts the returned string for all seven `SimpleType` variants, including `Integer` to `DECIMAL(19,0)` and `None` for `Null`, `Object`, and `Array`. Add its row to plan.md § Verification § Scenario Coverage.

## Prose Quality

#### [PROSE_UNCLEAR] ADVISORY
- Location: decision-log.md § Interview, first paragraph
- Issue: the paragraph states that the orchestrator passed "15 review comments from GitHub user `antonireus`". PR #48 carries 14 comments from `antonireus`. The 15th comment on the PR, `4102614292`, belongs to `marconae`, asks that specific tags not be tracked as an ADR, and was already fixed in `4938f8a`. The numbering used throughout the log starts the `antonireus` comments at 2, which is consistent with the PR order but contradicts the attribution sentence. A reader auditing coverage cannot tell whether comment 1 was dropped.
- Fix: Change the sentence in decision-log.md § Interview to state that the orchestrator passed 15 PR comments, of which 14 come from `antonireus` and comment 1 comes from `marconae` and was already fixed in `4938f8a`, so the triage below covers comments 2 through 15.
