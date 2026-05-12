# Changelog

## Unreleased

- `profile add` prompts for the password via a hidden TTY prompt when `--password` is omitted in an interactive shell; non-TTY contexts still fail with a hint to use `profile init` or pass `--password`
- New `profile init` subcommand: guided wizard for cold-start profile creation with optional pre-fill flags (`--name` positional, `--host`, `--port`, `--user`, `--schema`, `--certificate-fingerprint`, `--default`, `--no-bucketfs`); password never accepted on the command line
- New `profile edit` subcommand: interactive editor with current values shown as defaults; password change gated behind a confirm prompt; BucketFS section can be skipped with `--no-bucketfs`
- `profile remove` now asks for confirmation in a TTY before deleting; pass `-y/--yes` to skip (required for scripted use, refuses without it in non-TTY contexts)
- Saved config files are now written with `0600` permissions on unix to protect credentials at rest

## 0.9.0

- Bump exarrow-rs to 0.12.0 (introduces native and websocket Cargo features)
- Add `--transport native|websocket` flag on all connection-bearing subcommands (`upload`, `sql`, `export`, `interactive`)
- Default transport is `native` (aligned with exarrow-rs 0.12 upstream default)

## 0.8.0

- Bump exarrow-rs to 0.8.0 (driver now defaults `tls=true`)
- Profile DSNs omit `tls` and `validateservercertificate` when unset; driver applies secure defaults

## 0.7.5

- Strip SQL line (`--`) and block (`/* ... */`) comments from `sql` command input before execution (#8)

## 0.7.4

- Pin TLS connections to a server certificate via `--certificate-fingerprint` flag and `certificate_fingerprint` profile field (SHA-256 hex of DER)
- Bump exarrow-rs to 0.7.3

## 0.7.2

- Bump exarrow-rs to 0.7.0

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
