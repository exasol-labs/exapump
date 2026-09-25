# Feature: JSON Import Rejection

Reject a JSON or NDJSON upload that cannot resolve a target schema, that cannot be parsed into a table family, or that fails partway through loading its table family. This feature covers the error paths of `upload/json-import`; see that feature for the table-family, framing, and typing rules that a successful import follows.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn`, `EXAPUMP_DSN`, or `--profile`. File format is detected from the file extension (`.json` or `.ndjson`). The upload command is async.

Normalization is performed by the `json_tables_core` crate from `exasol-labs/exasol-json-tables`. exapump owns file reading, connection handling, the Arrow conversion, and the import. `json_tables_core` owns every decision about which tables exist, which columns they carry, and which DDL describes them.

`--table <name>` names the root table and supplies the base name for every subtable. Every table of a family is created and loaded in one resolved schema. When `--table` carries a schema part, that part supplies it. When `--table` carries no schema part, the connection's default schema supplies it. When neither supplies one, the command fails before creating any table.

Resolving a schema name and opening it are two separate steps with two separate failures. A name that no rule supplies fails during resolution. A resolved name that Exasol does not hold fails when the command opens the schema.

Rows are imported per table over `exarrow_rs::Connection::import_from_record_batches`. The import is not atomic across the table family. A failure partway through leaves the tables already loaded in place.

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
* *AND* stderr MUST state that it failed to open the schema
* *AND* stderr MUST name the schema `NO_SUCH_SCHEMA_XYZ`

### Scenario: Import failure reports the tables already loaded

* *GIVEN* a file `orders.json` produces a family of more than one table
* *AND* one table in the family cannot be loaded
* *WHEN* the user runs `exapump upload orders.json --table sales.orders --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code, and stderr MUST name the table that failed
* *AND* stdout MUST list the tables loaded before the failure
* *AND* the command MUST NOT roll back the tables loaded before the failure

### Scenario: Empty JSON file

* *GIVEN* a file `empty.json` exists and contains no non-whitespace bytes
* *WHEN* the user runs `exapump upload empty.json --table raw.empty --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that the file is empty
* *AND* the command MUST NOT create any table

### Scenario: JSON file with no documents

* *GIVEN* a file `none.json` holds an empty top-level JSON array
* *WHEN* the user runs `exapump upload none.json --table raw.none --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that the file contains no documents
* *AND* the command MUST NOT create any table

### Scenario: Documents with no properties

* *GIVEN* a file `blank.json` holds a top-level JSON array whose every element is an empty object
* *WHEN* the user runs `exapump upload blank.json --table raw.blank --dsn <dsn>`
* *THEN* the command MUST reject the input because every planned table carries only the generated key columns `_id`, `_parent`, and `_pos`
* *AND* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that no column could be derived from the documents
* *AND* the command MUST NOT create any table

### Scenario: Document that is not a JSON object

* *GIVEN* a file `scalars.json` holds a top-level JSON array whose third element is the number `42`
* *WHEN* the user runs `exapump upload scalars.json --table raw.scalars --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that the entry is not an object
* *AND* stderr MUST name the position of the offending entry

### Scenario: JSON file not found

* *GIVEN* the specified file path does not exist
* *WHEN* the user runs `exapump upload missing.json --table raw.missing --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST contain the file path that was not found

### Scenario: Connection failure

* *GIVEN* a valid JSON file exists
* *AND* the DSN points to an unreachable Exasol host
* *WHEN* the user runs `exapump upload orders.json --table sales.orders --dsn exasol://bad:bad@nowhere:9999`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that the connection to Exasol failed
