# Mission: exapump

> The simplest path from file to Exasol table — a single-binary CLI for CSV and Parquet ingest.

## Problem Statement

Loading data files into Exasol today requires writing custom code or setting up ETL tools. A customer with Parquet files from a Spark job, CSV exports from another system, or data lake downloads must either:

1. Write a Python/Java script using pyexasol or JDBC — handling connection setup, schema creation, format parsing, error handling, and progress tracking themselves
2. Deploy an ETL tool (Informatica, Talend, Airflow) — massive overhead for what is fundamentally "put this file in that table"
3. Use Exasol's IMPORT statement directly — which requires files accessible via HTTP/FTP/BucketFS, not local disk

There is no simple `command -> data in Exasol` workflow. For the common case of "I have files on disk and I want them in Exasol," every option involves too much friction.

## Target Users

| Persona | Goal | Key Workflow |
|---------|------|--------------|
| Data Engineer | Quickly ingest files from Spark jobs, data lakes, or pipeline outputs into Exasol | `exapump upload *.parquet --table schema.table` after a batch job completes |
| DBA / Analyst | Ad-hoc data loading without writing code or deploying tools | `exapump upload export.csv --table staging.imports --dry-run` to preview, then load |

## Core Capabilities

1. **Single-command upload** — load CSV or Parquet files into an Exasol table with one command
2. **Auto table creation** — infer schema from file metadata/sampling and create the target table if it doesn't exist
3. **Glob and directory support** — load all matching files from a path pattern in a single invocation
4. **Parallel multi-file import** — leverage exarrow-rs parallel connections for high-throughput transfer
5. **Dry-run mode** — preview the inferred schema and planned CREATE TABLE without executing

## Out of Scope

- ETL orchestration, scheduling, or workflow management
- Data transformations, filtering, or column mapping
- GUI or web interface
- Database-to-database replication
- Streaming/real-time ingestion (exapump is batch-oriented)
- Export/download functionality (deferred to v2)

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
| Core library | exarrow-rs (crates.io) | Exasol connectivity, Arrow-native import, schema inference, parallel transfer |
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
│   ├── main.rs         # Entry point, CLI dispatch
│   └── cli/            # Argument definitions, subcommands
├── tests/              # Integration tests
├── specs/              # Feature specifications
├── Cargo.toml          # Dependencies and metadata
└── .gitignore
```

## Architecture

**Thin CLI wrapper** over exarrow-rs. exapump owns only CLI-specific concerns:

```
User → [CLI (clap)] → [Orchestration: file discovery, progress] → [exarrow-rs: connect, infer, import]
```

- **CLI layer**: Argument parsing, validation, environment variable resolution
- **Orchestration**: File glob expansion, format detection, progress display, error reporting
- **exarrow-rs**: Connection management, schema inference, type mapping, parallel data transfer

exapump contains minimal business logic — it translates CLI intent into exarrow-rs API calls and presents results to the user.

## Constraints

- **Distribution**: Single static binary. No runtime dependencies beyond libc.
- **Platforms**: Linux (x86_64, aarch64), macOS (x86_64, aarch64). Windows via WSL only.
- **Performance**: Throughput bounded by exarrow-rs and network. exapump itself must add negligible overhead.

## External Dependencies

| Service | Purpose | Failure Impact |
|---------|---------|----------------|
| Exasol database | Target for data loading | Cannot function — all operations require a live Exasol connection |
| crates.io (build-time) | Fetch exarrow-rs and other dependencies | Cannot build — resolved at compile time only |
