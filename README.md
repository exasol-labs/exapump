<div align="center">

![exapump logo](assets/exapump-logo.svg)

# exapump

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/exasol-labs/exapump/actions/workflows/ci.yml/badge.svg)](https://github.com/exasol-labs/exapump/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)


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

For a local Docker container (default presets):

```bash
exapump profile add local
```

For a custom server (use `--default` to mark it as the default profile):

```bash
exapump profile add mydb \
  --host exasol-prod.example.com \
  --user admin \
  --password s3cret \
  --schema my_schema \
  --default
```

**3. Run a command**

```bash
exapump sql 'SELECT 1'
exapump upload data.csv --table schema.my_table
exapump export --table schema.my_table --output data.csv --format csv
exapump interactive
```

No `--dsn` needed — exapump uses your default profile automatically.

---

## Connection

exapump resolves connections in this order:

- **Profile** (recommended): `exapump profile add local`, then run commands directly
- **DSN flag**: `--dsn exasol://user:pwd@host:8563`
- **Environment variable**: `EXAPUMP_DSN=exasol://user:pwd@host:8563`

See [docs/configuration.md](docs/configuration.md) for full details.

---

## Commands

| Command | Description |
|---------|-------------|
| `upload` | Upload CSV or Parquet files to an Exasol table (auto-creates table if needed) |
| `export` | Export an Exasol table or query result to a CSV or Parquet file |
| `sql` | Execute SQL statements and print results (CSV or JSON) |
| `interactive` | Start an interactive SQL session with table/CSV/JSON output |
| `profile` | Manage connection profiles (add, list, show, remove) |

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
