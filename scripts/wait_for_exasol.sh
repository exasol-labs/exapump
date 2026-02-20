#!/usr/bin/env bash
set -euo pipefail

CONTAINER_NAME="exasol-test"
MAX_ATTEMPTS=120
SLEEP_INTERVAL=5

# Build the health_check test binary (harness = false)
echo "Building health_check..."
cargo test --test health_check --no-run 2>/dev/null

# Locate the binary in target/debug/deps/
HEALTH_CHECK=$(find target/debug/deps -name 'health_check-*' -type f ! -name '*.d' ! -name '*.o' | head -1)
if [ -z "$HEALTH_CHECK" ]; then
  echo "ERROR: Could not find health_check binary"
  exit 1
fi

echo "Waiting for Exasol to be ready (max $((MAX_ATTEMPTS * SLEEP_INTERVAL))s)..."

for i in $(seq 1 $MAX_ATTEMPTS); do
  # Verify container is still running
  if ! docker inspect -f '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null | grep -q true; then
    echo "ERROR: Container '$CONTAINER_NAME' is not running!"
    echo "--- Last 50 lines of container logs ---"
    docker logs --tail 50 "$CONTAINER_NAME" 2>&1 || true
    exit 1
  fi

  if REQUIRE_EXASOL=1 "$HEALTH_CHECK" 2>/dev/null; then
    echo "Exasol is ready after $((i * SLEEP_INTERVAL))s"
    exit 0
  fi

  echo "  Attempt $i/$MAX_ATTEMPTS — not ready yet, sleeping ${SLEEP_INTERVAL}s..."
  sleep "$SLEEP_INTERVAL"
done

echo "ERROR: Exasol failed to become ready within $((MAX_ATTEMPTS * SLEEP_INTERVAL))s"
echo "--- Last 50 lines of container logs ---"
docker logs --tail 50 "$CONTAINER_NAME" 2>&1 || true
exit 1
