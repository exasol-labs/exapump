# Feature: CSV Export Splitting

Split a CSV export into multiple files based on row-count or file-size thresholds, while preserving the formatting options of `export/csv-export`.

## Background

When split options (`--max-rows-per-file` or `--max-file-size`) are provided, the export writes to multiple files using a splitting writer that preserves Exasol-side CSV formatting. Split files are named `<stem>_000.csv`, `<stem>_001.csv`, etc. Each split file includes the header row unless `--no-header` is set. If splitting is requested but only one file is produced, the output uses the original `--output` name without a suffix.

## Scenarios

### Scenario: Split CSV by max rows per file

* *GIVEN* a table with 10 rows exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-rows-per-file 3 --dsn <dsn>`
* *THEN* the command MUST produce 4 files: `data_000.csv`, `data_001.csv`, `data_002.csv`, `data_003.csv`
* *AND* each split file MUST include the header row
* *AND* stderr MUST print the total number of rows exported and the number of files written
* *AND* the command MUST exit with code 0

### Scenario: Split CSV by max file size

* *GIVEN* a table with enough data to exceed the size threshold
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-file-size 1MB --dsn <dsn>`
* *THEN* the command MUST split output into multiple files where each file's size SHOULD NOT exceed the threshold
* *AND* files MUST be named `data_000.csv`, `data_001.csv`, etc.
* *AND* each split file MUST include the header row
* *AND* stderr MUST print the total number of rows exported and the number of files written

### Scenario: Split CSV produces single file

* *GIVEN* a table with 5 rows exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-rows-per-file 1000 --dsn <dsn>`
* *THEN* the command MUST write the output to `data.csv` (the original `--output` name, no numeric suffix)
* *AND* the command MUST exit with code 0

### Scenario: Split CSV with no-header

* *GIVEN* a table with 6 rows exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-rows-per-file 3 --no-header --dsn <dsn>`
* *THEN* each split file MUST NOT contain a header row

### Scenario: Split CSV preserves formatting options

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-rows-per-file 3 --delimiter '\t' --dsn <dsn>`
* *THEN* each split file MUST use tab as the field separator

### Scenario: Both CSV split thresholds active

* *GIVEN* a table with data exists in Exasol
* *WHEN* the user runs `exapump export --table schema.table --output data.csv --format csv --max-rows-per-file 100000 --max-file-size 50MB --dsn <dsn>`
* *THEN* the command MUST split into a new file whenever either threshold is reached first
