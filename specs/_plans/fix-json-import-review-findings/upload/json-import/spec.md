# Feature: JSON Import

<!-- DELTA:CHANGED -->
## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn`, `EXAPUMP_DSN`, or `--profile`. File format is detected from the file extension (`.json`, `.ndjson`, or `.jsonl`). The upload command is async.

The `json_tables_core` crate from `exasol-labs/exasol-json-tables` normalizes the document set. `json_tables_core` decides which tables the family holds and which columns each table carries. exapump owns file reading, framing, the SQL it runs, connection handling, the Arrow conversion, and the import.

Framing is detected from the first non-whitespace byte of the file, not from the extension. A leading `[` selects a single top-level JSON array of objects. Any other byte selects NDJSON, one JSON object per line. All three extensions accept both framings. A single JSON object spread over several lines matches neither framing. The sibling feature `upload/json-import-rejection` covers its rejection.

The document set is read twice. The first pass collects property and type statistics and derives the table family. The second pass writes rows into in-memory column buffers.

`--table <name>` names the root table and supplies the base name for every subtable. exapump uppercases the schema part and the table part of `--table`, then quotes both, so one `--table` value names the same root table for JSON, CSV, and Parquet input. Subtable naming for nested paths, and the array fan-out that produces them, is covered by the sibling feature `upload/json-import-nesting`.

Every table of a family is created and loaded in one resolved schema. When `--table` carries a schema part, that part supplies it. When `--table` carries no schema part, the connection's default schema supplies it. When neither supplies one, the command fails before creating any table. exapump resolves the schema once, before the first statement, and names it in every `CREATE TABLE` statement and every `IMPORT` statement. Under `--dry-run` there is no connection, so an unqualified `--table` previews unqualified table names.

exapump renders the `CREATE TABLE` statements from the column plan and maps each contract type to an Exasol type:

| JSON value | Exasol column type |
|------------|--------------------|
| boolean | `BOOLEAN` |
| integer | `DECIMAL(19,0)` |
| fractional number | `DOUBLE` |
| string | `VARCHAR(2000000)` |

An integer column holds a 64-bit signed integer, so `DECIMAL(19,0)` covers every value `json_tables_core` classifies as an integer. A number outside the 64-bit signed range is classified as a fractional number and lands in a `DOUBLE` column.

A property whose values carry more than one scalar type gets a primary column for the majority type and one `<name>|<type>` sibling column per remaining type. The type token is the `json_tables_core` type label, for example `integer` or `string`. A property that appears as an explicit JSON `null` in at least one document gets a `<name>|n` boolean mask column, so an explicit null stays distinguishable from an absent field.

The command creates, loads, and reports the tables of a family in a deterministic order derived from the table path, so two runs over the same input report the same table order.

exapump imports rows by column name. Each `IMPORT` statement names the columns of the batch it sends, so a value lands in the column of its own name whatever order the file lists the properties in. A batch column that the target table does not hold fails the import. The sibling feature `upload/json-import-rejection` covers that failure. A target-table column the batch does not name loads NULL, so a file that omits an optional property still imports.

All tables are created with `CREATE TABLE IF NOT EXISTS`, so a repeated run against the same target loads into the existing family. Primary-key and foreign-key constraint statements are out of scope for this feature.

Limit: generated `_id` values repeat across runs. `json_tables_core` restarts the `_id` counter at 1 on every run, so an `_id` value is unique only within one run. A `_parent` value therefore resolves only against the rows that the same run wrote. The command prints this limit as a warning on stderr on every import.

exapump warns rather than refusing the run or recreating the family, because `json_tables_core` generates `_id` and exapump cannot renumber it. A refused repeated run would remove the append behavior that the CSV and Parquet uploads already offer. A dropped and recreated family would delete the rows an earlier run loaded.

exapump keeps every row in memory until the import starts, for both framings, so peak memory grows with the file size. NDJSON needs less memory than a JSON array, because exapump never parses the whole file into one value. This feature adds no chunking and no spill to disk.

Rejection of malformed or empty input, and reporting of a failure partway through a family load, are covered by the sibling feature `upload/json-import-rejection`.
<!-- /DELTA:CHANGED -->

## Scenarios

<!-- DELTA:NEW -->
### Scenario: Import an NDJSON file with the .jsonl extension

* *GIVEN* a file `events.jsonl` holds one JSON object per line
* *WHEN* the user runs `exapump upload events.jsonl --table raw.events --dsn <dsn>`
* *THEN* the command MUST import one row per non-empty line into `"RAW"."EVENTS"`
* *AND* the command MUST NOT report the file format as unsupported
* *AND* the command MUST exit with code 0
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Integer column holds the full 64-bit signed range

* *GIVEN* a file `ids.json` holds the single document `{"id": 1234567890123456789}`
* *WHEN* the user runs `exapump upload ids.json --table raw.ids --dsn <dsn>`
* *THEN* the command MUST create the column `id` with the Exasol type `DECIMAL(19,0)`
* *AND* the command MUST import the value `1234567890123456789` unchanged
* *AND* the command MUST exit with code 0
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Repeated run places each value in the column of its own name

* *GIVEN* a file `first.json` holding `[{"a": "x", "b": "y"}]` was already imported into `"RAW"."REORDER"`
* *AND* a file `second.json` holds `[{"b": "y2", "a": "x2"}]`, which lists the same two properties in the other order
* *WHEN* the user runs `exapump upload second.json --table raw.reorder --dsn <dsn>`
* *THEN* the command MUST load `x2` into column `a` and `y2` into column `b`
* *AND* `"RAW"."REORDER"` MUST hold 2 rows
* *AND* the command MUST exit with code 0
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Second file omits a column the target table holds

* *GIVEN* a file `first.json` holding `[{"a": 1, "b": "x"}, {"a": 2, "b": "y"}]` was already imported into `"RAW"."OPTIONAL"`
* *AND* a file `second.json` holds `[{"a": 3}]`, which names no property `b`
* *WHEN* the user runs `exapump upload second.json --table raw.optional --dsn <dsn>`
* *THEN* the command MUST load one row holding `3` in column `a` and NULL in column `b`
* *AND* `"RAW"."OPTIONAL"` MUST hold 3 rows, of which the two the first file loaded still hold `x` and `y` in column `b`
* *AND* the command MUST exit with code 0
<!-- /DELTA:NEW -->
