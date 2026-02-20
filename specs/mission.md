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
| Data Engineer | Quickly ingest files from Spark jobs, data lakes, or pipeline outputs into Exasol; export query results to Parquet for downstream processing | `exapump upload *.parquet --table schema.table` after a batch job completes; `exapump export --query 'SELECT ...' --output results.parquet --format parquet` for pipeline handoff |
| DBA / Analyst | Ad-hoc data loading, exports, and SQL without writing code or deploying tools | `exapump upload export.csv --table staging.imports --dry-run` to preview, then load; `exapump sql 'SELECT count(*) FROM t' --dsn ...` for quick checks |

## Core Capabilities

1. **Single-command upload** — load CSV or Parquet files into an Exasol table with one command
2. **Auto table creation** — infer schema from file metadata/sampling and create the target table if it doesn't exist
3. **Glob and directory support** — load all matching files from a path pattern in a single invocation
4. **Parallel multi-file import** — leverage exarrow-rs parallel connections for high-throughput transfer
5. **Dry-run mode** — preview the inferred schema and planned CREATE TABLE without executing
6. **Single-command export** — export a table or SQL query result to a local CSV or Parquet file
7. **SQL execution** — run a SQL statement and get results as CSV or JSON (not a REPL)

## Out of Scope

- ETL orchestration, scheduling, or workflow management
- Data transformations, filtering, or column mapping
- GUI or web interface
- Database-to-database replication
- Streaming/real-time ingestion (exapump is batch-oriented)
- Interactive REPL or shell (SQL is single-statement, not interactive)

## Domain Glossary

Standard Exasol and Arrow terminology applies. No project-specific redefinitions.

| Term | Definition |
|------|------------|
| DSN | Data Source Name — connection string in the format `exasol://user:pwd@host:port` |
| Schema inference | Detecting column names, types, and nullability from file metadata (Parquet) or row sampling (CSV) |
| exarrow-rs | The underlying Rust library providing Arrow-native Exasol connectivity, schema inference, type mapping, and parallel transfer |

---

## Tech Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Language | Rust | Systems language for single-binary distribution |
| CLI framework | clap (derive) | Argument parsing and help generation |
| Core library | exarrow-rs (crates.io) | Exasol connectivity, Arrow-native import/export, schema inference, parallel transfer, SQL execution |
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
│       ├── export.rs         # Export command (planned)
│       └── sql.rs            # SQL command (planned)
├── tests/              # Integration tests
├── specs/              # Feature specifications
├── Cargo.toml          # Dependencies and metadata
└── .gitignore
```

## Architecture

**Thin CLI wrapper** over exarrow-rs. exapump owns only CLI-specific concerns:

```
User → [CLI (clap)] → [Orchestration] → [exarrow-rs: connect, infer, import/export/query]
```

- **CLI layer**: Argument parsing, validation, environment variable resolution
- **Orchestration**: File glob expansion, format detection, progress display, error reporting
- **exarrow-rs**: Connection management, schema inference, type mapping, parallel data transfer, SQL execution

exapump contains minimal business logic — it translates CLI intent into exarrow-rs API calls and presents results to the user.

### Export Command

```bash
exapump export --table schema.table --output data.csv --format csv --dsn ...
exapump export --query 'SELECT * FROM t WHERE ...' --output results.parquet --format parquet --dsn ...
```

- Source: `--table` or `--query` (mutually exclusive)
- Output: `--output <file>` (required)
- Format: `--format csv|parquet` (required, explicit)
- DSN: same `--dsn` / `EXAPUMP_DSN` pattern as upload

### SQL Command

```bash
exapump sql 'CREATE TABLE t(id INT)' --dsn ...
exapump sql 'SELECT * FROM t' --dsn ... --format csv
exapump sql 'SELECT * FROM t' --dsn ... --format json
```

- SQL as positional argument
- Output format: `--format csv|json` (default: csv)
- DDL/DML: prints affected row count or "OK"
- SELECT: streams result set to stdout in chosen format
- Not a REPL: one statement per invocation

## Constraints

- **Distribution**: Single static binary. No runtime dependencies beyond libc.
- **Platforms**: Linux (x86_64, aarch64), macOS (x86_64, aarch64). Windows via WSL only.
- **Performance**: Throughput bounded by exarrow-rs and network. exapump itself must add negligible overhead.

## External Dependencies

| Service | Purpose | Failure Impact |
|---------|---------|----------------|
| Exasol database | Target for data import/export and SQL execution | Cannot function — all operations require a live Exasol connection |
| crates.io (build-time) | Fetch exarrow-rs and other dependencies | Cannot build — resolved at compile time only |
