#!/usr/bin/env bash
set -euo pipefail

MAX_ATTEMPTS="${WAIT_MAX_ATTEMPTS:-120}"
SLEEP_INTERVAL="${WAIT_SLEEP_INTERVAL:-5}"

echo "Building health_check binary..."
cargo test --test health_check --no-run 2>&1

HEALTH_CHECK=$(find target/debug/deps -name 'health_check-*' -type f \
    ! -name '*.d' ! -name '*.o' | head -1)
if [ -z "$HEALTH_CHECK" ]; then
    echo "ERROR: Could not find health_check binary"
    exit 1
fi

echo "Waiting for Exasol (max $((MAX_ATTEMPTS * SLEEP_INTERVAL))s)..."

for i in $(seq 1 "$MAX_ATTEMPTS"); do
    if REQUIRE_EXASOL=1 "$HEALTH_CHECK" 2>/dev/null; then
        echo "Exasol is ready after ~$((i * SLEEP_INTERVAL))s"
        exit 0
    fi
    echo "  Attempt $i/$MAX_ATTEMPTS — not ready, sleeping ${SLEEP_INTERVAL}s..."
    sleep "$SLEEP_INTERVAL"
done

echo "ERROR: Exasol did not become ready within $((MAX_ATTEMPTS * SLEEP_INTERVAL))s"
exit 1
