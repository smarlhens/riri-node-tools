#!/usr/bin/env bash
set -euo pipefail

# Binary Size Analysis for riri-nce
# Prerequisites: cargo install cargo-bloat
# TODO: Add NAPI-RS binding size analysis once phase 7 is complete

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

JSON_MODE=""
[ "${1:-}" = "--json" ] && JSON_MODE="1"

# Resolve the release binary path (.exe on Windows)
BIN="$ROOT_DIR/target/release/riri-nce"
if [ -f "$BIN.exe" ]; then
    BIN="$BIN.exe"
fi

# Portable byte-size of a file (GNU stat, BSD/macOS stat, then wc fallback)
file_bytes() {
    local f="$1"
    stat -c %s "$f" 2> /dev/null || stat -f %z "$f" 2> /dev/null || wc -c < "$f" | tr -d ' '
}

if [ -n "$JSON_MODE" ]; then
    cargo build --release -p riri-nce --manifest-path="$ROOT_DIR/Cargo.toml" 1>&2
    if [ -f "$BIN" ]; then
        printf '%s\t%s\n' "riri-nce" "$(file_bytes "$BIN")"
    fi
    exit 0
fi

echo "=== Release binary size ==="
cargo build --release -p riri-nce --manifest-path="$ROOT_DIR/Cargo.toml"

if [ -f "$BIN" ]; then
    ls -lh "$BIN"
fi

if command -v cargo-bloat &> /dev/null; then
    echo ""
    echo "=== Top 20 functions by size ==="
    cargo bloat --release -p riri-nce -n 20 --manifest-path="$ROOT_DIR/Cargo.toml"

    echo ""
    echo "=== Crate breakdown ==="
    cargo bloat --release -p riri-nce --crates --manifest-path="$ROOT_DIR/Cargo.toml"
else
    echo ""
    echo "cargo-bloat not installed. Install with: cargo install cargo-bloat"
    echo "Skipping detailed size analysis."
fi
