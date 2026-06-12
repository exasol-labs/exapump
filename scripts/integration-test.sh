#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

export EXASOL_HOST="${EXASOL_HOST:-localhost}"
export EXASOL_PORT="${EXASOL_PORT:-8563}"
export EXASOL_USER="${EXASOL_USER:-sys}"
export EXASOL_PASSWORD="${EXASOL_PASSWORD:-exasol}"
export REQUIRE_EXASOL="${REQUIRE_EXASOL:-1}"

echo "=== Waiting for Exasol ==="
cargo run --bin exapump -- wait \
    --container exasol-test \
    --timeout-secs 1500 \
    --dsn "exasol://sys:exasol@localhost:8563?tls=true&validateservercertificate=0"

echo "=== Running all tests ==="
cargo test --verbose

echo "=== All integration tests passed ==="
