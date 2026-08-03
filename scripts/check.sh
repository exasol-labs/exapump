#!/usr/bin/env bash
set -euo pipefail

echo "=== Formatting ==="
cargo fmt --all -- --check

echo "=== Clippy ==="
cargo clippy --all-targets --all-features -- -D warnings

echo "=== License and advisory check ==="
if command -v cargo-deny &>/dev/null; then
    cargo deny check licenses
    cargo deny check advisories
else
    echo "WARN: cargo-deny not installed, skipping license/advisory check"
    echo "      Install with: cargo install cargo-deny"
fi

echo "=== Unit tests ==="
cargo test --bin exapump --verbose

echo "=== All checks passed ==="
