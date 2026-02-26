# Feature: Parquet Export

Export an Exasol table or SQL query result to one or more local Parquet files. Supports configurable compression and automatic file splitting based on row count or file size thresholds.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn` or `EXAPUMP_DSN`. For single-file export without splitting, the command delegates to `Connection::export_to_parquet()`. For split export (when `--max-rows-per-file` or `--max-file-size` is set), exapump fetches Arrow RecordBatches via `Connection::export_to_record_batches()` and writes them to Parquet files using the `parquet` crate's `ArrowWriter`, splitting into new files when thresholds are reached. Split files are named `<stem>_000.parquet`, `<stem>_001.parquet`, etc. If splitting is requested but only one file is produced, the output uses the original `--output` name without a suffix.

## Scenarios

### Scenario: Export table to single Parquet file

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --dsn <dsn>`
* *THEN* the command MUST write all rows from the table to `data.parquet` in Parquet format
* *AND* the Parquet file MUST use Snappy compression by default
* *AND* stderr MUST print the number of rows exported
* *AND* the command MUST exit with code 0

### Scenario: Export query result to Parquet file

* *GIVEN* a valid SQL query that returns rows
* *WHEN* the user runs `exapump export --query 'SELECT id, name FROM schema.table WHERE active = true' --output results.parquet --format parquet --dsn <dsn>`
* *THEN* the command MUST write all result rows to `results.parquet` in Parquet format
* *AND* stderr MUST print the number of rows exported
* *AND* the command MUST exit with code 0

### Scenario: Query with WHERE clause exports only matching rows to Parquet

* *GIVEN* a table with 3 rows exists in Exasol (ids 1, 2, 3)
* *WHEN* the user runs `exapump export --query 'SELECT * FROM schema.table WHERE id > 1' --output filtered.parquet --format parquet --dsn <dsn>`
* *THEN* the command MUST write only the 2 matching rows to `filtered.parquet`
* *AND* the Parquet file MUST contain a valid schema
* *AND* stderr MUST print the number of rows exported
* *AND* the command MUST exit with code 0

### Scenario: Custom compression codec

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --compression zstd --dsn <dsn>`
* *THEN* the Parquet file MUST use Zstd compression

### Scenario: No compression

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --compression none --dsn <dsn>`
* *THEN* the Parquet file MUST use no compression

### Scenario: Split by max rows per file

* *GIVEN* a table with 10 rows exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --max-rows-per-file 3 --dsn <dsn>`
* *THEN* the command MUST produce 4 files: `data_000.parquet`, `data_001.parquet`, `data_002.parquet`, `data_003.parquet`
* *AND* each of the first three files MUST contain exactly 3 rows
* *AND* the last file MUST contain the remaining 1 row
* *AND* stderr MUST print the total number of rows exported and the number of files written
* *AND* the command MUST exit with code 0

### Scenario: Split by max file size

* *GIVEN* a table with enough data to exceed the size threshold
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --max-file-size 1MB --dsn <dsn>`
* *THEN* the command MUST split output into multiple files where each file's size SHOULD NOT exceed the threshold
* *AND* files MUST be named `data_000.parquet`, `data_001.parquet`, etc.
* *AND* stderr MUST print the total number of rows exported and the number of files written

### Scenario: Split produces single file

* *GIVEN* a table with 5 rows exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --max-rows-per-file 1000 --dsn <dsn>`
* *THEN* the command MUST write the output to `data.parquet` (the original `--output` name, no numeric suffix)
* *AND* stderr MUST print the number of rows exported
* *AND* the command MUST exit with code 0

### Scenario: Both split thresholds active

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --max-rows-per-file 100000 --max-file-size 50MB --dsn <dsn>`
* *THEN* the command MUST split into a new file whenever either threshold is reached first

### Scenario: File size accepts human-readable units

* *GIVEN* exapump is installed
* *WHEN* the user provides `--max-file-size` with values like `500KB`, `1MB`, `2GB`, or `1073741824`
* *THEN* the CLI MUST parse the value correctly using base-10 units (KB=1000, MB=1000000, GB=1000000000)
* *AND* a plain number without suffix MUST be interpreted as bytes

### Scenario: Export empty table to Parquet

* *GIVEN* a table exists but contains no rows
* *WHEN* the user runs `exapump export --table schema.empty_table --output data.parquet --format parquet --dsn <dsn>`
* *THEN* the command MUST write a valid Parquet file containing only the schema (zero rows)
* *AND* stderr MUST print that 0 rows were exported
* *AND* the command MUST exit with code 0

### Scenario: CSV-specific options ignored for Parquet

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.parquet --format parquet --delimiter '\t' --dsn <dsn>`
* *THEN* the command MUST ignore the `--delimiter` option and export valid Parquet
* *AND* the command MUST exit with code 0

### Scenario: Compression option rejected for CSV format

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --compression snappy --dsn <dsn>`
* *THEN* the CLI MUST exit with a non-zero code
* *AND* stderr MUST indicate that `--compression` is only supported for Parquet format

### Scenario: Table not found (Parquet)

* *GIVEN* the specified table does not exist in Exasol
* *WHEN* the user runs `exapump export --table schema.nonexistent --output data.parquet --format parquet --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that the table was not found

### Scenario: Output file not writable (Parquet)

* *GIVEN* the output path is in a directory that does not exist or is not writable
* *WHEN* the user runs `exapump export --table schema.table --output /nonexistent/dir/data.parquet --format parquet --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST contain the output file path
