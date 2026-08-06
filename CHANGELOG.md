# Changelog

## 0.12.0

- New `--timeout <SECONDS>` option for `exapump export`: sets a client-side deadline on CSV exports; rejected for `--format parquet`
- Bump exarrow-rs to 0.16.0 (`CsvExportOptions::timeout_ms` becomes `Option<u64>` defaulting to `None`)
- Fix CSV exports failing at a fixed, undocumented 300-second client-side bound; exports now run until Exasol finishes the EXPORT statement unless `--timeout` is set (#38)
- Parquet exports lose the same implicit 300-second bound, split or not, and gain no flag to replace it; use `?query_timeout=<seconds>` in the DSN to bound them
- A CSV export that exceeds `--timeout` now deletes the partial output files it wrote and names them on stderr

## 0.11.2

- Bump exarrow-rs to 0.13.0 (TLS connections can now accept a self-signed server certificate via the builder; internal dead-code/over-engineering cleanup).

## 0.11.1

- Bump exarrow-rs to 0.12.8 (multi-row batch prepared-statement execution).

## 0.11.0

- New `exapump wait` subcommand: blocks until an Exasol database is ready, then exits. Readiness is checked in two phases (TCP port open, then a `SELECT 1` succeeds) on a fixed 5s poll interval; `--timeout-secs` sets the deadline (default 1500). Optional `--container` verifies a named Docker container is running before/during polling and dumps its logs on failure. Distinct exit codes for CI branching: 0 ready, 2 timeout, 3 container failure.
- CI and `scripts/integration-test.sh` now use `exapump wait`; removed the ad-hoc `scripts/wait-for-exasol.sh`.

## 0.10.1

- Fix `bucketfs cp` mangling `bfs://` URIs into malformed HTTP URLs: strip the `bfs://<bucket>/` prefix before composing the HTTP request URL so both upload and download accept `bfs://` URIs (#22)
- Fix SQL statement splitter breaking `CREATE … SCRIPT` bodies on internal semicolons: add a `ScriptBody` scanner state that treats the entire script body up to a lone `/` terminator line as a single statement (#23)

## 0.10.0

- `profile add` prompts for the password via a hidden TTY prompt when `--password` is omitted in an interactive shell; non-TTY contexts still fail with a hint to use `profile init` or pass `--password`
- New `profile init` subcommand: guided wizard for cold-start profile creation with optional pre-fill flags (`--name` positional, `--host`, `--port`, `--user`, `--schema`, `--certificate-fingerprint`, `--default`, `--no-bucketfs`); password never accepted on the command line
- New `profile edit` subcommand: interactive editor with current values shown as defaults; password change gated behind a confirm prompt; BucketFS section can be skipped with `--no-bucketfs`
- `profile remove` now asks for confirmation in a TTY before deleting; pass `-y/--yes` to skip (required for scripted use, refuses without it in non-TTY contexts)
- Saved config files now warn on unix when group or other users can access them; permissions are left under user control
- Bump exarrow-rs to 0.12.7 (zero-row result sets now carry their column schema); upgrade arrow/parquet crates to 58

## 0.9.2

- Bump exarrow-rs to 0.12.3: fixes `?` placeholder collision inside SQL literals/identifiers/comments (#17), `WHERE col IN (...)` returning zero rows over native transport (#18), configurable statement timeout (0.12.1), and security patches for rustls-webpki CVE and rand unsoundness (0.12.2)

## 0.9.1

- Fix SQL classification for statements prefixed with `--` or `/* */` comments (issue #14): comments and hints now reach Exasol verbatim; stripping is done internally for statement-type classification only
- Add `EXECUTE SCRIPT` support (issue #16): statements are classified as `Execute` and dispatched via `conn.execute`, branching on `result_set.row_count()` to handle both result-set and no-result-set scripts correctly
- Replace `strip_comments` pre-pass in `sql` and `interactive` with a comment-aware four-state scanner in `split_statements`

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
