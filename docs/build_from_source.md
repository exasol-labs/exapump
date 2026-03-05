# Build from Source

```bash
git clone https://github.com/exasol-labs/exapump.git
cd exapump
cargo build --release
```

The binary will be at `target/release/exapump`.

## Install a Specific Version

```bash
EXAPUMP_VERSION=0.3.0 \
  curl -fsSL https://raw.githubusercontent.com/exasol-labs/exapump/main/install.sh | sh
```
