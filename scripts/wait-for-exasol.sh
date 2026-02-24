#!/usr/bin/env bash
set -e

CONTAINER_NAME="${1:-exasol-test}"
MAX_WAIT_SECS="${2:-1500}"
SLEEP_INTERVAL=5
DIAG_INTERVAL=30

HOST="${EXASOL_HOST:-localhost}"
PORT="${EXASOL_PORT:-8563}"

# Verify container exists and is running
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
  echo "ERROR: Container '$CONTAINER_NAME' is not running"
  exit 1
fi

# Locate or build the health check binary
HEALTH_CHECK=""
for candidate in \
    target/release/examples/health_check \
    target/debug/examples/health_check; do
  if [ -x "$candidate" ]; then
    HEALTH_CHECK="$candidate"
    break
  fi
done

if [ -z "$HEALTH_CHECK" ]; then
  echo "Health check binary not found, building..."
  cargo build --example health_check
  HEALTH_CHECK="target/debug/examples/health_check"
fi

echo "Using health check: $HEALTH_CHECK"
echo "Waiting for Exasol container '$CONTAINER_NAME' (max ${MAX_WAIT_SECS}s)..."
echo ""

START_TIME=$(date +%s)
LAST_DIAG=0

elapsed() {
  echo $(( $(date +%s) - START_TIME ))
}

check_timeout() {
  if [ "$(elapsed)" -ge "$MAX_WAIT_SECS" ]; then
    echo ""
    echo "ERROR: Timed out after $(elapsed)s waiting for Exasol"
    docker logs "$CONTAINER_NAME" 2>&1 | tail -50
    exit 1
  fi
}

check_container() {
  if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo ""
    echo "ERROR: Container '$CONTAINER_NAME' stopped unexpectedly"
    docker logs "$CONTAINER_NAME" 2>&1 | tail -30
    exit 1
  fi
}

# --- Phase 1: TCP port check ---
echo "--- Phase 1: Waiting for TCP port ${HOST}:${PORT} ---"
PHASE1_START=$(date +%s)

while true; do
  check_timeout
  check_container

  if (echo > "/dev/tcp/${HOST}/${PORT}") 2>/dev/null; then
    PHASE1_ELAPSED=$(( $(date +%s) - PHASE1_START ))
    echo "TCP port ${HOST}:${PORT} is open (after ${PHASE1_ELAPSED}s)"
    echo ""
    break
  fi

  SECS=$(elapsed)
  if [ $(( SECS - LAST_DIAG )) -ge "$DIAG_INTERVAL" ]; then
    echo "  Still waiting for TCP port... (${SECS}s elapsed)"
    LAST_DIAG=$SECS
  fi

  sleep "$SLEEP_INTERVAL"
done

# --- Phase 2: SQL health check ---
echo "--- Phase 2: Waiting for SQL health check (SELECT 1) ---"
PHASE2_START=$(date +%s)
PHASE2_ATTEMPT=0
LAST_ERROR=""
LAST_DIAG=$(elapsed)

while true; do
  check_timeout
  check_container

  PHASE2_ATTEMPT=$(( PHASE2_ATTEMPT + 1 ))

  # Run health check, capturing stderr (diagnostics) while suppressing stdout
  if HC_ERR=$(REQUIRE_EXASOL=1 "$HEALTH_CHECK" 2>&1 >/dev/null); then
    TOTAL_SECS=$(elapsed)
    echo "Exasol is ready! (phase2 attempt ${PHASE2_ATTEMPT}, total ${TOTAL_SECS}s)"
    exit 0
  else
    LAST_ERROR="$HC_ERR"
  fi

  SECS=$(elapsed)
  if [ $(( SECS - LAST_DIAG )) -ge "$DIAG_INTERVAL" ]; then
    PHASE2_ELAPSED=$(( $(date +%s) - PHASE2_START ))
    echo "  Attempt ${PHASE2_ATTEMPT} (${PHASE2_ELAPSED}s in phase 2, ${SECS}s total)"
    if [ -n "$LAST_ERROR" ]; then
      echo "  Last error: $LAST_ERROR"
    fi
    LAST_DIAG=$SECS
  fi

  sleep "$SLEEP_INTERVAL"
done
