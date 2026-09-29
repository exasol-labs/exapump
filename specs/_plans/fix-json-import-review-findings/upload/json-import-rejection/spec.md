<!-- DELTA:CHANGED -->
# Feature: JSON Import Rejection

Reject a JSON, NDJSON, or JSONL upload that cannot resolve a target schema, that cannot be framed or planned into a usable table family, or that fails partway through loading its table family. This feature covers the error paths of `upload/json-import`. See that feature for the table-family, framing, and typing rules that a successful import follows.
<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
## Background

The sibling feature `upload/json-import` covers connection setup, framing detection, `--table` handling, column typing, and the import. The sibling feature `upload/json-import-nesting` covers subtable naming and the generated key columns. This Background states only what the error paths add.

exapump resolves one target schema before it runs any statement, and names that schema in every `CREATE TABLE` statement and every `IMPORT` statement. A schema name that no rule supplies fails during resolution, before any table is created. A resolved name that Exasol does not hold fails when the command creates the family's first table.

exapump rejects a family that carries no document data. `_id`, `_parent`, `_pos`, and the `<name>|object` and `<name>|array` link columns are generated, so a table holding only those columns carries nothing that came out of a document.

exapump rejects a family whose planned table names are not unique, because the load would write two different JSON paths into one table.

exapump imports rows per table over `exarrow_rs::Connection::import_from_record_batches`, by column name. The import is not atomic across the table family. A failure partway through leaves the tables already loaded in place.
<!-- /DELTA:CHANGED -->

## Scenarios

<!-- DELTA:CHANGED -->
### Scenario: Qualified table name whose schema does not exist

* *GIVEN* a file `orders.json` exists
* *AND* Exasol holds no schema named `NO_SUCH_SCHEMA_XYZ`
* *WHEN* the user runs `exapump upload orders.json --table NO_SUCH_SCHEMA_XYZ.orders --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST state that it failed to create the first table of the family
* *AND* stderr MUST name the schema `NO_SUCH_SCHEMA_XYZ`
<!-- /DELTA:CHANGED -->

<!-- DELTA:CHANGED -->
### Scenario: Documents with no properties

* *GIVEN* a file `blank.json` holds a top-level JSON array whose every element is an empty object
* *WHEN* the user runs `exapump upload blank.json --table raw.blank --dsn <dsn>`
* *THEN* the command MUST reject the input, because no planned table carries a column that came out of a document
* *AND* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that no column could be derived from the documents
* *AND* the command MUST NOT create any table
<!-- /DELTA:CHANGED -->

<!-- DELTA:NEW -->
### Scenario: Nested object that holds no properties

* *GIVEN* a file `hollow.json` holds the single document `{"a": {}}`
* *WHEN* the user runs `exapump upload hollow.json --table raw.hollow --dsn <dsn>`
* *THEN* the command MUST reject the input, because the root table carries only `_id` and the generated `"a|object"` link column, and the subtable carries only `_id`
* *AND* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that no column could be derived from the documents
* *AND* the command MUST NOT create any table
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Two JSON paths map to one table name

* *GIVEN* a file `clash.json` holds the single document `{"customer": {"address": {"city": "B"}}, "customer_address": {"zip": "1"}}`
* *WHEN* the user runs `exapump upload clash.json --table raw.col --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST name the table name `COL_customer_address`
* *AND* stderr MUST name both JSON paths `customer.address` and `customer_address`
* *AND* the command MUST NOT create any table
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Second file carries a column the target table does not hold

* *GIVEN* a file `first.json` holding `[{"a": 1, "b": "from_b"}]` was already imported into `"RAW"."MISMATCH"`
* *AND* a file `second.json` holds `[{"a": 2, "c": "from_c"}]`
* *WHEN* the user runs `exapump upload second.json --table raw.mismatch --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST name the table `"RAW"."MISMATCH"` and the column `c`
* *AND* `"RAW"."MISMATCH"` MUST still hold exactly one row, whose column `b` holds `from_b`
<!-- /DELTA:NEW -->

<!-- DELTA:NEW -->
### Scenario: Single JSON object spread over several lines

* *GIVEN* a file `single.json` holds one JSON object formatted over several lines
* *WHEN* the user runs `exapump upload single.json --table raw.single --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST state that the file is neither a JSON array of objects nor one JSON object per line
* *AND* the command MUST NOT create any table
<!-- /DELTA:NEW -->
