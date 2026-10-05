# Architecture

## Overview

```
args, env, .env
      │
      ▼
┌───────────┐     ┌───────────────────────────────┐     ┌─────────────────────┐
│ main, cli │────▶│ commands (one per subcommand) │────▶│ format, size, split │◀──▶ local files
└───────────┘     └────┬───────────┬─────────┬────┘     └─────────────────────┘
                       │           │         │
                       ▼           ▼         ▼
                ┌────────────┐ ┌────────┐ ┌─────────┐
                │ connection │▶│ config │ │ reqwest │──▶ Exasol BucketFS
                └─────┬──────┘ └───┬────┘ └─────────┘
                      ▼            ▼
                ┌────────────┐  ~/.exapump/config.toml
                │ exarrow-rs │──▶ Exasol database
                └────────────┘
```

- Thin CLI over exarrow-rs. Each subcommand is one module under `src/commands/`. Shared modules resolve connections, read the profile config, detect file formats, and split export output.

## Components

- main (src/main.rs): installs the rustls ring crypto provider, loads `.env`, parses arguments, and dispatches to one subcommand handler | owns: process exit codes for parse errors and a missing subcommand | depends on: cli, upload, export, sql, interactive, profile, bucketfs, wait
- cli (src/cli.rs): defines the clap subcommands and their arguments for upload, sql, export, interactive, bucketfs, and wait | owns: argument definitions, output and compression enums, export timeout bound | depends on: connection, profile
- connection (src/connection.rs): resolves a DSN from `--dsn`/`EXAPUMP_DSN`, `--profile`, or the default profile, appends certificate fingerprint and transport parameters, and opens an exarrow-rs connection | owns: connection flags and DSN precedence | depends on: config, exarrow-rs
- config (src/config.rs): loads and saves connection profiles as TOML and derives DSNs and BucketFS connections from a profile | owns: `~/.exapump/config.toml` and default ports, bucket, and Docker preset | depends on: none
- format (src/format.rs): detects CSV or Parquet from the file extension | owns: supported upload formats | depends on: none
- size (src/size.rs): parses human-readable sizes such as `500KB` for `--max-file-size` | owns: none | depends on: none
- split (src/split.rs): writes a CSV byte stream across numbered files by row count or byte size and names split files `<stem>_NNN.<ext>` | owns: split output files on disk | depends on: none
- upload (src/commands/upload.rs): infers a schema from one CSV or Parquet file, runs `CREATE TABLE IF NOT EXISTS`, and imports the file, or prints the schema and DDL on `--dry-run` | owns: none | depends on: cli, format, connection, exarrow-rs
- export (src/commands/export.rs): exports a table or query to CSV or Parquet with optional compression, client-side timeout, and file splitting | owns: none | depends on: cli, connection, split, size, exarrow-rs
- sql (src/commands/sql.rs): splits SQL from an argument or stdin into statements, runs them in order until the first failure, and renders result sets as CSV or JSON | owns: SQL statement splitter and result rendering | depends on: cli, connection, exarrow-rs
- interactive (src/commands/interactive.rs): runs a line-editor REPL that buffers SQL until `;`, handles `.format`, `.help`, and `.exit`, and renders results as table, CSV, or JSON | owns: `~/.exapump/history` | depends on: cli, sql, connection, exarrow-rs
- profile (src/commands/profile.rs): lists, shows, adds, removes, edits, and initializes connection profiles with interactive prompts | owns: none | depends on: config
- bucketfs (src/commands/bucketfs.rs): lists, copies to or from, and deletes BucketFS files over HTTP(S) with basic auth | owns: none | depends on: config, cli
- wait (src/commands/wait.rs): polls the Exasol TCP port and then `SELECT 1` until success or timeout, and optionally checks a Docker container | owns: wait exit codes | depends on: connection, cli

## Data Flow

- local file -> format -> exarrow-rs schema inference -> upload: a CSV or Parquet file becomes an inferred table schema and a `CREATE TABLE IF NOT EXISTS` statement
- upload -> exarrow-rs import -> Exasol table: file rows load into the target table and the imported row count prints to stdout
- Exasol table or query -> exarrow-rs export -> export -> local file: rows stream as CSV bytes or Arrow record batches and land in one CSV or Parquet file
- exarrow-rs CSV stream -> split -> local files: with `--max-rows-per-file` or `--max-file-size`, rows spread across `<stem>_NNN.<ext>` files, each with its own header unless `--no-header` is set
- argument or stdin -> sql statement splitter -> exarrow-rs execute -> stdout: each statement runs in order, result sets print as CSV or JSON, and status lines and a summary print to stderr
- terminal line -> interactive buffer -> sql execution -> stdout: a statement that ends in `;` runs and prints as a table, CSV, or JSON
- flags, env, `.env`, config.toml -> connection -> exarrow-rs driver: a resolved DSN string configures the exarrow-rs connection that a command opens (`wait` opens a new one per SQL probe)
- local file or BucketFS path -> bucketfs -> HTTP(S) GET, PUT, or DELETE -> BucketFS or local file: file bytes move between disk and the bucket
- resolved DSN -> wait -> TCP probe then `SELECT 1` -> exit code: readiness becomes exit code 0, 1, 2, or 3

## Interfaces

- CLI: `exapump <upload|sql|export|interactive|profile|bucketfs|wait> [args]`, exit code 1 on argument errors, 2 with help text when no subcommand is given
- Connection flags: `--dsn`/`-d` or `EXAPUMP_DSN`, `--profile`/`-p`, `--certificate-fingerprint`, `--transport native|websocket`, applied in that DSN precedence before the default profile
- DSN: `exasol://user:pwd@host:port[/schema]` with optional `tls`, `validateservercertificate`, `certificate_fingerprint`, and `transport=websocket` query parameters
- Config file: TOML at `~/.exapump/config.toml`, or the path in `EXAPUMP_CONFIG`, with one table per named profile holding host, port, user, password, schema, TLS, fingerprint, default flag, and `bfs_*` fields
- `.env` file: read from the working directory at startup and loaded into the process environment
- Upload input: files ending in `.csv` or `.parquet`, CSV options `--delimiter`, `--quote`, `--escape`, `--no-header`, `--null-value`
- Export output: `--format csv|parquet`, `--compression snappy|gzip|lz4|zstd|none` for Parquet, `--timeout` for CSV, split files named `<stem>_NNN.<ext>`
- sql output: result sets on stdout as CSV or JSON, per-statement `[i/n]` status lines and a summary on stderr
- interactive: prompt `exapump> `, dot-commands `.format`, `.help`, `.exit`, history in `~/.exapump/history`
- bucketfs: subcommands `ls`, `cp`, `rm` with `--bfs-*` overrides, HTTP basic auth as user `r` for reads and `w` for writes
- wait: `--container`, `--timeout-secs` (default 1500), exit 0 ready, 1 config error, 2 timeout, 3 container failure

## Constraints

- One binary named `exapump` built from a single Rust 2021 crate
- Linux release binaries build in an AlmaLinux 8 container, and the release job fails when the binary's glibc symbol ceiling exceeds GLIBC_2.28
- Supported release targets are Linux x86_64 and aarch64 and macOS x86_64 and aarch64
- The release also ships a native Windows x86_64 binary as best effort, without tests or official support
- CI check and integration-test jobs pin Rust toolchain 1.92.0
- TLS uses rustls with the ring crypto provider, installed at process start
- Dependency licenses must appear in the `deny.toml` allow list, which `about.toml` mirrors
- Clippy enforces a cognitive complexity threshold of 10
- The config file stores credentials in plain text, and exapump warns on Unix when the file grants group or other access
- `wait` polls every 5 seconds with a 2-second TCP connect timeout
- Export `--timeout` accepts 1 to `u64::MAX / 1000` seconds and applies to CSV only

## External Dependencies

- Exasol database: target of upload, export, sql, interactive, and wait through exarrow-rs on the native or WebSocket transport | failure impact: upload (except `--dry-run`), export, sql, and interactive fail, and wait exits with code 2 after its timeout
- Exasol BucketFS HTTP(S) service: stores and serves bucket files for the bucketfs subcommands, default port 2581 and bucket `default` | failure impact: `bucketfs ls`, `cp`, and `rm` fail, other commands are unaffected
- Docker CLI: lists running containers and dumps container logs for `wait --container` | failure impact: wait reports the container as not running and exits with code 3
- crates.io (build time): supplies exarrow-rs and the other Cargo dependencies | failure impact: the project cannot build
