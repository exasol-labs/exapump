# File Exchange

## Upload

Upload CSV or Parquet files to an Exasol table. If the table does not exist, exapump auto-creates it by inferring the schema from the file.

```bash
exapump upload data.csv --table schema.my_table
exapump upload *.parquet --table schema.my_table
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--table` | *required* | Target table name (e.g. `schema.table`) |
| `--dry-run` | — | Preview inferred schema without loading data |
| `--delimiter` | `,` | CSV field delimiter |
| `--quote` | `"` | CSV quoting character |
| `--escape` | — | CSV escape character |
| `--no-header` | — | Treat the first row as data, not a header |
| `--null-value` | `""` | String to interpret as NULL |

### Examples

```bash
# Upload a CSV with a custom delimiter
exapump upload data.tsv --table my_schema.events --delimiter $'\t'

# Upload Parquet files
exapump upload part-*.parquet --table my_schema.events

# Dry run — preview the inferred schema
exapump upload data.csv --table my_schema.events --dry-run
```

---

## Export

Export an Exasol table or query result to a CSV or Parquet file.

```bash
exapump export --table schema.my_table --output data.csv --format csv
exapump export --query 'SELECT * FROM t WHERE id > 100' --output result.parquet --format parquet
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--table` | — | Table to export (mutually exclusive with `--query`) |
| `--query` | — | SQL query to export results from (mutually exclusive with `--table`) |
| `--output` | *required* | Output file path |
| `--format` | *required* | Export format: `csv` or `parquet` |
| `--delimiter` | `,` | CSV field delimiter |
| `--quote` | `"` | CSV quoting character |
| `--no-header` | — | Exclude header row from output |
| `--null-value` | `""` | String to represent NULL values |
| `--timeout` | — | Client-side export deadline in seconds; CSV only |
| `--compression` | — | Compression codec for Parquet: `snappy`, `gzip`, `lz4`, `zstd`, `none` |
| `--max-rows-per-file` | — | Maximum rows per output file (enables splitting) |
| `--max-file-size` | — | Maximum file size per output file, e.g. `500KB`, `1MB`, `2GB` (enables splitting) |

Three different timeouts can bound a CSV export, and only one of them is the flag above. `--timeout` sets a client-side export deadline, in seconds, for CSV exports only. `?query_timeout=<seconds>` in the DSN sets a server-enforced query bound that applies to every format. `?timeout=<seconds>` in the DSN sets only the connection's connect deadline; it does not bound an export. If `--timeout` elapses, exapump deletes the output files the export had written. On a split CSV export (`--max-rows-per-file` or `--max-file-size`), `--timeout` bounds only the download phase; the file-writing phase that follows runs unbounded. The single-file path stays bounded through both phases.

A Parquet export has no client-side deadline and cannot take one; `--timeout` is rejected for `--format parquet`. exapump versions before 0.12.0 bounded it implicitly at 300 seconds, whether or not it was split. `?query_timeout=<seconds>` in the DSN is the only bound available for a Parquet export.

### Examples

```bash
# Export a table to CSV
exapump export --table my_schema.events --output events.csv --format csv

# Export a CSV with a 300-second client-side deadline
exapump export --table my_schema.events --output events.csv --format csv --timeout 300

# Export a query result to compressed Parquet
exapump export --query 'SELECT * FROM t' --output out.parquet --format parquet --compression zstd

# Split output into files of at most 100MB
exapump export --table my_schema.big_table --output chunks.parquet --format parquet --max-file-size 100MB
```
