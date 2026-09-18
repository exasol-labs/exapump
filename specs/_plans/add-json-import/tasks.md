# Tasks: add-json-import

## PR Lifecycle
- [x] resolved
- [x] implemented
- [x] version-bumped
- [ ] tested-green
- [ ] recorded
- [ ] pr-ready

## Phase 2: Implementation (Group A: Format surface and dependency)
- [x] 1.1 Add the `json_tables_core` git dependency pinned to tag `v0.3` and the `serde_json` dependency to `Cargo.toml`. Run `cargo deny check licenses` and `cargo deny check advisories`. Add a `[sources]` allow entry to `deny.toml` only if the git source is rejected.
- [x] 1.2 Add `FileFormat::Json` to `src/format.rs`, map the `.json` and `.ndjson` extensions to it, and update `SUPPORTED_FORMATS` to `.parquet, .csv, .json, .ndjson`. Update the `src/format.rs` unit tests: the unsupported-extension test must use `.txt`, and new tests must cover both JSON extensions in lower and upper case.
- [x] 1.2a Add the `(FileFormat::Json, true)` and `(FileFormat::Json, false)` arms to the `(format, args.dry_run)` match in `src/commands/upload.rs`. Both arms `anyhow::bail!` with a not-yet-implemented message.
- [x] 1.3 Update `tests/parquet_test.rs::unsupported_file_extension` to use a `.txt` file and assert the new supported-format list.
- [x] 1.4 Extend the `UploadArgs::table` doc comment in `src/cli.rs` to state that JSON input creates a root table plus one subtable per nested path. Add the `tests/cli_test.rs` assertion for it.
- [x] 1.5 Update `README.md` and `specs/mission.md` for the new formats: the supported-format list, the core-capability list, and the `json_tables_core` entry in the Tech Stack table.

## Phase 2: Implementation (Group B: JSON table family) [expert]
- [x] 2.1 Add `tests/fixtures/mod.rs` helpers that create the JSON inputs the scenarios need: a flat array file, a nested array-and-object file, an NDJSON file, a `.json` file holding NDJSON framing, a mixed-scalar-type file, an explicit-null file, an empty file, an empty-array file, an all-empty-objects file, and a file whose third array entry is a number.
- [x] 2.2 Write the failing `tests/json_test.rs` tests for the dry-run, file-not-found, empty-file, no-documents, no-properties, non-object-entry, and connection-failure scenarios, plus a test that `--delimiter` is accepted and ignored on a JSON dry run.
- [x] 2.3 Implement `plan_family` and `TableFamily::describe` in the new `src/json_tables.rs`. Apply the identifier rules from the plan's Design section and rewrite each `CREATE TABLE` to `CREATE TABLE IF NOT EXISTS`. Discard the constraint statements. Reject an empty plan and a family of only generated key columns before any statement is built.
- [x] 2.4 Replace the task 1.2a `(FileFormat::Json, true)` bail arm in `src/commands/upload.rs` with the dry-run dispatch into `json_tables`, and declare the `json_tables` module in `src/main.rs`.
- [x] 2.5 Write the failing `tests/json_test.rs` tests for the load scenarios against the Exasol Docker container: flat import, nested subtable creation, generated key columns, NDJSON, content-detected framing, mixed scalar types, explicit null mask, repeated run, unqualified table name, unqualified table name with no connection schema, and partial-family failure reporting.
- [x] 2.6 Implement the `ColumnBuffers` to `arrow::RecordBatch` bridge in `src/json_tables.rs`. [expert]
- [x] 2.7 Implement `load` in `src/json_tables.rs`, per the plan's Design section identifier-resolution and family-order rules. [expert]
- [x] 2.8 Replace the task 1.2a `(FileFormat::Json, false)` bail arm in `src/commands/upload.rs` with the import dispatch, printing per-table and total row counts and the stderr `_id` warning.

## Phase 4: Review Fixes
- [x] 4.1 Run `cargo update -p rustls` to lift `rustls` to 0.23.45 inside the declared `0.23` range, clearing GHSA-2mjx-qc3c-rqvc. Re-run the full check set and, if the aws-lc-sys bump changes its license metadata, add the new license to both `deny.toml` `[licenses].allow` and `about.toml` `accepted`.
- [x] 4.2 In `src/json_tables.rs`, change `read_documents`' closure bound to `anyhow::Result<()>` and carry the visitor's error out of band, so pass-2 buffering failures are no longer reported as `failed to read {path}`. Update the `plan_family` and `collect_rows` call sites, the latter with its own `failed to buffer the documents of {path}` context.
- [x] 4.3 Add `tests/json_test.rs::exasol_json_missing_target_schema_fails` covering the `OPEN SCHEMA` failure for a non-existent target schema, and a `src/json_tables.rs` unit test `build_ddl_rejects_an_unrecognised_create_statement` covering the `strip_prefix` bail.
- [x] 4.4 In `src/json_tables.rs` `build_record_batch`, replace the `v.clone()` calls in the `Bool`, `Int` and `Double` arms with `from_iter(v.iter().copied())`, leaving the `BoolMask` arm unchanged.
- [x] 4.5 In `src/json_tables.rs`, inline `TableFamily::qualified_name` into its single caller in `describe` and delete the method.
- [x] 4.6 Add the `tests/fixtures/mod.rs` helper `create_all_empty_arrays_json` and the `tests/json_test.rs` test `exasol_json_array_empty_in_every_document_creates_an_empty_subtable`, pinning that an array empty in every document plans and creates a key-only subtable, loads zero rows into it, and leaves the family accepted.
- [x] 4.7 Add the `src/json_tables.rs` unit test `ddl_column_order_matches_record_batch_field_order`, pinning that the CREATE body's column order equals the `RecordBatch` field order for every table of a family covering `Primary`, `Alternate` and `NullBitmask` columns. [expert]

## Phase 5: Verification
- [x] 5.1 Run `cargo build`
- [x] 5.2 Run `cargo test` (Exasol container required)
- [x] 5.3 Run `cargo clippy --all-targets --all-features -- -D warnings`
- [x] 5.4 Run `cargo fmt --check`
- [x] 5.5 Run `cargo deny check licenses` and `cargo deny check advisories`
- [x] 5.6 Scenario coverage audit against plan.md § Verification § Scenario Coverage
- [x] 5.7 Manual verification per plan.md § Verification § Manual Testing
