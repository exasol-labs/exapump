# Feature: JSON Import

Upload a JSON or NDJSON file into Exasol as a relational table family. A flat document set becomes one table with one column per scalar property. This feature covers file framing, dry-run preview, and scalar column typing. Schema resolution, column-name-based import, and repeated-run behavior are covered by the sibling feature `upload/json-import-columns`. Fan-out of nested objects and arrays into subtables, and the generated key columns that link them, are covered by `upload/json-import-nesting`.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn`, `EXAPUMP_DSN`, or `--profile`. File format is detected from the file extension (`.json`, `.ndjson`, or `.jsonl`). The upload command is async.

The `json_tables_core` crate from `exasol-labs/exasol-json-tables` normalizes the document set. `json_tables_core` decides which tables the family holds and which columns each table carries. exapump owns file reading, framing, the SQL it runs, connection handling, the Arrow conversion, and the import.

Framing is detected from the first non-whitespace byte of the file, not from the extension. A leading `[` selects a single top-level JSON array of objects. Any other byte selects NDJSON, one JSON object per line. All three extensions accept both framings. A single JSON object spread over several lines matches neither framing. The sibling feature `upload/json-import-rejection` covers its rejection.

The document set is read twice. The first pass collects property and type statistics and derives the table family. The second pass writes rows into in-memory column buffers.

`--table <name>` names the root table and supplies the base name for every subtable. exapump uppercases the schema part and the table part of `--table`, then quotes both, so one `--table` value names the same root table for JSON, CSV, and Parquet input. The sibling feature `upload/json-import-columns` covers schema resolution when `--table` carries no schema part. Subtable naming for nested paths, and the array fan-out that produces them, is covered by the sibling feature `upload/json-import-nesting`.

exapump renders the `CREATE TABLE` statements from the column plan and maps each contract type to an Exasol type:

| JSON value | Exasol column type |
|------------|--------------------|
| boolean | `BOOLEAN` |
| integer | `DECIMAL(19,0)` |
| fractional number | `DOUBLE` |
| string | `VARCHAR(2000000)` |

A number outside the 64-bit signed range is classified as a fractional number and lands in a `DOUBLE` column. The sibling feature `upload/json-import-columns` covers the full 64-bit signed range that an integer column holds.

A property whose values carry more than one scalar type gets a primary column for the majority type and one `<name>|<type>` sibling column per remaining type. The type token is the `json_tables_core` type label, for example `integer` or `string`. A property that appears as an explicit JSON `null` in at least one document gets a `<name>|n` boolean mask column, so an explicit null stays distinguishable from an absent field.

The command creates, loads, and reports the tables of a family in a deterministic order derived from the table path, so two runs over the same input report the same table order.

exapump imports rows by column name, so a value lands in the column of its own name whatever order the file lists the properties in. The sibling feature `upload/json-import-columns` covers column-name-based import, repeated runs against an existing family, and the schema each run resolves.

exapump keeps every row in memory until the import starts, for both framings, so peak memory grows with the file size. NDJSON needs less memory than a JSON array, because exapump never parses the whole file into one value. This feature adds no chunking and no spill to disk.

Rejection of malformed or empty input, and reporting of a failure partway through a family load, are covered by the sibling feature `upload/json-import-rejection`.

## Scenarios

### Scenario: Import a flat JSON array into one table

* *GIVEN* a file `orders.json` holds a top-level JSON array of objects with only scalar properties
* *AND* no target table exists in Exasol
* *WHEN* the user runs `exapump upload orders.json --table sales.orders --dsn <dsn>`
* *THEN* the command MUST create exactly one table `"SALES"."ORDERS"`, carrying an `_id` column plus one column per JSON property typed per the contract table in Background
* *AND* the command MUST import one row per document
* *AND* the command MUST print the row count loaded for `"SALES"."ORDERS"` and exit with code 0

### Scenario: Import an NDJSON file

* *GIVEN* a file `events.ndjson` holds one JSON object per line and one blank line
* *WHEN* the user runs `exapump upload events.ndjson --table raw.events --dsn <dsn>`
* *THEN* the command MUST import one row per non-empty line into `"RAW"."EVENTS"`
* *AND* the command MUST skip the blank line without raising an error
* *AND* the command MUST exit with code 0

### Scenario: Framing detected from file content, not extension

* *GIVEN* a file `events.json` holds one JSON object per line rather than a top-level array
* *WHEN* the user runs `exapump upload events.json --table raw.events --dsn <dsn>`
* *THEN* the command MUST read the file as NDJSON and import one row per non-empty line
* *AND* the command MUST exit with code 0

### Scenario: Dry-run shows the planned table family

* *GIVEN* a file `orders.json` holds documents with a nested array property `items`
* *WHEN* the user runs `exapump upload orders.json --table sales.orders --dsn <dsn> --dry-run`
* *THEN* the command MUST print the name, column names, and Exasol column types of every table in the planned family
* *AND* the command MUST print the planned `CREATE TABLE IF NOT EXISTS` statement for every table
* *AND* the command MUST NOT connect to Exasol or modify any data
* *AND* the command MUST exit with code 0

### Scenario: Property with mixed scalar types gets alternate columns

* *GIVEN* a file `mixed.json` holds documents where the property `code` appears as a string in most documents and as an integer in the rest
* *WHEN* the user runs `exapump upload mixed.json --table raw.mixed --dsn <dsn>`
* *THEN* the command MUST create a `code` column typed for the majority type and a `"code|integer"` column for the integer values
* *AND* each row MUST carry its value in the column matching that document's type
* *AND* the column for the other type MUST be NULL in that row

### Scenario: Explicit JSON null stays distinct from an absent property

* *GIVEN* a file `nulls.json` holds one document where `note` is explicit JSON `null` and one document where `note` is absent
* *WHEN* the user runs `exapump upload nulls.json --table raw.nulls --dsn <dsn>`
* *THEN* the command MUST create a `"note|n"` boolean mask column
* *AND* the mask column MUST hold `TRUE` for the document with the explicit null
* *AND* the mask column MUST hold `FALSE` for the document with the absent property

### Scenario: Import an NDJSON file with the .jsonl extension

* *GIVEN* a file `events.jsonl` holds one JSON object per line
* *WHEN* the user runs `exapump upload events.jsonl --table raw.events --dsn <dsn>`
* *THEN* the command MUST import one row per non-empty line into `"RAW"."EVENTS"`
* *AND* the command MUST NOT report the file format as unsupported
* *AND* the command MUST exit with code 0
