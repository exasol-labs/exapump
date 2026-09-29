# Feature: JSON Import Rejection

Reject a JSON, NDJSON, or JSONL upload whose file cannot be found, framed, or read as usable document data. This feature covers the input-validation error paths of `upload/json-import`. Rejection of an unresolved schema, a colliding table family, or a failure partway through loading a family is covered by the sibling feature `upload/json-import-rejection-family`.

## Background

The sibling feature `upload/json-import` covers connection setup, framing detection, `--table` handling, column typing, and the import. The sibling feature `upload/json-import-nesting` covers subtable naming and the generated key columns. This Background states only what the input-validation error paths add.

exapump rejects a family that carries no document data. `_id`, `_parent`, `_pos`, and the `<name>|object` and `<name>|array` link columns are generated, so a table holding only those columns carries nothing that came out of a document.

## Scenarios

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
* *THEN* the command MUST reject the input, because no planned table carries a column that came out of a document
* *AND* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that no column could be derived from the documents
* *AND* the command MUST NOT create any table

### Scenario: Nested object that holds no properties

* *GIVEN* a file `hollow.json` holds the single document `{"a": {}}`
* *WHEN* the user runs `exapump upload hollow.json --table raw.hollow --dsn <dsn>`
* *THEN* the command MUST reject the input, because the root table carries only `_id` and the generated `"a|object"` link column, and the subtable carries only `_id`
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
