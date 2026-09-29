# Plan Review Findings: fix-json-import-review-findings (round 2)

## Summary
- Axes checked: 6/6
- Total findings: 11 (Blockers: 2, Advisory: 9)
- Intent Fidelity blockers: 0
- Human-escalation blockers: 1

Plan Size is `full`: `plan.md` carries a populated `## Design` section and `decision-log.md` carries 14 Design Decisions. Round 2 therefore ran the full six-axis pass after the blocker recheck.

Seven of the nine advisories are carried forward from round 1 unchanged. Each was re-verified against the current artifacts and still applies. They are marked `(carried from round 1)`.

### Premortem

Two fresh failure stories drove this pass.

1. A user uploaded a second file that omitted one optional property. The old build refused the file with `ETL-6009`. The new build accepted it and wrote NULL into the missing column. Nobody noticed, because no spec sentence, no scenario, and no `§ Impact` paragraph described that direction. See BLOCKER 1.
2. A maintainer read `build_record_batch`'s doc comment, which states that the import maps CSV fields onto columns by position, and reordered the batch fields to "match the DDL". The import still worked, because it names its columns, so the reorder shipped and the next reader trusted the comment again. See BLOCKER 2.

### Checks that passed

These claims were verified against the source or against the Exasol container at `localhost:8563`. None produced a finding.

- Task 1.2 completeness. A per-function count of `conn` references across `tests/json_test.rs`, `tests/csv_test.rs`, `tests/parquet_test.rs`, and `tests/cli_test.rs` returns exactly four tests where the binding and the `DROP SCHEMA` block are the only two uses. All four are the tests task 1.2 names. Every other test of the 26 uses the connection for a `SELECT`, a `CREATE`, or a `count_rows` call.
- Task 1.2 second-order risk. The four `cli_test.rs` tests derive `schema_upper` from `schema_name` and use it in a `CREATE SCRIPT` statement before the deleted block, so no second binding falls unused.
- Task 3.4 citations. `src/json_tables.rs:3-4` carries the DDL-ownership sentence verbatim. `qualified_name` (line 115) and `create_tables` (line 276) each explain themselves through `build_sql_schema`, and the `create_tables` comment carries the `OPEN SCHEMA` sentence.
- Task 2.7 citations. `tests/parquet_test.rs:63` and `src/format.rs:53` both assert the substring `".parquet, .csv, .json, .ndjson"`. `SUPPORTED_FORMATS` is at `src/format.rs:12`.
- 26 call sites. `setup_exasol_schema` has exactly 26 callers and exactly 26 `DROP SCHEMA` blocks follow them. `tests/export_test.rs` uses its own helper and is untouched, as decision [12] states.
- `DECIMAL(18,0)` blast radius. Outside the plan the literal appears in `specs/upload/json-import/spec.md:24`, which the delta's full-Background `DELTA:CHANGED` block replaces, and in `tests/json_test.rs:31`, which task 2.1 updates. The two remaining hits (`tests/csv_test.rs:271`, `tests/parquet_test.rs:93`) are hand-written CSV and Parquet fixtures unrelated to JSON typing.
- No recorded-spec conflict. The recorded scenario "Repeated run loads into the existing family" re-uploads the same file, so the named-import rules do not contradict it. The other eight recorded `upload/json-import` scenarios and nine `upload/json-import-rejection` scenarios are unaffected by the deltas.
- ADR promotion gate. The four new `[plan-review]` entries in `decision-log.md` § Review Findings are all marked `Promotes to ADR: no`. Each records a revision to a task, not a change in behavior, architecture, or design, so the gate is met. Decision [1] remains the only `yes`.
- Validator. `speq plan validate fix-json-import-review-findings` passes on all four deltas.
- Prose markers. `plan.md`, `decision-log.md`, and the four deltas carry no em dash outside table cells, no semicolon, and no contraction.

## Round-1 Blocker Recheck

- Resolved: [EFFORT_MISESTIMATION] Task 1.2 left four connection bindings unused — `plan.md` task 1.2 now reads "Where the deleted block was the connection's only use, change the binding to `let (_conn, schema_name)`" and names `exasol_csv_import_prints_row_count`, `exasol_csv_import_with_custom_delimiter`, `exasol_csv_import_with_no_header`, and `exasol_csv_flags_ignored_for_parquet`. An independent per-function count of `conn` uses across all four test files returns those four tests and no others, so the list is complete, not merely restated.
- Resolved: [UNSTATED_ASSUMPTION] The `SchemaGuard` drop path had no failure policy — `plan.md` task 1.1 now reads "The drop path MUST discard every error instead of unwrapping it, across the connection open, the `DROP SCHEMA` execution, and the `JoinHandle` result", points at the existing `let _ = conn.execute_update(...)` cleanups as the policy, and carries "CAUTION: a panic inside `Drop` while a failed assertion unwinds aborts the test process and hides the original assertion." `decision-log.md` § [12] Consequences states the same policy and names a leaked schema as the failure mode of a broken cleanup.
- Resolved: [COMPLETENESS_GAP] The `.jsonl` scenario had no verification — `plan.md` task 2.7 now names both stale assertions and requires the full list `.parquet, .csv, .json, .ndjson, .jsonl`. § Verification raises the count from two tests to four and names `unsupported_file_extension` in `tests/parquet_test.rs` and `unsupported_extension_returns_error_with_supported_formats` in `src/format.rs`. Both assertions were re-confirmed in the repository at `tests/parquet_test.rs:63` and `src/format.rs:53`. Either test now fails if `.jsonl` never reaches `SUPPORTED_FORMATS`.
- Resolved: [TRACEABILITY_GAP] No task corrected the doc comments in `src/json_tables.rs` — `plan.md` task 3.4 exists, names the module doc comment at lines 3-4, the `qualified_name` doc comment, and the `create_tables` doc comment, and fixes the replacement content. § Parallelization lists group B as `2.1-2.8, 3.4` and states why task 3.4 sits there. All three cited locations were re-confirmed in the source. This fix is complete for the three comments round 1 named. Three further comments in the same file stay stale, which BLOCKER 2 below raises as a new finding rather than a re-opening of this one.

## Intent Fidelity

No new finding. Every one of the 14 `antonireus` review comments still maps to a decision-log entry, and the four round-2 revisions added no work outside the 15 comments and the orchestrator's upstream investigation. Task 3.4 stays inside comment 10's ask.

#### [SCOPE_REDUCTION] ADVISORY (carried from round 1)
- Location: plan.md § Implementation Tasks, task 3.3, and decision-log.md § [9]
- Issue: review comment `4102827216` asks for two things in `docs/file_exchange.md`, a recorded trade-off and "a rough size note". Task 3.3 delivers the first and declines the second: "State the mechanism, not an unmeasured size threshold." decision-log.md § [9] records the declination, but plan.md § Impact still names neither the declined ask nor a follow-up. Re-verified: § Impact carries no such sentence.
- Fix: Add one sentence to plan.md § Impact stating that `docs/file_exchange.md` carries no size threshold and that measuring the peak-memory-to-file-size ratio is a follow-up outside this plan, so the reviewer sees the declined ask without opening the decision log.

## Feasibility

#### [EFFORT_MISESTIMATION] ADVISORY
- Location: plan.md § Implementation Tasks, task 2.1
- Issue: task 2.1 names one import to delete, "the `json_tables_core::ddl::build_sql_schema` import", and misses a second one that the same task orphans. `column_sql_type` is imported at `src/json_tables.rs:18` and used at exactly one place, `physical_columns` at line 172. Task 2.1 repoints `physical_columns` at the new `exasol_type`, so the import falls unused and `cargo build` and `cargo clippy` both report `unused_imports`. § Verification § Checklist requires `cargo clippy` to report 0 warnings. This is the same defect class as round 1's task 1.2 finding, at a location round 1 did not check, and it is raised as advisory rather than blocking because the compiler names the import directly.
- Fix: Extend task 2.1's deletion sentence to read "Drop the `ddl` field from `TableFamily` and the `json_tables_core::ddl::build_sql_schema` and `json_tables_core::contract::column_sql_type` imports."

#### [NFR_IGNORED] ADVISORY (carried from round 1)
- Location: plan.md § Impact, paragraph 2
- Issue: the paragraph still tells a user with an existing `DECIMAL(18,0)` table to "Recreate the table to widen it." Recreating discards every row the table holds. Verified on the container at `localhost:8563` in round 1: `ALTER TABLE <t> MODIFY COLUMN "id" DECIMAL(19,0)` widens the column in place, a 19-digit insert then succeeds, and the existing rows stay.
- Fix: Replace "Recreate the table to widen it." in plan.md § Impact with "Widen it with `ALTER TABLE <table> MODIFY COLUMN <column> DECIMAL(19,0)`, which keeps the rows already loaded." Carry the same sentence into task 3.3's `docs/file_exchange.md` section.

## Requirement Quality

#### [COMPLETENESS_GAP] BLOCKER
- Location: specs/_plans/fix-json-import-review-findings/upload/json-import/spec.md § Background line 33, specs/_plans/fix-json-import-review-findings/upload/json-import-rejection/spec.md § Scenarios, and plan.md § Impact paragraph 3
- Issue: the plan defines one direction of the column-shape mismatch and leaves the other undefined, and § Impact then states the wrong outcome for it. The Background says "A batch column that the target table does not hold fails the import", and the rejection delta adds the scenario "Second file carries a column the target table does not hold". No artifact says what happens when the target table holds a column the batch does not name. That case changes behavior. Verified on the container at `localhost:8563` against `target/debug/exapump`: uploading `[{"a":1,"b":"x"},{"a":2}]` and then `[{"a":3}]` into the same table fails today with `ETL-6009: Number of columns in source (=2) and destination (=3) table differs`. After task 2.2 the statement becomes `IMPORT INTO <t> ("_id", "a")`, which Exasol accepts and leaves `b` at NULL. Also verified on the container: `json_tables_core` marks only `_id` `NOT NULL`, so a data column is nullable even when every document of the first file carried the property, and nothing stops the NULL-filled load. plan.md § Impact paragraph 3 therefore states the opposite of the new behavior for this direction: "A repeated upload into a table whose columns do not match the new file now fails instead of loading values into the wrong columns." A second file that drops one property now succeeds where it previously failed, and the row it writes carries a NULL the user never wrote.
- Fix: Decide whether a batch that omits a column the target table holds loads with NULL or is rejected, and record the choice in decision-log.md § [2] Consequences with both options named. Then, for the chosen behavior: add one sentence to `upload/json-import` § Background stating it, add a `DELTA:NEW` scenario covering a second file that omits a property the first file carried (to `upload/json-import` for the NULL-load behavior, to `upload/json-import-rejection` for the rejection behavior), add its row to plan.md § Verification § Scenario Coverage with a new integration test in `tests/json_test.rs`, add the implementing clause to task 2.2, and rewrite plan.md § Impact paragraph 3 to state both directions rather than only the failing one.
- Escalation: HUMAN. The choice changes a user-visible outcome, from a hard `ETL-6009` failure to a silent NULL-filled load, in a plan whose stated goal is to stop the import writing values the user did not supply. Both options are defensible, the plan's own artifacts settle neither, and the requester reviewed the opposite direction explicitly.

#### [AMBIGUOUS_REQUIREMENT] ADVISORY
- Location: plan.md § Verification § Manual Testing, the `upload/json-import-nesting` row
- Issue: the row expects the dry run to print a `CREATE TABLE IF NOT EXISTS` statement for `"SALES"."N"`, `"SALES"."N_customer"`, and `"SALES"."N_items_arr"`, "each with `"_id" DECIMAL(19,0) NOT NULL`". The nesting delta's own Background states the opposite rule: "An array element table carries `_parent` and `_pos`, and carries `_id` only when it holds a nested array of its own." The input `[{"id":1,"customer":{"tier":"gold"},"items":[{"sku":"S1"}]}]` gives `items` elements no nested array, so `"SALES"."N_items_arr"` carries `_parent`, `_pos`, and `sku`, and no `_id`. Confirmed against the equivalent shipped fixture: `exasol_json_nested_tables_carry_the_generated_key_columns` joins `ORDERS_items_arr` to its parent through `_parent`, while `exasol_json_multi_level_nesting_creates_all_subtables` joins through `DEEP_items_arr."_id"` only because that table holds a nested `tags` array. A human running the manual step finds no `_id` on the array table and reports a failure the code does not have.
- Fix: Change the expected output in that row to "Prints a `CREATE TABLE IF NOT EXISTS` statement for `"SALES"."N"`, `"SALES"."N_customer"`, and `"SALES"."N_items_arr"`. The first two carry `"_id" DECIMAL(19,0) NOT NULL`. `"SALES"."N_items_arr"` carries `"_parent"` and `"_pos"` and no `"_id"`."

#### [IMPLEMENTATION_LEAKAGE] ADVISORY (carried from round 1)
- Location: specs/_plans/fix-json-import-review-findings/upload/json-import/spec.md § Background, paragraphs at lines 39 and 41
- Issue: two Background paragraphs state facts that no GIVEN, WHEN, or THEN step of the merged feature depends on. Line 39 gives three reasons why exapump warns rather than refusing or recreating. Line 41 states the memory behavior and that the feature adds no chunking. The "Repeated run loads into the existing family" scenario asserts that the warning is printed, not why it won. No scenario asserts memory behavior. The reviewer asked for both statements, so this is flagged rather than blocked. Re-verified: both paragraphs are unchanged and decision-log.md § [10] and § [11] still record no relaxation.
- Fix: Keep both paragraphs, and record in decision-log.md § [10] and § [11] that the Background rule was knowingly relaxed at the reviewer's request, naming the two paragraphs. This keeps `/speq:audit` from re-reporting them as leakage later.

#### [AMBIGUOUS_REQUIREMENT] ADVISORY (carried from round 1)
- Location: plan.md § Verification § Manual Testing, the first `upload/json-import` row
- Issue: the row still expects the dry run to print ``id: DECIMAL(19,0)``. `describe` prints the column name through `sanitize_ident` (`src/json_tables.rs:82-88`), and `sanitize_ident` wraps the name in double quotes, so the real output is `  "id": DECIMAL(19,0)`. A human checking the stated string finds no match and reports a failure.
- Fix: Change the expected output in that row to `"id": DECIMAL(19,0)` with the double quotes included, matching what `sanitize_ident` emits.

## Task Breakdown

#### [TRACEABILITY_GAP] BLOCKER
- Location: plan.md § Implementation Tasks, task 3.4, and § Dead Code Removal
- Issue: task 3.4 corrects the three doc comments round 1 named and leaves three more in the same file that this plan makes false. `TableFamily` at `src/json_tables.rs:29-34` reads "The family is ordered: `plans`, `ddl`, and every row count this module returns share the order `build_all_schema_plans` produced", and task 2.1 deletes the `ddl` field the sentence names. `load` at lines 228-237 reads "The returned vector holds the row count loaded per table ... The returned option holds the failure" and "the counts come back beside the error rather than instead of it", and task 2.6 replaces that pair return with an accumulator and `anyhow::Result<()>`. `build_record_batch` at lines 313-318 reads "Field order is the CREATE statement's column order, because the import maps CSV fields onto table columns by position rather than by name", which is the exact defect task 2.2 removes, so the comment will tell the next reader that the fixed bug is still the contract. A fourth comment narrows rather than falsifies: `GENERATED_KEY_COLUMNS` at lines 25-27 states "A table holding nothing else carries no data that came out of the documents", and task 2.4 widens that rule to the `<name>|object` and `<name>|array` link columns, which the const does not hold. § Dead Code Removal names none of the four. No task covers them, and no gate catches them, because a stale comment breaks neither the build nor a test.
- Fix: Extend plan.md task 3.4 from three comments to six. Rewrite the `TableFamily` doc comment so it names `plans` and the row counts and not the deleted `ddl` field. Rewrite the `load` doc comment to describe the accumulator parameter and the `anyhow::Result<()>` return, keeping the statement that the import is not atomic across the family. Rewrite the `build_record_batch` doc comment to state that the import names its columns, so field order no longer decides which column a value lands in, and that the field order still follows `physical_columns`. Restate the `GENERATED_KEY_COLUMNS` doc comment so it says the const holds the fixed generated names only, and that `reject_unusable_plans` adds the link columns from `plan.properties`.
- Escalation: MECHANICAL. All four comments are in one file, their staleness follows from tasks 2.1, 2.2, 2.4, and 2.6, and their replacement content is fixed by decisions [1], [2], [4], and [6].

#### [TASK_GRANULARITY] ADVISORY (carried from round 1)
- Location: plan.md § Implementation Tasks, tasks 2.1 and 2.5
- Issue: task 2.1 requires dropping the `TableFamily::ddl` field. Nothing can render the DDL at plan time once that field is gone, so task 2.1 already forces the move of rendering into `describe` and `create_tables`, which is the body of task 2.5. Neither task can be completed or verified alone. Re-verified: both task texts are unchanged.
- Fix: Merge task 2.5 into task 2.1 under one `[expert]` heading, or move the "Drop the `ddl` field from `TableFamily`" clause from task 2.1 into task 2.5 so each task compiles on its own.

## Design Depth

No new finding. The round-2 revisions added one task and three task clauses and introduced no module, interface, or boundary. Decision [1] stays the only ADR promotion and still passes the gate. Group B remains one cluster over one spec-delta set and one source file, which § Parallelization justifies.

#### [INFORMATION_LEAKAGE] ADVISORY (carried from round 1)
- Location: decision-log.md § [1] Consequences, and plan.md § Implementation Tasks, task 2.1
- Issue: after this change two functions encode the same contract-type-to-Exasol-type decision. `json_tables_core::contract::column_sql_type` keeps `DECIMAL(18,0)` and the new `exasol_type` uses `DECIMAL(19,0)`. Nothing enforces the divergence. Decision [1] states the rule as prose, "A future tag bump MUST NOT restore `DECIMAL(18,0)` by re-adopting `column_sql_type`", and no test, type, or compile-time check holds it. Re-verified: task 2.1 still specifies no unit test for the mapping, and § Verification § Scenario Coverage carries no row for one.
- Fix: Add to task 2.1 a unit test named `exasol_type_maps_every_contract_type` that asserts the returned string for all seven `SimpleType` variants, including `Integer` to `DECIMAL(19,0)` and `None` for `Null`, `Object`, and `Array`. Add its row to plan.md § Verification § Scenario Coverage.

## Prose Quality

No new finding. The text added in round 2 (task 1.1's CAUTION, task 1.2's binding sentence, task 2.7's assertion list, task 3.4, and the four `decision-log.md` § Review Findings entries) carries no em dash, no semicolon, no contraction, no superlative, and no weak modal. The `§ Review Findings` entries narrate a revision, which is that section's purpose under `/speq:planning`, not process narration in governed prose.

#### [PROSE_UNCLEAR] ADVISORY (carried from round 1)
- Location: decision-log.md § Interview, first paragraph
- Issue: the paragraph still states that the orchestrator passed "15 review comments from GitHub user `antonireus`". PR #48 carries 14 comments from `antonireus`. The 15th comment on the PR, `4102614292`, belongs to `marconae`, asks that specific tags not be tracked as an ADR, and was already fixed in `4938f8a`. The numbering used throughout the log starts the `antonireus` comments at 2, which contradicts the attribution sentence. A reader auditing coverage cannot tell whether comment 1 was dropped.
- Fix: Change the sentence in decision-log.md § Interview to state that the orchestrator passed 15 PR comments, of which 14 come from `antonireus` and comment 1 comes from `marconae` and was already fixed in `4938f8a`, so the triage below covers comments 2 through 15.
