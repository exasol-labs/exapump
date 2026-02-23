#!/usr/bin/env bash
set -euo pipefail

echo "=== Formatting ==="
cargo fmt --all -- --check

echo "=== Clippy ==="
cargo clippy --all-targets -- -D warnings

echo "=== License check ==="
if command -v cargo-deny &>/dev/null; then
    cargo deny check licenses
else
    echo "WARN: cargo-deny not installed, skipping license check"
    echo "      Install with: cargo install cargo-deny"
fi

echo "=== Unit tests ==="
cargo test --bin exapump --verbose

echo "=== All checks passed ==="
