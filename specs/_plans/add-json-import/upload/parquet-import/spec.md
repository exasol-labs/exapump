# Feature: Parquet Import

Upload a single Parquet file into an Exasol table, with automatic table creation from Parquet metadata and dry-run schema preview.

## Background

exapump connects to Exasol via exarrow-rs using the DSN provided by `--dsn` or `EXAPUMP_DSN`. File format is detected from the file extension. The upload command is async.

## Scenarios

<!-- DELTA:CHANGED -->
### Scenario: Unsupported file extension

* *GIVEN* a file exists with an unrecognized extension (not `.parquet`, `.csv`, `.json`, or `.ndjson`)
* *WHEN* the user runs `exapump upload data.txt --table schema.table --dsn <dsn>`
* *THEN* the command MUST exit with a non-zero code
* *AND* stderr MUST indicate that the file format is not supported
* *AND* stderr SHOULD list the supported formats
<!-- /DELTA:CHANGED -->
