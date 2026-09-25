# Feature: JSON Import Nesting

Fan out a document set's nested objects and arrays into a subtable per nested path, linked to their parent by generated key columns. This feature covers the table-family shape of `upload/json-import`; see that feature for connection setup, framing detection, dry-run preview, scalar column typing, and repeated-run behavior.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn`, `EXAPUMP_DSN`, or `--profile`. The upload command is async.

Normalization is performed by the `json_tables_core` crate from `exasol-labs/exasol-json-tables`. exapump owns file reading, connection handling, the Arrow conversion, and the import. `json_tables_core` owns every decision about which tables exist, which columns they carry, and which DDL describes them.

`--table <name>` names the root table and supplies the base name for every subtable. exapump uppercases the schema part and the table part of `--table`, then quotes both. A subtable name is the uppercased root table name, an underscore, and the encoded JSON path, with array segments suffixed `_arr`. The path segment keeps the JSON key case, because two JSON keys may differ only by case. `--table sales.orders` with a document property `items` holding an array therefore yields `"SALES"."ORDERS"` and `"SALES"."ORDERS_items_arr"`.

Nesting fans out past one level. A nested path yields one subtable per level, so an object inside an object, an object inside an array element, and an array inside an array element each get their own subtable. A keyed property of an array element keeps its key, for example `"DEEP_items_arr_tags_arr"`. An array element that is itself an array carries no key, so its own subtable takes the path segment `value`, for example `"MAT_matrix_arr_value_arr"`.

Every table of a family is created and loaded in one resolved schema. When `--table` carries a schema part, that part supplies it. When `--table` carries no schema part, the connection's default schema supplies it.

Generated key columns link the family. Every object table carries `_id`, including the root table of a flat document set that has no children. An array element table carries `_parent` and `_pos`, and carries `_id` only when it holds a nested array of its own. A parent of a nested object carries a `<name>|object` column holding the child row's `_id`. A parent of a nested array carries a `<name>|array` column holding that array's element count, named `_value|array` when the parent is an array element that is itself an array.

A table is planned from the property, not from the values. A property that holds an array in every document therefore gets a subtable even when every one of those arrays is empty.

The command creates, loads, and reports the tables of a family in a deterministic order derived from the table path, so two runs over the same input report the same table order.

Column typing, mixed-type columns, null handling, and repeated-run behavior are covered by the sibling feature `upload/json-import`. Rejection of malformed or empty input is covered by `upload/json-import-rejection`.

## Scenarios

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

### Scenario: Array empty in every document still gets its subtable

* *GIVEN* a file `orders.json` holds the documents `{"id": 1, "items": []}` and `{"id": 2, "items": []}`, and no target table exists in Exasol
* *WHEN* the user runs `exapump upload orders.json --table sales.orders --dsn <dsn>`
* *THEN* the command MUST create both `"SALES"."ORDERS"` and `"SALES"."ORDERS_items_arr"`
* *AND* the command MUST import 2 rows into `"SALES"."ORDERS"` and 0 rows into `"SALES"."ORDERS_items_arr"`
* *AND* the command MUST NOT reject the input, because `"SALES"."ORDERS"` carries the document property `id`
* *AND* the command MUST exit with code 0

### Scenario: Nesting past one level creates a subtable at every depth

* *GIVEN* a file `deep.json` holds one document with the nested object `customer`, the nested object `customer.address`, the nested array `items`, the nested object `items[].meta`, and the nested array `items[].tags`
* *WHEN* the user runs `exapump upload deep.json --table sales.deep --dsn <dsn>`
* *THEN* the command MUST create 6 tables: `"SALES"."DEEP"`, `"SALES"."DEEP_customer"`, `"SALES"."DEEP_customer_address"`, `"SALES"."DEEP_items_arr"`, `"SALES"."DEEP_items_arr_meta"`, and `"SALES"."DEEP_items_arr_tags_arr"`, print the row count loaded for each, print `Imported 10 rows in total`, and exit with code 0
* *AND* `"SALES"."DEEP_customer"` MUST carry an `"address|object"` column holding the `_id` of the matching `"SALES"."DEEP_customer_address"` row
* *AND* `"SALES"."DEEP_items_arr"` MUST carry a `"meta|object"` column holding the `_id` of the matching `"SALES"."DEEP_items_arr_meta"` row
* *AND* `"SALES"."DEEP_items_arr_tags_arr"` MUST carry a `_parent` column holding the `_id` of its parent `"SALES"."DEEP_items_arr"` row

### Scenario: Array of arrays fans out one subtable per array depth

* *GIVEN* a file `matrix.json` holds the documents `{"id": 1, "matrix": [[1, 2], [3, 4, 5]]}` and `{"id": 2, "matrix": [[6]]}`
* *WHEN* the user runs `exapump upload matrix.json --table sales.mat --dsn <dsn>`
* *THEN* the command MUST create 3 tables: `"SALES"."MAT"`, `"SALES"."MAT_matrix_arr"`, and `"SALES"."MAT_matrix_arr_value_arr"`, import 2 rows, 3 rows, and 6 rows into them, print `Imported 11 rows in total`, and exit with code 0
* *AND* `"SALES"."MAT"` MUST carry a `"matrix|array"` column holding 2 for the document `id` 1 and 1 for the document `id` 2
* *AND* `"SALES"."MAT_matrix_arr"` MUST carry a `_parent` column, a `_pos` column, and a `"_value|array"` column holding the element count of the sub-array at that position
* *AND* `"SALES"."MAT_matrix_arr_value_arr"` MUST hold one row per leaf value, linked to its sub-array row by `_parent`
