# Decision Log: add-json-import

## Interview

This plan ran in headless mode. No live interview took place. The orchestrator carried out the investigation the user asked for and passed the result as the planning brief. The entries below record the user intent and the brief's findings in the interview position.

**Q:** What should exapump gain?
**A:** A feature that imports JSON as tables, analogous to `https://github.com/exasol-labs/exasol-json-tables`, covered by specs.

**Q:** Can existing Rust code from `exasol-json-tables` be reused rather than reimplemented?
**A:** Yes, in part. The repository is a Cargo workspace of three crates. `json_tables_core` (v0.3.0) is a pure normalization engine whose only dependency is `serde_json`, with no Exasol, Arrow, or CLI coupling. `json_tables_ingest` (package `json_to_parquet`) is the local-file CLI that stages Parquet and imports over `exarrow-rs 0.16.0`, the same version exapump already uses. `json_tables_udf` is an in-database Exasol UDF.

**Q:** Is the code available as a crates.io dependency?
**A:** No. Neither `json_tables_core` nor `json_to_parquet` is published. No Cargo.toml sets `publish`, and no workflow publishes to crates.io. Reuse needs a git dependency, an upstream crates.io release, or vendoring.

**Q:** Does exapump's current upload model fit?
**A:** No. `src/commands/upload.rs` infers one schema, emits one `CREATE TABLE IF NOT EXISTS`, and runs one import. JSON tables is multi-table by construction.

## Design Decisions

### [1] Reuse `json_tables_core` through a git dependency pinned to tag `v0.3`

- **Decision:** Add `json_tables_core = { git = "https://github.com/exasol-labs/exasol-json-tables", tag = "v0.3" }` to exapump's `Cargo.toml`.
- **Alternatives:** Vendor the normalization logic into exapump. Block this plan until the upstream owners publish `json_tables_core` to crates.io.
- **Rationale:** A git dependency blocks no exapump distribution path. `.github/workflows/auto-tag.yml` builds release binaries and uploads them to a GitHub release, and no workflow runs `cargo publish`, so the crates.io restriction on git dependencies does not apply. The evidence that `json_tables_core` is unpublished is the orchestrator's crates.io lookup plus the absence of any `publish` field and any publish workflow in the upstream repository. A direct crates.io lookup from this planning session returned nothing because the sandbox has no network, so task 1.1 confirms the git dependency resolves before anything else is built. Vendoring forks the exact logic the user asked to reuse and guarantees drift. A crates.io release belongs to another repository's owners and would make this plan wait on an external release. The repository is public, so CI fetches it anonymously and no confidentiality rule applies. Getting `json_tables_core` published to crates.io stays a worthwhile follow-up outside this plan.
- **Promotes to ADR:** yes

### [2] Reuse `json_tables_core` only, not `json_to_parquet`

- **Decision:** Depend on the core crate. Reimplement the I/O half inside exapump.
- **Alternatives:** Add `json_to_parquet` as a second dependency and call its `run(Args)`.
- **Rationale:** `json_to_parquet` exposes one public function whose `Args` fields are private, so a caller cannot construct them. It opens its own connection with `Driver::open(url)` from a raw URL, which bypasses exapump's DSN, profile, certificate-fingerprint, and transport resolution in `src/connection.rs`. It prints its own progress format. Consuming it would put two connection layers and two output conventions in one binary. Its own crate documentation states that anything an alternative loader also needs belongs in the core crate, which is the boundary this decision follows.
- **Promotes to ADR:** yes

### [3] Full multi-table fan-out in the first release

- **Decision:** The first release creates the root table and every subtable, with the `_id`, `_parent`, `_pos`, and `<name>|object` linkage columns.
- **Alternatives:** Scope the first release to flat, non-nested JSON, one table, and defer nesting.
- **Rationale:** The user asked for a feature analogous to `exasol-json-tables`. The table family is that tool's whole capability. A flat-only release would need almost none of `json_tables_core` and would not answer the request. The two-pass core API makes the fan-out no harder than the flat case, because the same `build_all_schema_plans` call returns one plan or many.
- **Promotes to ADR:** no

### [4] Import through `import_from_record_batches` instead of staging Parquet

- **Decision:** Convert each `TableBuffer` to an `arrow::RecordBatch` and call `exarrow_rs::Connection::import_from_record_batches`.
- **Alternatives:** Follow upstream and write one Parquet file per table into a temporary directory, then call `import_parquet_from_files`.
- **Rationale:** `exarrow_rs 0.16.0` provides `import_from_record_batches` at `src/adbc/connection.rs:1697`. Using it removes the temporary directory, the low-level Parquet writer, and the cleanup logic that upstream needs. exapump's mission defines the tool as a thin wrapper over exarrow-rs that adds negligible overhead, and exapump already writes Arrow data through `parquet::arrow::ArrowWriter` in `src/commands/export.rs`, so Arrow is the established currency in this codebase.
- **Promotes to ADR:** no

### [5] Uppercase and quote the `--table` schema and root-table parts

- **Decision:** `plan_family` uppercases the schema part and the table part of `--table`, uses the uppercased table part as the `json_tables_core` stem, and qualifies every import target as `"<SCHEMA>"."<raw name>"`.
- **Alternatives:** Pass `--table` through verbatim, so `--table sales.orders` creates a lowercase `"sales"."orders"`. Emit the root table unquoted and only quote the subtables.
- **Rationale:** `json_tables_core` quotes every identifier, because JSON keys are case sensitive and may contain characters an unquoted Exasol identifier rejects. exapump's CSV and Parquet paths pass unquoted names, and `exarrow_rs::types::InferredTableSchema::to_ddl` documents that it deliberately leaves them unquoted so Exasol folds them to uppercase. Uppercasing before quoting reproduces that folding, so one `--table` value names the same root table in all three formats. Mixed quoting across one family would be harder to explain than one uniform rule.
- **Promotes to ADR:** no

### [6] `OPEN SCHEMA` for DDL only, explicit qualification for imports

- **Decision:** `load` resolves one target schema first: the uppercased `--table` prefix when present, otherwise the connection's default schema from `exarrow_rs::Connection::params().schema`, and an error when neither supplies one. It then runs `OPEN SCHEMA "<SCHEMA>"` before the CREATE statements and qualifies every import target explicitly.
- **Alternatives:** Rewrite the table names inside the strings that `build_sql_schema` returns. Rebuild the DDL inside exapump from `PlannedTable` and `column_sql_type`. Rely on `OPEN SCHEMA` for the imports as well. Pass bare quoted names for an unqualified `--table` and let the session schema resolve them.
- **Rationale:** `build_sql_schema` emits unqualified quoted names, and its signature takes only a stem, so no argument makes it emit a qualified name. String surgery on another crate's output is brittle. Rebuilding the DDL inside exapump would place the plan-to-DDL decision in two crates at once, which is the information leakage this project's design rules name as the defect class to avoid. Relying on session state for the imports would add an unverified assumption about whether the HTTP-transport IMPORT inherits the session schema, so the imports name their schema explicitly instead. Reading the default schema from the connection parameters keeps that property for an unqualified `--table` too: exapump knows the schema name before it sends a statement, so it can qualify the imports rather than trust the session. `ConnectionParams::schema` is a public field and `Connection::params()` a public accessor, so this needs no upstream change.
- **Consequence:** `--dry-run` opens no connection, so an unqualified `--table` previews unqualified table names.
- **Promotes to ADR:** no

### [7] `CREATE TABLE IF NOT EXISTS` and no constraint statements

- **Decision:** Rewrite each CREATE statement to `CREATE TABLE IF NOT EXISTS` and discard the constraint statements that `build_sql_schema` returns.
- **Alternatives:** Execute the constraint statements and treat a failure as a warning. Probe the catalog for pre-existing tables and apply constraints only to new ones. Execute the constraints and let a repeated run fail.
- **Rationale:** `build_sql_schema` emits plain `CREATE TABLE` plus `ALTER TABLE ... ADD CONSTRAINT`, so a second run against the same target fails on both. exapump's CSV and Parquet uploads are re-runnable, and `src/commands/upload.rs` already applies the same `CREATE TABLE IF NOT EXISTS` rewrite. Upstream emits the constraints `DISABLE`, so they enforce nothing and serve as relationship metadata only. The `_id`, `_parent`, `_pos`, and `<name>|object` columns still carry the relationship. Swallowing errors and parsing error strings are both worse than leaving the metadata out. Applying constraints only to newly created tables stays a follow-up.
- **Promotes to ADR:** no

### [8] The family import is not atomic, and reports what loaded

- **Decision:** State in the spec that a failure partway through leaves the loaded tables in place. On failure, print the tables loaded so far and name the failed table on stderr.
- **Alternatives:** Wrap the whole family in one transaction and commit at the end. Delete the loaded tables on failure.
- **Rationale:** Whether the HTTP-transport IMPORT that `import_from_record_batches` issues participates in a surrounding transaction is unverified for `exarrow_rs 0.16.0`. Claiming atomicity on an unverified mechanism is worse than reporting the real state. Deleting tables on failure would destroy data the operator may already have had in a pre-existing table. Explicit reporting lets the operator clean up with full information.
- **Promotes to ADR:** no

### [9] Accept `.json` and `.ndjson`, not `.jsonl`

- **Decision:** `src/format.rs` maps `.json` and `.ndjson` to `FileFormat::Json`.
- **Alternatives:** Also map `.jsonl`. Add a flag that selects the framing.
- **Rationale:** `json_tables_core::read::detect_format` decides the framing from the first non-whitespace byte, so the extension never selects it. Both extensions therefore accept both framings, and a third spelling adds no capability. A framing flag would be a decision the module declined to make. Add `.jsonl` when a user asks.
- **Promotes to ADR:** no

### [10] Exclude provenance comments, manifests, and schema-SQL artifacts

- **Decision:** The first release emits no `COMMENT ON TABLE` provenance statements, no source manifest, and no `.sql` schema file.
- **Alternatives:** Port upstream's `--schema-sql`, `--manifest-output`, and provenance stamping.
- **Rationale:** The essential analog to `exasol-json-tables` is the table family. The artifacts are additive concerns with their own flags, output paths, and failure modes. `--dry-run` already shows the planned DDL, which is what the schema-SQL artifact carries. Each excluded artifact stays available as a later feature.
- **Promotes to ADR:** no

### [11] One file per invocation, single connection, sequential imports

- **Decision:** JSON import reads `args.files[0]` and imports the family over one connection, table by table.
- **Alternatives:** Expand all of `args.files`. Import tables in parallel over a bounded connection pool, as upstream does.
- **Rationale:** `src/commands/upload.rs` reads `args.files[0]` for CSV and Parquet today, despite `UploadArgs::files` being a `Vec`. Making JSON the one multi-file format would be inconsistent, and fixing it for all three formats is a separate change. Sequential import keeps ordering deterministic, keeps the failure report exact, and matches the existing single-connection import model. A table family is usually small enough that the parallelism upstream needs for large Parquet sets does not apply.
- **Promotes to ADR:** no

### [12] Buffer the whole family in memory

- **Decision:** Use `json_tables_core::buffer::ColumnBuffers` as-is and import each table only after the whole family is buffered.
- **Alternatives:** Implement a streaming `RowSink` in exapump that flushes fixed-size chunks to Exasol.
- **Rationale:** `ColumnBuffers` buffers by design, and `for_each_document` parses a top-level JSON array as one value, so array framing is already bounded by memory regardless of the sink. A streaming sink is possible, because `RowSink` exists for exactly that, but it needs a chunked import loop, a memory budget, and a rule for what a partial chunk means when a later chunk fails. Those are a separate feature. The spec Background states the memory ceiling and names NDJSON as the shape for large input.
- **Promotes to ADR:** no

### [13] Reject an empty plan before building any statement

- **Decision:** `plan_family` fails when the plan contains no table, and when every planned table carries only the generated key columns `_id`, `_parent`, and `_pos`.
- **Alternatives:** Let Exasol reject the generated SQL. Accept the key-only table and load it.
- **Rationale:** A zero-document input produces no `TableStats`, so `build_all_schema_plans` returns an empty vector and the run would silently create and load nothing. `json_tables_core/src/infer.rs:203-209` sets `include_id = true` for every `PathKind::Object` table unconditionally, so an all-empty-object input plans a root table holding `_id DECIMAL(18,0) NOT NULL`. That DDL is valid and Exasol accepts it, so no database-side error catches this input and the run would create a table holding no document data. The rejection rule therefore tests the column set rather than the column count. `upload/csv-import` already requires a non-zero exit and a clear message for an empty CSV, so this matches the existing contract.
- **Promotes to ADR:** no

### [14] Known constraint inherited from `json_tables_core`

- **Decision:** Accept `json_tables_core`'s column type mapping unchanged, including `VARCHAR(2000000)` for every string column.
- **Alternatives:** Narrow string columns by observed maximum length.
- **Rationale:** The mapping is the core crate's contract, and overriding it in exapump would duplicate a decision that belongs upstream. Whether a wide family of `VARCHAR(2000000)` columns hits an Exasol table limit is not established here and is not tested by this plan. Record it as a limit to watch rather than a claim in either direction.
- **Promotes to ADR:** no

## Review Findings

### [plan-review] Every object table carries `_id`, so the empty-plan rule was wrong

- **Finding:** `[UNSTATED_ASSUMPTION]` BLOCKER. Decision [13] and plan.md § Consequences claimed an all-empty-object input yields a table with no columns and therefore invalid DDL. `json_tables_core/src/infer.rs:203-209` sets `include_id = true` unconditionally for every `PathKind::Object` table, so `[{},{}]` plans a root table holding `_id DECIMAL(18,0) NOT NULL`. That DDL is valid. The guard "reject any planned table with no physical column" would never fire, the run would exit 0, and the scenario "Documents with no properties" would fail.
- **Direction change:** The rejection rule now tests the column set, not the column count. `plan_family` rejects when no table is planned, and rejects when every planned table carries only the generated key columns `_id`, `_parent`, and `_pos`. Decision [13], the plan.md § Consequences row, task 2.3, and the spec scenario "Documents with no properties" all carry the restated rule and the upstream fact behind it.
- **Promotes to ADR:** no

### [plan-review] Adding `FileFormat::Json` breaks the exhaustive match in Group B's file

- **Finding:** `[HIDDEN_DEPENDENCY]` BLOCKER. `src/commands/upload.rs:15-20` matches `(format, args.dry_run)` exhaustively. Task 1.2 adds `FileFormat::Json`, which makes that match non-exhaustive and stops the crate compiling with E0004. `src/commands/upload.rs` sat in Group B, so Group A's own `cargo test` assertions in tasks 1.3 and 1.4 could not have built, and the claim "Group A touches no file Group B touches" was false.
- **Direction change:** Added task 1.2a, which adds both `FileFormat::Json` arms as `anyhow::bail!` stubs inside Group A so the crate keeps compiling. Tasks 2.4 and 2.8 now read as replacing those stubs rather than adding arms. Group A's Knowledge column lists `src/commands/upload.rs`, and the false disjointness claim is replaced by the real reason the two groups do not contend: Group A must finish before Group B starts, so the shared file is sequenced, not parallel.
- **Promotes to ADR:** no

### [plan-review] "Family order" was undefined and the only accessor for it is unordered

- **Finding:** `[UNSTATED_ASSUMPTION]` BLOCKER. `load`'s contract promised counts "in family order" without defining the term or naming the accessor. `json_tables_core/src/buffer.rs:158-180` stores tables in a `HashMap<TablePath, TableBuffer>`, so `ColumnBuffers::tables()` yields a per-process random order. `partial_family_failure_reports_loaded_tables` would have been a coin flip and the requirement "stdout MUST list the tables loaded before the failure" had no deterministic result.
- **Direction change:** Family order is now defined in plan.md § Decision as the `build_all_schema_plans` output order, which `StatsCollector::finish` sorts by table path. Task 2.7 states that `load` walks `family.plans` for the CREATE statements, the imports, and the counts, resolves each buffer through `ColumnBuffers::table(&plan.path)`, and never calls `ColumnBuffers::tables()`. The spec Background states the deterministic order as a user-visible property. Fixing the order exposed a second error: `TablePath::to_string()` renders the root as the literal `root`, so the root sorts among the subtables and generally last. The § Verification note on `partial_family_failure_reports_loaded_tables` sabotaged the subtable, which under the real order is the first table imported and would leave nothing loaded before the failure. That note now sabotages the root table instead.
- **Promotes to ADR:** no

### [plan-review] The flat-import scenario asserted a table layout the crate cannot produce

- **Finding:** `[COMPLETENESS_GAP]` BLOCKER. The spec Background stated "A table whose children need a parent reference carries `_id`". Upstream gives every `PathKind::Object` table `_id` unconditionally, and gives an array table `_id` only when `has_nested_array` is true. The flat-import scenario asserted one table "with one column per JSON property", which is false for every input, so `import_flat_json_array_creates_one_table` was written against an unmeetable contract.
- **Direction change:** The spec Background now states the real rule: every object table carries `_id` including a flat root, and an array element table carries `_parent` and `_pos` plus `_id` only when it holds a nested array. The flat-import scenario's THEN step now expects an `_id` column alongside the property columns.
- **Promotes to ADR:** no

### [plan-review] A renamed scenario inside a `DELTA:CHANGED` block would not merge

- **Finding:** `[REQUIREMENT_CONFLICT]` BLOCKER. The delta block was headed "CSV flags ignored for non-CSV files" while the recorded library holds "CSV flags ignored for Parquet files". `DELTA:CHANGED` replaces the scenario of the same name, so the merge would have found nothing to replace, appended a second scenario, and left two overlapping rules for the same flag in the library.
- **Direction change:** Restored the library heading "CSV flags ignored for Parquet files" inside the `DELTA:CHANGED` block and changed only its steps, which now cover Parquet, JSON, and NDJSON. Updated the two plan.md references to the old name, in § Parallelization and § Verification. A `DELTA:CHANGED` block does not rename a scenario.
- **Promotes to ADR:** no

### [plan-review] An unqualified `--table` had no defined schema resolution

- **Finding:** `[REQUIREMENT_CONFLICT]` BLOCKER. Decision [6] rejected session-state resolution and plan.md claimed "no import depends on session state", while the scenario "Unqualified table name uses the connection schema" required exactly that. With no schema part there was no `<SCHEMA>` to qualify with, `OPEN SCHEMA` was skipped, and no artifact said what `load` does. `unqualified_table_uses_connection_schema` could not be written from the plan.
- **Direction change:** `load` now resolves one target schema before any statement runs: the uppercased `--table` prefix when present, otherwise the connection's default schema read from `exarrow_rs::Connection::params().schema`, and an error naming both sources when neither supplies one. Because exapump learns the schema name before it sends a statement, the imports still name it explicitly and decision [6]'s no-session-state property holds unchanged. `OPEN SCHEMA` now always runs rather than being skipped for an unqualified target. Recorded in plan.md § Identifier rules, decision [6], task 2.7, and the spec Background. The resolution failure needed a rule of its own, so the spec gains the scenario "Unqualified table name with no connection schema", with its test in task 2.5 and a row in § Verification. `--dry-run` opens no connection, so an unqualified `--table` previews unqualified names; that consequence is recorded in decision [6] and the spec Background.
- **Promotes to ADR:** no
