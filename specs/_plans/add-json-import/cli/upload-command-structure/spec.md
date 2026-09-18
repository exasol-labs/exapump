# Feature: Upload Command Structure

The upload command is the primary entrypoint for data ingestion. It accepts file paths, a target table, and a connection string. All arguments are parsed via clap derive macros.

## Background

The upload command is available as `exapump upload`. Connection arguments are provided via the shared `ConnectionArgs` flattened into `UploadArgs`.

## Scenarios

<!-- DELTA:CHANGED -->
### Scenario: CSV flags ignored for Parquet files

* *GIVEN* a Parquet, JSON, or NDJSON file exists at the specified path
* *WHEN* the user runs `exapump upload <file> --table <table> --delimiter ';'`
* *THEN* the command MUST ignore the `--delimiter` flag
* *AND* the command MUST proceed with the import for the detected format as normal
<!-- /DELTA:CHANGED -->

<!-- DELTA:NEW -->
### Scenario: Upload help describes the table family for JSON input

* *GIVEN* exapump is installed
* *WHEN* the user runs `exapump upload --help`
* *THEN* the `--table` option description MUST state that JSON input creates a root table plus one subtable per nested path
<!-- /DELTA:NEW -->
