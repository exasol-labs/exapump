# Plan Review Findings: add-json-import (round 1)

## Summary
- Axes checked: 6/6
- Total findings: 21 (Blockers: 6, Advisory: 15)
- Intent Fidelity blockers: 0

## Premortem

Three failure stories drove this review.

**Story 1. The first integration run creates a table nobody planned.** Every object table that `json_tables_core` plans carries `_id`, including the root of a flat document set. The spec says the root carries `_id` only when its children need a parent reference. The flat-import test asserts "one column per JSON property" and fails on the extra column. Routed to `[COMPLETENESS_GAP]`.

**Story 2. Group A lands, CI turns red, and nobody can run a test.** Adding `FileFormat::Json` makes the exhaustive match in `src/commands/upload.rs` non-exhaustive. That file belongs to Group B. Routed to `[HIDDEN_DEPENDENCY]`.

**Story 3. A repeated run loads values into the wrong columns.** The import is positional, and exapump rebuilds, in its own module, the physical-column filter that `json_tables_core::ddl` applies internally. An upstream change to either rule misaligns the two without an error. Routed to `[INFORMATION_LEAKAGE]`.

## Intent Fidelity

[no objection — axis checked: the user asked for a JSON-as-tables feature analogous to `exasol-labs/exasol-json-tables`, an investigation of code reuse, and spec coverage. plan.md § Decision reuses `json_tables_core` rather than reimplementing it, decision-log.md [1] and [2] record the reuse investigation and the crate boundary, and three spec deltas cover the feature. plan.md § Non-Goals excludes `json_tables_udf`, provenance comments, manifests, and `--schema-sql`, each with a rationale tied to the client-side CLI role. No substituted problem, no untraceable extra work, and no silent removal of the ask.]

## Feasibility

#### [UNSTATED_ASSUMPTION] BLOCKER
- Location: decision-log.md § [13]; plan.md § Consequences, row "Reject an empty plan and a table with no physical column"; plan.md task 2.3
- Issue: the plan states "an all-empty-object input yields a table with no columns and therefore invalid DDL". Upstream contradicts this. `json_tables_core/src/infer.rs:203-209` sets `include_id = true` unconditionally for `PathKind::Object`, so `[{},{}]` plans a root table holding `_id DECIMAL(18,0) NOT NULL`. The zero-column DDL string `CREATE TABLE "X" (\n\n);` is reachable only from a hand-built `PlannedTable`, never from `build_all_schema_plans`. Task 2.3's guard "reject any planned table with no physical column" therefore never fires for this input, the run exits 0, and the scenario "Documents with no properties" fails.
- Fix: in decision-log.md § [13] and plan.md § Consequences, replace the false premise with the upstream fact that every object table carries `_id`. Restate the rejection rule as "reject when no table is planned, and reject when every planned table carries only generated key columns (`_id`, `_parent`, `_pos`)". Update task 2.3 to that wording and update the spec scenario "Documents with no properties" so its THEN clause matches the restated rule.

#### [HIDDEN_DEPENDENCY] BLOCKER
- Location: plan.md § Parallelization; tasks 1.2, 1.3, 1.4
- Issue: `src/commands/upload.rs:15-20` matches `(format, args.dry_run)` exhaustively over `FileFormat` with four arms. Task 1.2 adds `FileFormat::Json` to `src/format.rs`. That makes the match non-exhaustive and the crate stops compiling with E0004. `src/commands/upload.rs` is listed under Group B. The claim "Group A touches no file Group B touches" is therefore wrong, and Group A's own tasks 1.3 and 1.4 assert on CLI behavior through `cargo test`, which cannot build.
- Fix: in plan.md § Implementation Tasks, add the two dispatch arms to Group A. Either move tasks 2.4 and 2.8 into section 1 with stub bodies that return an error, or add a task 1.2a "add `(FileFormat::Json, _)` arms to `src/commands/upload.rs` that return `anyhow::bail!` until section 2 replaces them". Update the § Parallelization table so Group A lists `src/commands/upload.rs` in its Knowledge column and the "touches no file Group B touches" sentence matches the new split.

#### [UNSTATED_ASSUMPTION] BLOCKER
- Location: plan.md § Decision, the `load` doc comment "Returns the row count loaded per table, in family order"; task 2.7; plan.md § Verification, the note on `partial_family_failure_reports_loaded_tables`
- Issue: the plan never defines "family order" and never names the accessor that yields it. `json_tables_core/src/buffer.rs:158-180` stores tables in a `HashMap<TablePath, TableBuffer>` and `ColumnBuffers::tables()` iterates that map, so its order is randomized per process. Upstream `json_tables_ingest/src/lib.rs:480-481` sorts explicitly, with a comment recording that the unstable order broke reproducibility. The test `partial_family_failure_reports_loaded_tables` requires the root table to import before `"SALES"."ORDERS_items_arr"`. Under `tables()` order that test is a coin flip, and the spec requirement "stdout MUST list the tables loaded before the failure" has no deterministic result.
- Fix: in plan.md § Decision, define family order as the order of the `build_all_schema_plans` output, which `StatsCollector::finish` sorts by table path. State in task 2.7 that `load` iterates `family.plans` and resolves each buffer through `ColumnBuffers::table(&plan.path)`, and that it never iterates `ColumnBuffers::tables()`. Add one sentence to the spec Background of `upload/json-import` stating that the command creates, loads, and reports tables in a deterministic order derived from the table path.

#### [NFR_IGNORED] ADVISORY
- Location: `upload/json-import/spec.md` § Background, paragraph starting "The whole family is buffered in memory"; decision-log.md § [12]
- Issue: the spec states "Peak memory therefore scales with the file size". The real peak holds four materializations at once. For array framing `json_tables_core/src/read.rs:60` parses the whole file into one `serde_json::Value`. `ColumnBuffers` then holds every row of every table. `ColumnBuffers` exposes no consuming accessor and derives no `Clone`, so the Arrow arrays must be copied out while the buffers stay alive. `exarrow_rs::import::arrow::import_from_record_batches` then concatenates every batch into one in-memory CSV `Vec<u8>` before sending. The stated ceiling understates the real one by roughly a factor of four.
- Fix: replace the sentence "Peak memory therefore scales with the file size" in the spec Background with a statement that the command holds the parsed document tree, the column buffers, the Arrow arrays, and the serialized CSV payload at the same time, so peak memory is several times the input size. Add the same fact to decision-log.md § [12] § Rationale.

#### [UNSTATED_ASSUMPTION] ADVISORY
- Location: plan.md § Dependencies table, rows `json_tables_core` and `serde_json`
- Issue: the table states that `json_tables_core`'s "only dependency is `serde_json`, already in exapump's tree" and calls `serde_json` a dependency "already transitive". `exapump/Cargo.toml` lists no `serde_json`, and `json_tables_core/Cargo.toml` at tag `v0.3` pins `serde_json = { version = "1.0.150", features = ["preserve_order"] }`. The `preserve_order` feature swaps `serde_json::Map` for an `indexmap`-backed map and adds `indexmap`, `hashbrown`, and `equivalent` to exapump's tree. Cargo feature unification applies that feature to every other consumer of `serde_json` in the build, including `arrow-json`, which exapump uses for `sql --format json`.
- Fix: in plan.md § Dependencies, correct the `serde_json` row to state the minimum version `1.0.150` and the `preserve_order` feature, and name `indexmap` as a new transitive dependency that `cargo deny check licenses` and `about.toml` must accept. Add one sentence stating that feature unification applies `preserve_order` to `arrow-json` as well, and add a check to task 1.1 that `cargo test` still passes for the `sql --format json` scenarios.

## Requirement Quality

#### [COMPLETENESS_GAP] BLOCKER
- Location: `upload/json-import/spec.md` § Background, sentence "A table whose children need a parent reference carries `_id`."; § Scenario "Import a flat JSON array into one table"
- Issue: the stated rule is false. `json_tables_core/src/infer.rs:203-209` sets `include_id = true` for every `PathKind::Object` table unconditionally, and gives an array table `_id` only when `has_nested_array` is true. Every root table therefore carries `_id DECIMAL(18,0) NOT NULL`, whether or not it has children. The flat-import scenario asserts that the command "MUST create exactly one table `"SALES"."ORDERS"`, with one column per JSON property". That assertion is false for every input, so `import_flat_json_array_creates_one_table` is written against a contract the implementation cannot meet.
- Fix: in the spec Background, replace the `_id` sentence with the real rule: every object table, including the root, carries `_id`, and an array element table carries `_parent` and `_pos` plus `_id` only when it holds a nested array. In the scenario "Import a flat JSON array into one table", change the THEN step to "MUST create exactly one table `"SALES"."ORDERS"`, carrying an `_id` column plus one column per JSON property typed per the contract table in Background".

#### [REQUIREMENT_CONFLICT] BLOCKER
- Location: `cli/upload-command-structure/spec.md` lines 11-18, against `specs/cli/upload-command-structure/spec.md`
- Issue: `/speq:spec-merge` defines `DELTA:CHANGED` as "Replace scenario with same name". The delta block is headed "### Scenario: CSV flags ignored for non-CSV files". The recorded library holds "### Scenario: CSV flags ignored for Parquet files". No scenario of the delta's name exists in the library, so the merge has nothing to replace. Recording appends a second scenario and leaves the stale Parquet-only one beside it, producing two scenarios that state overlapping rules for the same flag.
- Fix: in `specs/_plans/add-json-import/cli/upload-command-structure/spec.md`, restore the heading "### Scenario: CSV flags ignored for Parquet files" inside the `DELTA:CHANGED` block and change only its steps to cover Parquet, JSON, and NDJSON. Do not rename a scenario inside a `DELTA:CHANGED` block.

#### [REQUIREMENT_CONFLICT] BLOCKER
- Location: plan.md § Identifier rules, sentence "Import targets are qualified explicitly as `"<SCHEMA>"."<raw name>"`, so no import depends on session state"; decision-log.md § [6]; `upload/json-import/spec.md` § Scenario "Unqualified table name uses the connection schema"
- Issue: decision [6] rejects session-state resolution for imports, because "Relying on session state for the imports would add an unverified assumption about whether the HTTP-transport IMPORT inherits the session schema". When `--table` carries no schema part there is no `<SCHEMA>` to qualify with, so the unqualified scenario requires exactly the behavior decision [6] forbids. No artifact states what `load` does in that case. `OPEN SCHEMA` is skipped per the same paragraph, and the import target is left bare. The scenario has no defined rule to test against, and `unqualified_table_uses_connection_schema` cannot be written from the plan.
- Fix: in plan.md § Identifier rules and decision-log.md § [6], add the unqualified rule explicitly. Either read the schema from `exarrow_rs::Connection` connection parameters (`ConnectionParams::schema` is public) and qualify with it, or state that the unqualified case passes bare quoted names and accepts session-schema resolution. If the second option is chosen, record the session-schema dependency as a named assumption in the spec Background of `upload/json-import` and soften decision [6]'s claim that "no import depends on session state".

#### [COMPLETENESS_GAP] ADVISORY
- Location: `upload/json-import/spec.md` § Background, paragraph starting "Generated key columns link the family."; decision-log.md § [3]
- Issue: both list the linkage columns as `_id`, `_parent`, `_pos`, and `<name>|object`. `json_tables_core/src/infer.rs:334-349` also emits a `<name>|array` column on the parent table whenever an array property appears, typed `DECIMAL(18,0)` and holding the element count. Every nested-array family therefore carries a column the spec never names. A reader of the spec, and the assertions in `import_nested_json_creates_subtable_per_path`, would not expect it.
- Fix: add `<name>|array` to the Generated key columns paragraph of the spec Background, stating that it holds the element count of the array property on the parent row. Add the same column to the list in decision-log.md § [3] § Decision.

#### [COMPLETENESS_GAP] ADVISORY
- Location: `upload/json-import/spec.md` § Background, the column-typing table and the paragraph starting "A property whose values carry more than one scalar type"
- Issue: the stated rule does not hold for the integer and fractional pair. `json_tables_core/src/infer.rs:302-308` merges `Integer` and `Number` statistics into a single `Number` column with summed counts. A property holding both `1` and `1.5` therefore gets one `DOUBLE` column and no `|integer` sibling, contrary to the general rule as written. The Background also gives no tie-break rule when two types have equal counts, though upstream breaks ties by `SimpleType` declaration order.
- Fix: add one sentence to the mixed-type paragraph of the spec Background stating that integer and fractional values of the same property merge into a single `DOUBLE` column with no sibling, and that a count tie resolves in the fixed `json_tables_core` type order.

#### [COMPLETENESS_GAP] ADVISORY
- Location: `upload/json-import/spec.md` § Scenarios; plan.md task 2.1
- Issue: the spec covers an empty file, a zero-document file, an all-empty-object file, and a non-object array entry, but no malformed-JSON input. Malformed input is the most likely user error for this format, and `json_tables_core` returns three distinct messages for it: `"Line {n}: {serde error}"` for a bad NDJSON line (`read.rs:78-79`), `"invalid JSON: {err}"` for a bad array body (`error.rs:27`), and `"Expected top-level JSON array"` when array framing holds a non-array value (`read.rs:63`). None of the three has a scenario, and task 2.1 plans no fixture for them.
- Fix: add a scenario "Malformed JSON input" to `upload/json-import/spec.md` requiring a non-zero exit, a stderr message naming the parse failure, and a stderr message naming the failing line for NDJSON framing. Add the matching fixture to task 2.1, the failing test to task 2.2, and the row to plan.md § Verification § Scenario Coverage.

#### [COMPLETENESS_GAP] ADVISORY
- Location: `upload/json-import/spec.md` § Scenarios; plan.md task 2.1
- Issue: two derivation behaviors have no scenario and no fixture. First, an array of scalar values. `json_tables_core/src/infer.rs:294-298` renames the synthetic property `value` to `_value` in an array table, so `{"tags": ["a", "b"]}` produces a subtable whose data column is `_value`. Second, identifier collision. `contract.rs:259` `sanitize_ident` only doubles embedded quotes, and a document property named `_id`, `_parent`, or `_pos` collides with a generated key column of the same name. Neither the resulting column layout nor the collision outcome is specified.
- Fix: add a scenario "Array of scalar values" to `upload/json-import/spec.md` stating that the element table carries `_value` alongside `_parent` and `_pos`. Add a scenario "Property name collides with a generated key column" stating the required outcome, and add both fixtures to task 2.1 and both tests to tasks 2.2 or 2.5.

#### [AMBIGUOUS_REQUIREMENT] ADVISORY
- Location: `upload/json-import/spec.md` § Scenario "Document that is not a JSON object", step "stderr MUST name the position of the offending entry"
- Issue: "position" has no defined base, and `json_tables_core` uses two. Array framing reports `"Entry at index {idx} is not an object"` with a zero-based index (`read.rs:66-68`). Line framing reports `"Line {n} is not an object"` with a one-based line number (`read.rs:80-82`). The fixture is "a top-level JSON array whose third element is the number `42`", so the message names index 2, not position 3. `non_object_entry_errors_with_position` cannot assert a value without the base being stated.
- Fix: change the step to "stderr MUST name the zero-based index of the offending entry for array framing, and the one-based line number for NDJSON framing".

#### [REQUIREMENT_CONFLICT] ADVISORY
- Location: `specs/mission.md` § Out of Scope, § Architecture, § External Dependencies; plan.md task 1.5
- Issue: the mission states "Data transformations, filtering, or column mapping" as out of scope, describes the architecture as a "Thin CLI wrapper over exarrow-rs" where "exapump owns only CLI-specific concerns", and lists crates.io as the only build-time external dependency. This plan adds a normalization engine, an Arrow bridge inside exapump, and a git source at `github.com/exasol-labs/exasol-json-tables`. Task 1.5 updates only "the supported-format list, the core-capability list, and the `json_tables_core` entry in the Tech Stack table", so three mission sections stay contradicted after the change lands. plan.md § Consequences itself cites "exapump's mission defines the tool as a thin wrapper over exarrow-rs" as a rationale while widening that boundary.
- Fix: extend task 1.5 to also update `specs/mission.md` § Out of Scope, so the JSON-to-table-family normalization is named as an in-scope exception, § Architecture, so the second core library appears in the layer diagram, and § External Dependencies, so the GitHub git source gets a row with its failure impact.

## Task Breakdown

#### [TRACEABILITY_GAP] ADVISORY
- Location: plan.md task 1.5
- Issue: the task instructs "Update `README.md` and `specs/mission.md` for the new formats: the supported-format list, the core-capability list, and the `json_tables_core` entry in the Tech Stack table." `README.md` holds none of those three. It has 99 lines, no supported-format list, no core-capability list, and no Tech Stack table. Its only format statement is the command table at line 74, "Upload CSV or Parquet files to an Exasol table", which links to `docs/file_exchange.md`. That document mentions CSV and Parquet 27 times and no task touches it.
- Fix: rewrite task 1.5 to name real artifacts. List `README.md` line 74's command-table row, `docs/file_exchange.md`, and the `specs/mission.md` Core Capabilities and Tech Stack entries as the four places to update.

#### [TASK_GRANULARITY] ADVISORY
- Location: plan.md task 2.5
- Issue: one checkbox covers ten integration scenarios against a live Exasol container, including nested subtable creation, generated key columns, mixed scalar types, the explicit null mask, a repeated run, an unqualified target, and partial-family failure reporting. The unit of verification is not one test but ten, several of which need distinct schema setup and teardown. Task 2.2 by contrast splits seven connection-free tests into one task of similar size, so the two are not comparable units.
- Fix: split task 2.5 into two tasks in plan.md § Implementation Tasks. Put the six happy-path load tests in one, and put `repeated_run_appends_to_existing_family`, `unqualified_table_uses_connection_schema`, and `partial_family_failure_reports_loaded_tables` in a second, because those three need their own schema fixtures.

## Design Depth

#### [INFORMATION_LEAKAGE] ADVISORY
- Location: plan.md task 2.6; plan.md § Patterns, row "Arrow RecordBatch import"
- Issue: task 2.6 instructs exapump to "keep only the columns for which `column_sql_type` returns `Some`" and to match "the CREATE statement column order exactly". `json_tables_core/src/ddl.rs:41-50` applies that same filter internally when it builds the CREATE statements. Two crates therefore hold the same decision about which columns are physical and in what order, with nothing enforcing agreement. `exarrow_rs::import::arrow::ArrowImportOptions` defaults `columns` to `None`, so `import_from_record_batches` emits `IMPORT INTO <table>` with no column list and Exasol maps CSV fields positionally. A divergence between the two filters therefore loads values into the wrong columns without an error, and no scenario in this plan would catch it.
- Fix: in plan.md task 2.7, state that `load` builds an `ArrowImportOptions` whose `columns` holds the sanitized physical column names in Arrow field order, so the IMPORT names its columns instead of relying on position. Add a row to plan.md § Patterns recording that the column list is the enforcement point for agreement with `json_tables_core::ddl`.

#### [INFORMATION_LEAKAGE] ADVISORY
- Location: plan.md § Patterns, row "Single owner for the naming rule"
- Issue: the row claims "The mapping from `--table` to schema, stem, and qualified table names exists in exactly one place". `src/commands/mod.rs:10-16` already holds `parse_table_name`, which splits `--table` into an optional schema and a table name, and both `parquet_import` and `csv_import` call it. Adding a second splitter inside `src/json_tables.rs` puts the same decision in two modules. A change to how exapump reads `--table`, for example accepting a quoted schema, would then need two edits.
- Fix: state in plan.md § Identifier rules that `plan_family` calls `crate::commands::parse_table_name` for the split and owns only the uppercasing, quoting, and stem rules on top of it. Correct the § Patterns row to name that split ownership.

#### [TACTICAL_SHORTCUT] ADVISORY
- Location: decision-log.md § [7], final sentence "Applying constraints only to newly created tables stays a follow-up"; decision-log.md § [1], final sentence "Getting `json_tables_core` published to crates.io stays a worthwhile follow-up outside this plan"; decision-log.md § [14]
- Issue: three decisions name a follow-up and none schedules one. No task, no ADR entry, and no tracked item records them. Decision [14] additionally leaves an untested limit open, stating that whether "a wide family of `VARCHAR(2000000)` columns hits an Exasol table limit is not established here". After recording, those three commitments exist only in an archived decision log.
- Fix: add a `## Follow-Ups` section to plan.md listing the three items, each with the artifact that will carry it after `/speq:record`. For each, name whether it becomes a GitHub issue, a mission constraint, or a known-limit line in the recorded `upload/json-import` spec.

## Prose Quality

#### [PROSE_BLOAT] ADVISORY
- Location: decision-log.md § [1] § Rationale; the sentence "A direct crates.io lookup from this planning session returned nothing because the sandbox has no network, so task 1.1 confirms the git dependency resolves before anything else is built."
- Issue: the paragraph runs nine sentences against the six-sentence cap in `/speq:writing-guardrails`. The quoted sentence narrates the planning process rather than stating the decision as fact, which the same guardrail forbids. Two further sentences in the paragraph restate rationale already given in plan.md § Consequences.
- Fix: cut decision-log.md § [1] § Rationale to at most six sentences. Delete the crates.io lookup narration and keep only the resulting instruction, which task 1.1 already carries. Delete the sentences duplicated from plan.md § Consequences.

#### [PROSE_BLOAT] ADVISORY
- Location: plan.md § Summary, first sentence; `upload/json-import/spec.md` § Background, the paragraph starting "`--table <name>` names the root table"
- Issue: several descriptive sentences exceed the 25-word cap in `/speq:writing-guardrails`. plan.md § Summary opens with a 30-word sentence joined by a participle clause. The spec Background sentence "exapump uppercases the schema part and the table part of `--table`, then quotes both, so one `--table` value names the same root table for JSON, CSV, and Parquet input" runs 30 words and states three ideas.
- Fix: split the plan.md § Summary opening sentence into two, one naming the accepted formats and one naming the resulting table family. Split the quoted spec Background sentence into one sentence stating the uppercase-and-quote rule and one stating the cross-format consequence.
