# Feature: JSON Import

Upload a JSON or NDJSON file into Exasol as a relational table family. A flat document set becomes one table. A document set with nested objects and arrays becomes a root table plus one subtable per nested path, linked by generated key columns.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn`, `EXAPUMP_DSN`, or `--profile`. File format is detected from the file extension (`.json` or `.ndjson`). The upload command is async.

Normalization is performed by the `json_tables_core` crate from `exasol-labs/exasol-json-tables`. exapump owns file reading, connection handling, the Arrow conversion, and the import. `json_tables_core` owns every decision about which tables exist, which columns they carry, and which DDL describes them.

Framing is detected from the first non-whitespace byte of the file, not from the extension. A leading `[` selects a single top-level JSON array of objects. Any other byte selects NDJSON, one JSON object per line. Both extensions accept both framings.

The document set is read twice. The first pass collects property and type statistics and derives the table family. The second pass writes rows into in-memory column buffers.

`--table <name>` names the root table and supplies the base name for every subtable. exapump uppercases the schema part and the table part of `--table`, then quotes both, so one `--table` value names the same root table for JSON, CSV, and Parquet input. A subtable name is the uppercased root table name, an underscore, and the encoded JSON path, with array segments suffixed `_arr`. The path segment keeps the JSON key case, because two JSON keys may differ only by case. `--table sales.orders` with a document property `items` holding an array therefore yields `"SALES"."ORDERS"` and `"SALES"."ORDERS_items_arr"`.

Every table of a family is created and loaded in one resolved schema. When `--table` carries a schema part, that part supplies it. When `--table` carries no schema part, the connection's default schema supplies it. When neither supplies one, the command fails before creating any table. Under `--dry-run` there is no connection, so an unqualified `--table` previews unqualified table names.

Column typing follows the `json_tables_core` contract:

| JSON value | Exasol column type |
|------------|--------------------|
| boolean | `BOOLEAN` |
| integer | `DECIMAL(18,0)` |
| fractional number | `DOUBLE` |
| string | `VARCHAR(2000000)` |

A property whose values carry more than one scalar type gets a primary column for the majority type and one `<name>|<type>` sibling column per remaining type. The type token is the `json_tables_core` type label, for example `integer` or `string`. A property that appears as an explicit JSON `null` in at least one document gets a `<name>|n` boolean mask column, so an explicit null stays distinguishable from an absent field.

Generated key columns link the family. Every object table carries `_id`, including the root table of a flat document set that has no children. An array element table carries `_parent` and `_pos`, and carries `_id` only when it holds a nested array of its own. A parent of a nested object carries a `<name>|object` column holding the child row's `_id`.

The command creates, loads, and reports the tables of a family in a deterministic order derived from the table path, so two runs over the same input report the same table order.

All tables are created with `CREATE TABLE IF NOT EXISTS`, so a repeated run against the same target loads into the existing family. Primary-key and foreign-key constraint statements are out of scope for this feature.

Limit: generated `_id` values repeat across runs. `json_tables_core` restarts the `_id` counter at 1 on every run, so an `_id` value is unique only within one run. A `_parent` value therefore resolves only against the rows that the same run wrote. The command prints this limit as a warning on stderr on every import.

Rows are imported per table over `exarrow_rs::Connection::import_from_record_batches`. The import is not atomic across the table family. A failure partway through leaves the tables already loaded in place.

The whole family is buffered in memory before the import starts. A top-level JSON array is also parsed as one value. Peak memory therefore scales with the file size, and NDJSON framing is the shape to prefer for a large input because it streams the read pass. This feature adds no chunking or spill-to-disk.

Rejection of malformed or empty input, and reporting of a failure partway through a family load, are covered by the sibling feature `upload/json-import-rejection`.

## Scenarios

### Scenario: Import a flat JSON array into one table

* *GIVEN* a file `orders.json` holds a top-level JSON array of objects with only scalar properties
* *AND* no target table exists in Exasol
* *WHEN* the user runs `exapump upload orders.json --table sales.orders --dsn <dsn>`
* *THEN* the command MUST create exactly one table `"SALES"."ORDERS"`, carrying an `_id` column plus one column per JSON property typed per the contract table in Background
* *AND* the command MUST import one row per document
* *AND* the command MUST print the row count loaded for `"SALES"."ORDERS"` and exit with code 0

### Scenario: Import nested JSON creates a subtable per nested path

* *GIVEN* a file `orders.json` holds documents with a nested object property `customer` and a nested array property `items`
* *AND* no target table exists in Exasol
* *WHEN* the user runs `exapump upload orders.json --table sales.orders --dsn <dsn>`
* *THEN* the command MUST create the tables `"SALES"."ORDERS"`, `"SALES"."ORDERS_customer"`, and `"SALES"."ORDERS_items_arr"`
* *AND* the command MUST import every nested object and every array element into its own subtable
* *AND* the command MUST print the row count loaded for each table and exit with code 0

### Scenario: Nested tables carry the generated key columns

* *GIVEN* a file `orders.json` holds documents with a nested object property `customer` and a nested array property `items`
* *WHEN* the user runs `exapump upload orders.json --table sales.orders --dsn <dsn>`
* *THEN* `"SALES"."ORDERS"` MUST carry a `"customer|object"` column holding the `_id` of the matching `"SALES"."ORDERS_customer"` row
* *AND* `"SALES"."ORDERS_items_arr"` MUST carry a `_parent` column holding the `_id` of its parent row
* *AND* `"SALES"."ORDERS_items_arr"` MUST carry a `_pos` column holding the zero-based element position

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

### Scenario: Repeated run loads into the existing family

* *GIVEN* a file `orders.json` was already imported into `"SALES"."ORDERS"` and its subtables
* *WHEN* the user runs the same `exapump upload orders.json --table sales.orders --dsn <dsn>` again
* *THEN* the command MUST NOT fail on table creation
* *AND* the command MUST append the documents to the existing tables
* *AND* the command MUST print a warning to stderr stating that `_id` values repeat across runs and the family linkage holds only within one run
* *AND* the command MUST exit with code 0

### Scenario: Unqualified table name uses the connection schema

* *GIVEN* a file `orders.json` exists
* *AND* the DSN selects a default schema
* *WHEN* the user runs `exapump upload orders.json --table orders --dsn <dsn>`
* *THEN* the command MUST read the default schema from the connection and create the whole family in it
* *AND* every import statement MUST name that schema explicitly rather than rely on session state
* *AND* the command MUST exit with code 0
