# Decision Log: fix-json-import-review-findings

## Interview

Interview Mode was headless. No live interview took place. The orchestrator passed 15 review comments from GitHub user `antonireus` on PR #48, collected verbatim, plus one piece of upstream investigation. The entries below record the input and the triage.

**Q:** Which of the 15 review comments are real correctness defects?
**A:** Three. Comment 2: the import maps columns by position, so a second file with a different property set loads into the first file's columns and reports success. Comment 3: two different JSON paths can produce one table name, so one table is created once and loaded twice. Comment 4: `DECIMAL(18,0)` rejects a 19-digit integer that `json_tables_core` classifies as an integer.

**Q:** Which comments are gaps to close rather than defects?
**A:** Three. Comment 5 asks for the `.jsonl` extension. Comment 6 asks for support or a clearer error for a single pretty-printed JSON object. Comment 7 is an unfixed round-2 ADVISORY finding from the `add-json-import` plan review that a human independently re-found, so this plan treats it as a defect.

**Q:** Which comments ask for prose accuracy only?
**A:** Four. Comment 8: the Background claims NDJSON "streams the read pass", which reads as if NDJSON bounds memory. Comment 9: the Background states the `_id` limit without the reason the warning won. Comment 10: `specs/_decision/008-add-json-import.md` claims `json_tables_core` owns the DDL, but `build_ddl` and `rewrite_create` rewrite it. Comment 14: three Background paragraphs are copied word for word between `upload/json-import` and `upload/json-import-rejection`.

**Q:** Which comments propose simplifications?
**A:** Three. Comment 11: build the `CREATE TABLE` statement from `physical_columns` instead of rewriting the upstream text by prefix match. Comment 12: resolve the schema once and use qualified names everywhere instead of mixing `OPEN SCHEMA` with qualified names. Comment 13: replace the five "return what loaded so far plus the error" blocks with `?`.

**Q:** What did the orchestrator verify before handing over?
**A:** That `DECIMAL(18,0)` is hardcoded upstream, at `crates/json_tables_core/src/contract.rs:298` and `:314` in the pinned `v0.3` checkout, not in exapump. Comments 4, 10, and 11 are therefore entangled, and one design change can resolve all three.

**Q:** What does comment 15 ask for?
**A:** A DRY and resource-leak fix in `tests/json_test.rs`. The same upload command block appears about 25 times and the schema cleanup about 14 times. The cleanup does not run when an assertion fails, so schemas accumulate.

## Design Decisions

### [1] exapump renders its own `CREATE TABLE` and maps an integer to `DECIMAL(19,0)`

- **Decision:** exapump stops calling `json_tables_core::ddl::build_sql_schema` and renders the `CREATE TABLE IF NOT EXISTS` text from `PlannedTable::columns`. exapump maps a contract type to an Exasol type itself, and maps `SimpleType::Integer` to `DECIMAL(19,0)`. `json_tables_core` keeps ownership of which tables exist, which columns each table carries, and which contract type each column has.
- **Alternatives:** Fix `column_sql_type` in `exasol-labs/exasol-json-tables` and bump the pinned tag. Rejected, because it blocks PR #48 on another repository's release, which ADR `reuse-json-tables-core-via-git-dependency` already rejected as a dependency strategy. Document the 18-digit limit in the spec and change no code. Rejected, because it leaves a defect that produces `ETL-3050 numeric value out of range` after the tables already exist.
- **Rationale:** `classify_value` admits every 64-bit signed integer into an integer column, and `DECIMAL(19,0)` is the narrowest Exasol type covering that range. A value outside the range is classified as a fractional number and lands in `DOUBLE`, so no wider type is reachable. exapump already rewrote the upstream statement by prefix match, so the DDL boundary had already moved. Taking the rendering over completes that move, removes the rewrite error path, and makes column order correct by construction.
- **Consequences:** `rewrite_create`, `build_ddl`, the `TableFamily::ddl` field, and the two SQL-parsing test helpers are deleted, which resolves comment 11. The Consequences line of `specs/_decision/008-add-json-import.md` and the `json_tables_core` row of `specs/mission.md` § Tech Stack both become false and are corrected in tasks 3.1 and 3.2, which resolves comment 10. exapump now diverges from upstream on one type width. A future tag bump MUST NOT restore `DECIMAL(18,0)` by re-adopting `column_sql_type`. A table an earlier exapump version created keeps `DECIMAL(18,0)`, because `CREATE TABLE IF NOT EXISTS` alters nothing.
- **Promotes to ADR:** yes

### [2] Import rows by column name

- **Decision:** `import_batch` builds the column list from `batch.schema().fields()`, quotes each name with `sanitize_ident`, and passes the list to `ArrowImportOptions::columns`. The failure context names the columns exapump sent.
- **Alternatives:** Read `SYS.EXA_ALL_COLUMNS` and compare before the import. Rejected, because it adds a round trip per table and a second copy of the matching rule that can disagree with the one Exasol applies.
- **Rationale:** `exarrow-rs` joins the names verbatim into `IMPORT INTO <table> (...)` (`src/query/import.rs:402`), so unquoted names would break for `_id` and for every `<name>|<type>` column. Deriving the list from the batch that is being sent keeps one source of truth, so the list cannot drift from the CSV field order.
- **Consequences:** A second file that carries a column the table does not hold now fails instead of loading a value into the wrong column. That is a breaking change for any caller relying on the old behavior, and the old behavior wrote wrong data. Column order stops deciding correctness, so `ddl_column_order_matches_record_batch_field_order` is deleted. The opposite direction of the same mismatch had two options, reject the file or load NULL, and the requester chose the NULL load. A second file that omits a column the target table holds now succeeds and leaves that column NULL, where it failed with `ETL-6009` before. Rejection is stricter and symmetric with the extra-column rejection, and it breaks a repeated upload whose file legitimately omits an optional property. An absent JSON key normally means a NULL value, which is what the chosen behavior loads.
- **Promotes to ADR:** no

### [3] Reject colliding table names at plan time

- **Decision:** `plan_family` fails when two planned tables render the same name. The message names the table name and both `TablePath` values.
- **Alternatives:** Rely on decision [2] to fail the second load. Rejected, because the family is already created by then and the error does not name the cause. Rename one table automatically. Rejected, because it invents a name the user never wrote and cannot predict.
- **Rationale:** The check runs before any statement, so `--dry-run` reports it and a real run creates nothing. The collision comes from upstream naming, where the separator between two path segments and an underscore inside a JSON key both render as `_`, so exapump cannot avoid it without renaming.
- **Promotes to ADR:** no

### [4] A `|object` or `|array` column is not document data

- **Decision:** `reject_unusable_plans` treats `<name>|object` and `<name>|array` columns as generated, alongside `_id`, `_parent`, and `_pos`. A family is rejected when no planned table carries any other column.
- **Alternatives:** Match on the column-name suffix `|object` and `|array`. Rejected, because a JSON property literally named `x|object` would then be misread as generated.
- **Rationale:** The names come from `PropertyColumns::object_fk` and `PropertyColumns::array_count`, which is the contract's own record of which column is a link. `ColumnKind` does not distinguish them, because upstream builds both as `ColumnKind::Primary`.
- **Consequences:** `[{"a": {}}]` is rejected instead of creating two tables that hold only identifiers. The recorded scenario "Documents with no properties" keeps its behavior, and its stated reason widens to cover link columns.
- **Promotes to ADR:** no

### [5] Resolve the schema once and name it in every statement

- **Decision:** `load` resolves the target schema, then renders the DDL with that schema and qualifies every `IMPORT` statement with it. The `OPEN SCHEMA` statement is deleted. `describe` renders with the schema part of `--table` only, so an unqualified `--table` still previews unqualified names.
- **Alternatives:** Keep `OPEN SCHEMA` as an existence probe for the schema. Rejected, because it leaves two mechanisms selecting one schema, which is what comment 12 reported.
- **Rationale:** The DDL can only carry the resolved schema once the connection exists, so building it in `plan_family` forced the `OPEN SCHEMA` fallback. Moving the rendering to the two call sites removes the fallback.
- **Consequences:** The failure for a schema Exasol does not hold moves from the open to the first `CREATE TABLE`. The recorded scenario "Qualified table name whose schema does not exist" changes its stderr clause and keeps its exit code and its requirement that stderr names the schema. `exasol_json_missing_target_schema_fails` changes its assertion.
- **Promotes to ADR:** no

### [6] `load` takes an accumulator and returns `anyhow::Result<()>`

- **Decision:** `load(family, path, conn, loaded: &mut Vec<(String, u64)>) -> anyhow::Result<()>`. `src/commands/upload.rs` owns the vector, prints it on the success path and the failure path, and then propagates the error.
- **Alternatives:** Keep `(Vec<(String, u64)>, Option<anyhow::Error>)`. Rejected, because it repeats the same five-line return block five times.
- **Rationale:** The pair return existed to let the caller print what loaded before a failure. An out-parameter keeps that guarantee, keeps every stdout write in `src/commands/upload.rs`, and reduces each early return to `?`. This supersedes the decision recorded as "[open-questions] `load` could not return both the counts and the error" in `specs/_recorded/009-add-json-import/decision-log.md`, which chose the pair return for the same guarantee.
- **Promotes to ADR:** no

### [7] Accept the `.jsonl` extension

- **Decision:** `detect_from_path` maps `jsonl` to `FileFormat::Json`, and `SUPPORTED_FORMATS` lists it.
- **Alternatives:** Document `.jsonl` as unsupported. Rejected, because `.jsonl` is the common name for the format exapump already reads, and the change is one match arm.
- **Rationale:** Framing is detected from file content, not from the extension, so a third extension adds no framing logic. The scenario "Unsupported file extension" in `upload/parquet-import` lists the accepted extensions and changes with it.
- **Promotes to ADR:** no

### [8] Reject a single JSON object spread over several lines, with a message naming both accepted shapes

- **Decision:** exapump does not read a single multi-line JSON object. When the detected framing is `InputFormat::Lines` and the read fails, exapump adds context stating that the file is neither a JSON array of objects nor one JSON object per line. The underlying upstream error stays in the chain.
- **Alternatives:** Support the shape by re-reading the file as one JSON value when line 1 fails to parse. Rejected for two reasons. The shape yields a one-row table, so the value is low for a tool aimed at pipeline outputs. The fallback cannot distinguish a pretty-printed object from NDJSON whose first line is malformed, so it would report a misleading whole-file parse error for a genuinely malformed NDJSON file.
- **Rationale:** The reviewer offered the clear error as an acceptable outcome. The error is added only on line framing, because array framing already names the offending entry through `Expected top-level JSON array` and `Entry at index N is not an object`.
- **Consequences:** `docs/file_exchange.md` states the limit, so a user can convert the file before running exapump.
- **Promotes to ADR:** no

### [9] Record the in-memory buffering trade-off rather than adding streaming

- **Decision:** `upload/json-import` § Background states that exapump keeps every row in memory until the import starts, for both framings, and that NDJSON needs less memory than a JSON array only because exapump never parses the whole file into one value. `docs/file_exchange.md` carries the same note.
- **Alternatives:** Add chunking or spill-to-disk in this plan. Rejected, because it is a feature, not a review fix, and it does not belong on a PR under review.
- **Rationale:** The old sentence said NDJSON "streams the read pass", which reads as a memory bound it does not provide. The new text states the mechanism. It states no size threshold, because no measurement exists and an invented number would be worse than none. Measuring the peak-memory-to-file-size ratio stays a follow-up outside this plan.
- **Promotes to ADR:** no

### [10] State why the `_id` warning won

- **Decision:** `upload/json-import` § Background adds the reason: `json_tables_core` generates `_id` and exapump cannot renumber it, a refused repeated run would remove the append behavior the CSV and Parquet uploads offer, and a dropped and recreated family would delete rows an earlier run loaded.
- **Alternatives:** Leave the reason in the recorded decision log only. Rejected, because the next reader of the spec would have to find PR #48 to learn it.
- **Rationale:** The three alternatives and their costs are recorded in `specs/_recorded/009-add-json-import/decision-log.md`. The spec stated the outcome without them.
- **Promotes to ADR:** no

### [11] Keep the shared Background in `upload/json-import` only

- **Decision:** `upload/json-import-rejection` § Background and `upload/json-import-nesting` § Background drop the paragraphs duplicated from `upload/json-import` and point at it instead. Each keeps only the facts its own scenarios depend on.
- **Alternatives:** Fix `upload/json-import-rejection` only, as the comment asked. Rejected, because `upload/json-import-nesting` carries the same duplicated paragraph and the same false DDL-ownership sentence, and both Backgrounds must change for decision [1] regardless.
- **Rationale:** Three copies of one paragraph drift. A Background fact no scenario step depends on is also what `/speq:plan-review` flags as implementation leakage, so trimming serves both.
- **Promotes to ADR:** no

### [12] The schema drop guard lives in the shared test fixture

- **Decision:** `SchemaGuard` is added to `tests/fixtures/mod.rs`, `setup_exasol_schema` returns it, and the trailing `DROP SCHEMA ... CASCADE` block is deleted from all 26 tests across `tests/json_test.rs`, `tests/csv_test.rs`, `tests/parquet_test.rs`, and `tests/cli_test.rs`.
- **Alternatives:** Add the guard to `tests/json_test.rs` only, as the comment scoped it. Rejected, because the guard has to live in the shared fixture to be reachable, and fixing one of the helper's four callers leaves the identical leak in the other three.
- **Rationale:** `setup_exasol_schema` creates the schema, so it owns the cleanup. `SchemaGuard` implements `Display` and `Deref<Target = str>`, so the existing `format!("{schema}.orders")` and `&schema` call sites compile unchanged. `Drop` cannot await, so the guard runs the statement on a dedicated thread with its own current-thread runtime.
- **Consequences:** The cleanup runs while a failed assertion unwinds, which is the leak comment 15 reported. The drop path discards every error from the connection open, the `DROP SCHEMA` execution, and the thread join, because a panic inside `Drop` during an unwinding assertion aborts the test process and hides the original assertion. A leaked schema is therefore the failure mode of a broken cleanup, not a lost assertion. Group A must run before group B, because both edit `tests/json_test.rs`.
- **Promotes to ADR:** no

### [13] Only three tasks carry the `[expert]` tag

- **Decision:** Tasks 2.1, 2.2, and 2.5 carry `[expert]`. Task 2.3, the duplicate-table-name check, does not.
- **Alternatives:** Tag 2.3 as well, as the planning brief suggested. Rejected, because the check is a comparison over a family that is already in memory, with no ordering, concurrency, or cross-file dependency.
- **Rationale:** Tasks 2.1, 2.2, and 2.5 each carry non-obvious correctness: a wrong type map or column order writes wrong data silently, an unquoted identifier breaks every generated column name, and removing `OPEN SCHEMA` moves a failure a recorded scenario asserts. Every task of group B routes to the expert agent anyway, because one tag prices the whole group, so the tag choice changes cost nothing and keeps the tag honest.
- **Promotes to ADR:** no

### [14] Correct `specs/mission.md` § Tech Stack in the same plan

- **Decision:** Task 3.2 corrects the `json_tables_core` row, which states the crate "emits Exasol DDL".
- **Alternatives:** Leave mission.md to `/speq:mission` or a later `/speq:audit` run. Rejected, because this plan is what makes the row false.
- **Rationale:** The edit is one table cell. Leaving a knowingly false statement in the mission would fail the next audit for a reason this plan created.
- **Promotes to ADR:** no

## Review Findings

### [plan-review] Task 1.2 left four connection bindings unused

- **Finding:** `plan-reviewer` round 1 raised `[EFFORT_MISESTIMATION]` on plan.md task 1.2. In four tests of `tests/csv_test.rs` the deleted `DROP SCHEMA ... CASCADE` block is the connection's only use. Deleting it leaves `let (mut conn, schema_name)` with `conn` unread, which raises `unused_variables` and `unused_mut`. § Verification § Checklist requires `cargo clippy` to report 0 warnings, so the plan failed its own gate.
- **Direction change:** Task 1.2 now instructs the implementer to change the binding to `let (_conn, schema_name)` where the deleted block was the connection's only use. It names the four tests: `exasol_csv_import_prints_row_count`, `exasol_csv_import_with_custom_delimiter`, `exasol_csv_import_with_no_header`, and `exasol_csv_flags_ignored_for_parquet`. All four were confirmed in `tests/csv_test.rs` at lines 350, 382, 413, and 443. No design decision changed.
- **Promotes to ADR:** no

### [plan-review] The `SchemaGuard` drop path had no failure policy

- **Finding:** `plan-reviewer` round 1 raised `[UNSTATED_ASSUMPTION]` on plan.md task 1.1 and decision-log.md § [12]. Three steps of the drop path can fail: the connection open, the `DROP SCHEMA` execution, and the thread join. `setup_exasol_schema` unwraps every fallible step today (`tests/fixtures/mod.rs:113-118`), so an implementer copying that style panics inside `Drop`. A panic inside `Drop` during an unwinding assertion aborts the test process and hides the original assertion, which is the leak task 1.1 exists to close.
- **Direction change:** Task 1.1 now states that the drop path MUST discard every error instead of unwrapping it, across all three steps, and carries a CAUTION naming the consequence. It points at the existing `let _ = conn.execute_update(...)` cleanups as the policy to follow. § [12] Consequences states the same policy and names a leaked schema as the failure mode of a broken cleanup. Decision [12] itself is unchanged.
- **Promotes to ADR:** no

### [plan-review] The `.jsonl` scenario had no verification

- **Finding:** `plan-reviewer` round 1 raised `[COMPLETENESS_GAP]` on the `upload/parquet-import` delta and plan.md § Verification. The delta adds `.jsonl` to the accepted extensions of the "Unsupported file extension" scenario. Its mapped test `unsupported_file_extension` asserts that stderr contains `".parquet, .csv, .json, .ndjson"` (`tests/parquet_test.rs:63`), which stays a substring of the new list. The unit test `unsupported_extension_returns_error_with_supported_formats` carries the identical stale substring (`src/format.rs:53`). Both assertions were confirmed in the repository. Neither test could fail if `.jsonl` never reached `SUPPORTED_FORMATS`.
- **Direction change:** Task 2.7 now requires both assertions to state the full new list `.parquet, .csv, .json, .ndjson, .jsonl`, naming each test and its file. § Verification raises the count of tests that change their assertions from two to four and names the two added tests. The changed scenario now has a test that fails when the extension is dropped.
- **Promotes to ADR:** no

### [plan-review] No task corrected the doc comments in `src/json_tables.rs`

- **Finding:** `plan-reviewer` round 1 raised `[TRACEABILITY_GAP]` on plan.md § Implementation Tasks section 3 and § Dead Code Removal. Comment 10 reported that the DDL-ownership statement is false. Tasks 3.1 and 3.2 correct it in the ADR and in `specs/mission.md`, and the three spec deltas correct it in the Backgrounds. The same false sentence stayed in `src/json_tables.rs:3-4`. The doc comments of `qualified_name` and `create_tables` each explain themselves through `build_sql_schema`, which decision [1] removes. No task covered any of the three.
- **Direction change:** Task 3.4 corrects all three comments in `src/json_tables.rs`, with the replacement text fixed by decision [1]. It also drops the `OPEN SCHEMA` sentence from the `create_tables` comment, which task 2.5 makes false. Task 3.4 sits in group B, not group C, because it edits a file group B rewrites. § Parallelization now lists `2.1-2.8, 3.4` for group B, adds `tests/parquet_test.rs` to its Knowledge for task 2.7, and states why task 3.4 sits there.
- **Promotes to ADR:** no

### [plan-review] Task 3.4 covered three of seven stale doc comments

- **Finding:** `plan-reviewer` round 2 raised `[TRACEABILITY_GAP]` on plan.md task 3.4, which left four doc comments in `src/json_tables.rs` uncovered. Task 2.1 deletes the `ddl` field that the `TableFamily` comment names at lines 29-34. Task 2.6 replaces the return pair that the `load` comment describes at lines 228-237. Task 2.2 removes the by-position column mapping that the `build_record_batch` comment states at lines 313-318. Task 2.4 widens the rule that the `GENERATED_KEY_COLUMNS` comment states at lines 25-27. No build or test gate catches a stale comment.
- **Direction change:** Task 3.4 now covers seven doc comments instead of three. It names each comment, its line range, the task that makes it false or narrow, and its replacement content. All four new locations were re-confirmed in the source. `build_record_batch` iterates `physical_columns` at line 327, so the new comment keeps that field-order rule. The Fix line states the new count as six, and task 3.4 carried three comments before the finding named four more, so the count is seven. No design decision changed.
- **Promotes to ADR:** no

### [plan-review] The column-shape mismatch defined one direction only

- **Finding:** `plan-reviewer` round 2 raised `[COMPLETENESS_GAP]` on the `upload/json-import` Background, the `upload/json-import-rejection` scenarios, and plan.md § Impact paragraph 3. The artifacts defined the extra-column direction and left the missing-column direction undefined. Verified on the container at `localhost:8563`, uploading `[{"a":1,"b":"x"},{"a":2}]` and then `[{"a":3}]` into one table fails today with `ETL-6009: Number of columns in source (=2) and destination (=3) table differs`. After task 2.2 the statement becomes `IMPORT INTO <t> ("_id", "a")`, which Exasol accepts and leaves `b` at NULL. § Impact paragraph 3 therefore stated the opposite outcome for that direction.
- **Direction change:** The requester chose the NULL load over rejection. Decision [2] Consequences names both options and records the choice. The `upload/json-import` Background gains one sentence stating that a target-table column the batch does not name loads NULL. The new `upload/json-import` scenario "Second file omits a column the target table holds" asserts the NULL, the surviving earlier values, and exit code 0, and § Verification maps it to `exasol_json_second_file_omitting_a_column_loads_null` in `tests/json_test.rs`. Task 2.2 now states the behavior, forbids padding the column list and probing the catalog, and requires three failing tests instead of two. § Impact paragraph 3 states both directions. The scenario needs no nullability change, because `json_tables_core` sets `is_required` on `_id`, `_parent`, and `_pos` only (`crates/json_tables_core/src/infer.rs:251-402`), and `NOT NULL` follows `is_required || is_null_mask` (`crates/json_tables_core/src/ddl.rs:43`).
- **Promotes to ADR:** no
