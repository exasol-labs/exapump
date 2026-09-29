# Feature: JSON Import Columns

Resolve the target schema for a JSON or NDJSON upload, import each batch by column name, and load a repeated run into the table family an earlier run created. This feature covers the schema, column-matching, and repeated-run rules of `upload/json-import`; see that feature for file framing, dry-run preview, and scalar column typing.

## Background

The sibling feature `upload/json-import` covers connection setup, framing detection, `--table` naming, and scalar column typing. This Background states only what schema resolution, column matching, and repeated runs add.

Every table of a family is created and loaded in one resolved schema. When `--table` carries a schema part, that part supplies it. When `--table` carries no schema part, the connection's default schema supplies it. When neither supplies one, the command fails before creating any table. exapump resolves the schema once, before the first statement, and names it in every `CREATE TABLE` statement and every `IMPORT` statement. Under `--dry-run` there is no connection, so an unqualified `--table` previews unqualified table names.

An integer column holds a 64-bit signed integer, so `DECIMAL(19,0)` covers every value `json_tables_core` classifies as an integer. A number outside the 64-bit signed range is classified as a fractional number and lands in a `DOUBLE` column instead, per the sibling feature `upload/json-import`.

exapump imports rows by column name. Each `IMPORT` statement names the columns of the batch it sends, so a value lands in the column of its own name whatever order the file lists the properties in. A batch column that the target table does not hold fails the import. The sibling feature `upload/json-import-rejection` covers that failure. A target-table column the batch does not name loads NULL, so a file that omits an optional property still imports.

All tables are created with `CREATE TABLE IF NOT EXISTS`, so a repeated run against the same target loads into the existing family. Primary-key and foreign-key constraint statements are out of scope for this feature.

Limit: generated `_id` values repeat across runs. `json_tables_core` restarts the `_id` counter at 1 on every run, so an `_id` value is unique only within one run. A `_parent` value therefore resolves only against the rows that the same run wrote. The command prints this limit as a warning on stderr on every import.

exapump warns rather than refusing the run or recreating the family, because `json_tables_core` generates `_id` and exapump cannot renumber it. A refused repeated run would remove the append behavior that the CSV and Parquet uploads already offer. A dropped and recreated family would delete the rows an earlier run loaded.

## Scenarios

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

### Scenario: Integer column holds the full 64-bit signed range

* *GIVEN* a file `ids.json` holds the single document `{"id": 1234567890123456789}`
* *WHEN* the user runs `exapump upload ids.json --table raw.ids --dsn <dsn>`
* *THEN* the command MUST create the column `id` with the Exasol type `DECIMAL(19,0)`
* *AND* the command MUST import the value `1234567890123456789` unchanged
* *AND* the command MUST exit with code 0

### Scenario: Repeated run places each value in the column of its own name

* *GIVEN* a file `first.json` holding `[{"a": "x", "b": "y"}]` was already imported into `"RAW"."REORDER"`
* *AND* a file `second.json` holds `[{"b": "y2", "a": "x2"}]`, which lists the same two properties in the other order
* *WHEN* the user runs `exapump upload second.json --table raw.reorder --dsn <dsn>`
* *THEN* the command MUST load `x2` into column `a` and `y2` into column `b`
* *AND* `"RAW"."REORDER"` MUST hold 2 rows
* *AND* the command MUST exit with code 0

### Scenario: Second file omits a column the target table holds

* *GIVEN* a file `first.json` holding `[{"a": 1, "b": "x"}, {"a": 2, "b": "y"}]` was already imported into `"RAW"."OPTIONAL"`
* *AND* a file `second.json` holds `[{"a": 3}]`, which names no property `b`
* *WHEN* the user runs `exapump upload second.json --table raw.optional --dsn <dsn>`
* *THEN* the command MUST load one row holding `3` in column `a` and NULL in column `b`
* *AND* `"RAW"."OPTIONAL"` MUST hold 3 rows, of which the two the first file loaded still hold `x` and `y` in column `b`
* *AND* the command MUST exit with code 0
