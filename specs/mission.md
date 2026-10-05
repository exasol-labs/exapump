# Mission: exapump

> Single-binary CLI for Exasol data exchange — import, export, and SQL in one command.

## Problem Statement

Working with data in Exasol today involves too much friction for common operations:

**Importing** data files requires writing custom code or setting up ETL tools. A customer with Parquet files from a Spark job, CSV exports from another system, or data lake downloads must either:

1. Write a Python/Java script using pyexasol or JDBC — handling connection setup, schema creation, format parsing, error handling, and progress tracking themselves
2. Deploy an ETL tool (Informatica, Talend, Airflow) — massive overhead for what is fundamentally "put this file in that table"
3. Use Exasol's IMPORT statement directly — which requires files accessible via HTTP/FTP/BucketFS, not local disk

**Exporting** data from Exasol to local files has the same friction in reverse. Getting query results or table data into a local CSV or Parquet file means writing a script with pyexasol/JDBC, handling serialization, and managing output formats manually.

**Running SQL** — even a single CREATE TABLE or quick SELECT — requires a full JDBC/ODBC client, a Python script, or Exasol's browser-based UI. There is no lightweight command-line option for ad-hoc statements.

There is no simple `command -> done` workflow for these common cases. Every option involves too much friction.

## Target Users

| Persona | Goal | Key Workflow |
|---------|------|--------------|
| Data Engineer | Quickly ingest files from Spark jobs, data lakes, or pipeline outputs into Exasol; export query results to Parquet for downstream processing | `exapump upload data.parquet --table schema.table` after a batch job completes; `exapump export --query 'SELECT ...' --output results.parquet --format parquet` for pipeline handoff |
| DBA / Analyst | Ad-hoc data loading, exports, and SQL without writing code or deploying tools | `exapump upload export.csv --table staging.imports --dry-run` to preview, then load; `exapump sql 'SELECT count(*) FROM t' --dsn ...` for quick checks |

## Core Capabilities

1. **Single-command upload** — load a CSV or Parquet file into an Exasol table with one command
2. **Auto table creation** — infer schema from file metadata/sampling and create the target table if it doesn't exist
3. **Dry-run mode** — preview the inferred schema and planned CREATE TABLE without executing
4. **Single-command export** — export a table or SQL query result to a local CSV or Parquet file, optionally split by row count or file size, with a timeout for CSV export
5. **SQL execution** — run one or more `;`-separated SQL statements (DDL/DML/query), given as an argument or on stdin, and get results as CSV or JSON
6. **BucketFS operations** — upload, download, list, and delete files in Exasol's BucketFS. `cp` accepts `bfs://` or `bfss://` URIs (`bfss://` implies TLS), while `ls` and `rm` take plain paths. Connection settings resolve from flags, environment, or a profile, and the read and write passwords fall back in the order read, write, anonymous
7. **Profile-based connection config** — named connection profiles (`profile list/add/show`) resolved alongside `--dsn`/`EXAPUMP_DSN`, with BucketFS host, bucket, and TLS fields, Docker presets for `profile add`, a warning when the config file permissions are too broad, certificate fingerprint pinning, `.env` loading (priority: flag > shell env > `.env` > profile), and a `--transport native|websocket` choice
8. **Contextual SQL errors** — SQL failures print an error with a hint, such as a syntax pointer, a missing object, or missing privileges
9. **Interactive SQL shell** — a REPL (`exapump interactive`) with dot-commands, multi-statement script execution, and table-formatted output
10. **Readiness polling** — `exapump wait` blocks until the target Exasol instance accepts a TCP connection and answers `SELECT 1`, for CI/E2E setup. A named Docker container is only a liveness guard: `wait` fails fast with exit code 3 when it stops running

## Out of Scope

- ETL orchestration, scheduling, or workflow management
- Data transformations, filtering, or column mapping
- GUI or web interface
- Database-to-database replication
- Streaming/real-time ingestion (exapump is batch-oriented)

Architecture: see specs/architecture.md.

## Domain Glossary

Standard Exasol and Arrow terminology applies. No project-specific redefinitions.

| Term | Definition |
|------|------------|
| DSN | Data Source Name — connection string in the format `exasol://user:pwd@host:port` |
| Schema inference | Detecting column names, types, and nullability from file metadata (Parquet) or row sampling (CSV) |
| exarrow-rs | The underlying Rust library providing Arrow-native Exasol connectivity, schema inference, type mapping, and SQL execution |
| BucketFS | Exasol's built-in distributed file storage, used for staging files (e.g. Script Language Containers) accessible to the database |
| Profile | A named connection configuration (host, user, credentials, TLS settings) stored in the exapump config file |

---

## Tech Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Language | Rust | Systems language for single-binary distribution |
| CLI framework | clap (derive) | Argument parsing and help generation |
| Core library | exarrow-rs (crates.io) | Exasol connectivity, Arrow-native import/export, schema inference, SQL execution |
| Testing | cargo test | Built-in unit and integration tests |

## Commands

```bash
# Build
cargo build

# Build release
cargo build --release

# Test
cargo test

# Lint & Format
cargo clippy && cargo fmt --check

# Format
cargo fmt
```

## Project Structure

```
exapump/
├── src/
│   ├── main.rs              # Entry point, CLI dispatch
│   ├── cli.rs               # Argument definitions, subcommands
│   ├── format.rs            # File format detection
│   └── commands/
│       ├── mod.rs
│       ├── upload.rs         # Import command
│       ├── export.rs         # Export command
│       ├── sql.rs            # SQL command
│       ├── bucketfs.rs       # BucketFS upload/list/delete
│       ├── interactive.rs    # Interactive SQL REPL
│       ├── profile.rs        # Connection profile management
│       └── wait.rs           # Readiness polling (exapump wait)
├── tests/              # Integration tests
├── specs/              # Feature specifications
├── Cargo.toml          # Dependencies and metadata
└── .gitignore
```
