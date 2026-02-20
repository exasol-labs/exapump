<div align="center">

![exapump logo](assets/exapump-logo.svg)

# exapump

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/exasol-labs/exapump/actions/workflows/ci.yml/badge.svg)](https://github.com/exasol-labs/exapump/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)


Single-binary CLI for Exasol data exchange — import, export, and SQL in one command.

Based on [exarrow-rs](https://github.com/exasol-labs/exarrow-rs) — an ADBC driver for Exasol written in Rust.

</div>

---

## Install

Get started right away:

```bash
curl -fsSL https://raw.githubusercontent.com/exasol-labs/exapump/main/install.sh | sh
```

Windows users: grab the `.exe` from the [latest release](https://github.com/exasol-labs/exapump/releases/latest).

## Quick Start

**Import** CSV or Parquet files into an Exasol table:

```bash
exapump upload data.csv \
  --table schema.my_table \
  --dsn exasol://user:pwd@host:8563

exapump upload data.parquet \
  --table schema.my_table \
  --dsn exasol://user:pwd@host:8563
```

**Export** a table or query result to CSV:

```bash
exapump export \
  --table schema.my_table \
  --output data.csv \
  --format csv \
  --dsn exasol://user:pwd@host:8563

exapump export \
  --query 'SELECT * FROM t WHERE id > 100' \
  --output results.csv \
  --format csv \
  --dsn exasol://user:pwd@host:8563
```

**Run SQL** statements directly:

```bash
exapump sql 'SELECT count(*) FROM my_table' \
  --dsn exasol://user:pwd@host:8563
```

Connection details can also be provided via environment variables (`EXAPUMP_DSN`) or a `.env` file.

---

## Commands

| Command | Description |
|---------|-------------|
| `upload` | Upload CSV or Parquet files to an Exasol table (auto-creates table if needed) |
| `export` | Export an Exasol table or query result to a CSV file |
| `sql` | Execute SQL statements and print results (CSV or JSON) |

Run `exapump <command> --help` for full argument details.

---

## Build from Source

```bash
git clone https://github.com/exasol-labs/exapump.git
cd exapump
cargo build --release
```

The binary will be at `target/release/exapump`.

---

Install a specific version:

```bash
EXAPUMP_VERSION=0.3.0 \
  curl -fsSL https://raw.githubusercontent.com/exasol-labs/exapump/main/install.sh | sh
```

---

## License

Community-supported. Licensed under [MIT](LICENSE).

---

<div align="center">

Built with Rust 🦀 and made with ❤️ as part of [Exasol Labs 🧪](https://github.com/exasol-labs/).

</div>
