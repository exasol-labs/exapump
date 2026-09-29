# Feature: JSON Import Nesting

<!-- DELTA:CHANGED -->
## Background

The sibling feature `upload/json-import` covers connection setup, framing detection, `--table` handling, schema resolution, column typing, and repeated-run behavior. This Background states only the table-family shape.

The `json_tables_core` crate from `exasol-labs/exasol-json-tables` decides which tables the family holds and which columns each table carries. exapump renders the `CREATE TABLE` statements from that plan and runs them.

A subtable name is the uppercased root table name, an underscore, and the encoded JSON path, with array segments suffixed `_arr`. The path segment keeps the JSON key case, because two JSON keys may differ only by case. `--table sales.orders` with a document property `items` holding an array therefore yields `"SALES"."ORDERS"` and `"SALES"."ORDERS_items_arr"`.

The separator between two path segments becomes `_`, and an underscore inside a JSON key stays `_`. A nested path and a single key can therefore produce one table name. The sibling feature `upload/json-import-rejection` covers the rejection of such a family.

Nesting fans out past one level. A nested path yields one subtable per level, so an object inside an object, an object inside an array element, and an array inside an array element each get their own subtable. A keyed property of an array element keeps its key, for example `"DEEP_items_arr_tags_arr"`. An array element that is itself an array carries no key, so its own subtable takes the path segment `value`, for example `"MAT_matrix_arr_value_arr"`.

Generated key columns link the family. Every object table carries `_id`, including the root table of a flat document set that has no children. An array element table carries `_parent` and `_pos`, and carries `_id` only when it holds a nested array of its own. A parent of a nested object carries a `<name>|object` column holding the child row's `_id`. A parent of a nested array carries a `<name>|array` column holding that array's element count, named `_value|array` when the parent is an array element that is itself an array.

A table is planned from the property, not from the values. A property that holds an array in every document therefore gets a subtable even when every one of those arrays is empty.

The command creates, loads, and reports the tables of a family in a deterministic order derived from the table path, so two runs over the same input report the same table order.

Column typing, mixed-type columns, null handling, and repeated-run behavior are covered by the sibling feature `upload/json-import`. Rejection of malformed or empty input is covered by `upload/json-import-rejection`.
<!-- /DELTA:CHANGED -->

## Scenarios

### Scenario: Import nested JSON creates a subtable per nested path

* *GIVEN* a file `orders.json` holds documents with a nested object property `customer` and a nested array property `items`
* *AND* no target table exists in Exasol
* *WHEN* the user runs `exapump upload orders.json --table sales.orders --dsn <dsn>`
* *THEN* the command MUST create the tables `"SALES"."ORDERS"`, `"SALES"."ORDERS_customer"`, and `"SALES"."ORDERS_items_arr"`
* *AND* the command MUST import every nested object and every array element into its own subtable
* *AND* the command MUST print the row count loaded for each table and exit with code 0
