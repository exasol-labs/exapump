# Changelog

## 0.6.2

- Bump exarrow-rs to 0.6.3

## 0.6.1

- Auto-set `default = true` when adding the first profile to an empty config
- Show "(set as default)" message to inform user of auto-defaulting

## 0.5.0

- Connection profiles via `~/.exapump/config.toml`
- `exapump profile add|list|show|remove` commands
- `--profile` / `-p` flag on all connection commands
- Docker presets via `exapump profile add default`

## 0.4.1

- Bump exarrow-rs to 0.6.1 (fix hanging on missing schema)
- Fix health check to use `query` instead of `execute_update`
- Fix CI container startup timing and wait diagnostics

## 0.4.0

- Interactive SQL REPL with readline history and table/CSV/JSON output
- Parquet export with compression (snappy, gzip, lz4, zstd)
- File splitting for exports (by row count or file size)
- Portable CI scripts for local and pipeline testing

## 0.3.0

- Install script with `curl` one-liner
- Cross-platform binary builds (Linux x86/arm64, macOS x86/arm64, Windows)
- CI release workflow with GitHub Actions
- Export command for tables and query results to CSV
- SQL command with `.env` support and CSV/JSON output
- CSV file upload with auto table creation
- Parquet file upload
