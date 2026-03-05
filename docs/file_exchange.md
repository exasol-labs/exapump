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
| `--compression` | — | Compression codec for Parquet: `snappy`, `gzip`, `lz4`, `zstd`, `none` |
| `--max-rows-per-file` | — | Maximum rows per output file (enables splitting) |
| `--max-file-size` | — | Maximum file size per output file, e.g. `500KB`, `1MB`, `2GB` (enables splitting) |

### Examples

```bash
# Export a table to CSV
exapump export --table my_schema.events --output events.csv --format csv

# Export a query result to compressed Parquet
exapump export --query 'SELECT * FROM t' --output out.parquet --format parquet --compression zstd

# Split output into files of at most 100MB
exapump export --table my_schema.big_table --output chunks.parquet --format parquet --max-file-size 100MB
```
