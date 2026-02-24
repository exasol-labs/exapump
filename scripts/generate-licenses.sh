#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo-about &>/dev/null; then
    echo "ERROR: cargo-about not installed" >&2
    echo "       Install with: cargo install cargo-about" >&2
    exit 1
fi

cargo about generate about.hbs
