<div align="center">

![exapump logo](assets/exapump-logo.svg)

# exapump

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/exasol-labs/exapump/actions/workflows/ci.yml/badge.svg)](https://github.com/exasol-labs/exapump/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![spec|driven](https://img.shields.io/badge/spec-driven-blue)](./specs/)


Single-binary CLI for Exasol data exchange — import, export, and SQL in one command.

<img src="assets/demo.gif" width="720" alt="exapump demo">

Based on [exarrow-rs](https://github.com/exasol-labs/exarrow-rs) — an ADBC driver for Exasol written in Rust.

</div>

---

## Quick Start

**1. Install**

```bash
curl -fsSL https://raw.githubusercontent.com/exasol-labs/exapump/main/install.sh | sh
```

Windows users: grab the `.exe` from the [latest release](https://github.com/exasol-labs/exapump/releases/latest).

**2. Set up a connection**

```bash
exapump profile add default
```

**3. Run a command**

```bash
exapump sql 'SELECT 1'
exapump upload data.csv --table schema.my_table
exapump export --table schema.my_table --output data.csv --format csv
exapump interactive
exapump bucketfs ls
```

No `--dsn` needed — exapump uses your default profile automatically.
To use a specific profile, pass `--profile` (or `-p`):

```bash
exapump sql -p production 'SELECT 1'
```

---

## User Guide

Full documentation is available in the [docs/](docs/index.md) directory.

| Command | Description | Docs |
|---------|-------------|------|
| `upload` | Upload CSV or Parquet files to an Exasol table | [File Exchange](docs/file_exchange.md) |
| `export` | Export a table or query result to CSV or Parquet | [File Exchange](docs/file_exchange.md) |
| `sql` | Execute SQL statements and print results | [SQL Interaction](docs/sql_interaction.md) |
| `interactive` | Start an interactive SQL session | [SQL Interaction](docs/sql_interaction.md) |
| `profile` | Manage connection profiles | [Configuration](docs/configuration.md) |
| `bucketfs` | Manage files in BucketFS (list, copy, delete) | [BucketFS](docs/bucketfs.md) |

Run `exapump <command> --help` for full argument details.

---

Want to build from source? See [Build from Source](docs/build_from_source.md).

---

## License

Community-supported. Licensed under [MIT](LICENSE).

---

<div align="center">

Built with Rust 🦀 and made with ❤️ as part of [Exasol Labs 🧪](https://github.com/exasol-labs/).

</div>
