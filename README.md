<div align="center">

![exapump logo](assets/exapump-logo.svg)

# exapump

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/exasol-labs/exapump/actions/workflows/ci.yml/badge.svg)](https://github.com/exasol-labs/exapump/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)


Single-binary CLI for Exasol data exchange — import, export, and SQL in one command.

</div>

---

## Install

```bash
cargo install exapump
```

Or build from source:

```bash
cargo build --release
```

## Quick Start

**Import** CSV or Parquet files into an Exasol table:

```bash
exapump upload data.csv --table schema.my_table --dsn exasol://user:pwd@host:8563
```

**Export** a table or query result to CSV:

```bash
exapump export --table schema.my_table --output data.csv --format csv --dsn exasol://user:pwd@host:8563
exapump export --query 'SELECT * FROM t WHERE id > 100' --output results.csv --format csv --dsn exasol://user:pwd@host:8563
```

**Run SQL** statements directly:

```bash
exapump sql 'SELECT count(*) FROM my_table' --dsn exasol://user:pwd@host:8563
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

## License

Community-supported. Licensed under [MIT](LICENSE).

---

<div align="center">

Built with Rust 🦀 and made with ❤️ as part of [Exasol Labs 🧪](https://github.com/exasol-labs/).

</div>
