# Feature: JSON Import Rejection Family

Reject a JSON, NDJSON, or JSONL upload that cannot resolve a target schema or whose planned table family collides, and report a failure partway through loading a family. This feature covers the schema-resolution and family-assembly error paths of `upload/json-import`. Rejection of a file that cannot be found, framed, or read as usable document data is covered by the sibling feature `upload/json-import-rejection`.

## Background

The sibling feature `upload/json-import` covers connection setup, framing detection, `--table` handling, column typing, and the import. The sibling feature `upload/json-import-columns` covers schema resolution and column-name-based import on a successful run. This Background states only what the schema-resolution and family-assembly error paths add.

exapump resolves one target schema before it runs any statement, and names that schema in every `CREATE TABLE` statement and every `IMPORT` statement. A schema name that no rule supplies fails during resolution, before any table is created. A resolved name that Exasol does not hold fails when the command creates the family's first table.

exapump rejects a family whose planned table names are not unique, because the load would write two different JSON paths into one table.

exapump imports rows per table over `exarrow_rs::Connection::import_from_record_batches`, by column name. The import is not atomic across the table family. A failure partway through leaves the tables already loaded in place.

## Scenarios

### Scenario: Unqualified table name with no connection schema

* *GIVEN* a file `orders.json` exists
* *AND* the DSN selects no default schema
* *WHEN* the user runs `exapump upload orders.json --table orders --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST state that no target schema could be resolved
* *AND* the command MUST NOT create any table

### Scenario: Qualified table name whose schema does not exist

* *GIVEN* a file `orders.json` exists
* *AND* Exasol holds no schema named `NO_SUCH_SCHEMA_XYZ`
* *WHEN* the user runs `exapump upload orders.json --table NO_SUCH_SCHEMA_XYZ.orders --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST state that it failed to create the first table of the family
* *AND* stderr MUST name the schema `NO_SUCH_SCHEMA_XYZ`

### Scenario: Import failure reports the tables already loaded

* *GIVEN* a file `orders.json` produces a family of more than one table
* *AND* one table in the family cannot be loaded
* *WHEN* the user runs `exapump upload orders.json --table sales.orders --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code, and stderr MUST name the table that failed
* *AND* stdout MUST list the tables loaded before the failure
* *AND* the command MUST NOT roll back the tables loaded before the failure

### Scenario: Two JSON paths map to one table name

* *GIVEN* a file `clash.json` holds the single document `{"customer": {"address": {"city": "B"}}, "customer_address": {"zip": "1"}}`
* *WHEN* the user runs `exapump upload clash.json --table raw.col --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST name the table name `COL_customer_address`
* *AND* stderr MUST name both JSON paths `customer.address` and `customer_address`
* *AND* the command MUST NOT create any table

### Scenario: Second file carries a column the target table does not hold

* *GIVEN* a file `first.json` holding `[{"a": 1, "b": "from_b"}]` was already imported into `"RAW"."MISMATCH"`
* *AND* a file `second.json` holds `[{"a": 2, "c": "from_c"}]`
* *WHEN* the user runs `exapump upload second.json --table raw.mismatch --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST name the table `"RAW"."MISMATCH"` and the column `c`
* *AND* `"RAW"."MISMATCH"` MUST still hold exactly one row, whose column `b` holds `from_b`

### Scenario: Single JSON object spread over several lines

* *GIVEN* a file `single.json` holds one JSON object formatted over several lines
* *WHEN* the user runs `exapump upload single.json --table raw.single --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST state that the file is neither a JSON array of objects nor one JSON object per line
* *AND* the command MUST NOT create any table
