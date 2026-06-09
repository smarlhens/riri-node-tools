#!/usr/bin/env bash
set -euo pipefail

# Binary size analysis for a native CLI (nce or npd).
# Usage: bench-size.sh [nce|npd] [--json]
# Prerequisites (detailed breakdown only): cargo install cargo-bloat

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

TOOL="${1:-nce}"
JSON_MODE=""
[ "${2:-}" = "--json" ] && JSON_MODE="1"

case "$TOOL" in
    nce) CRATE="riri-nce" BIN_NAME="nce" ;;
    npd) CRATE="riri-npd" BIN_NAME="npd" ;;
    *)
        echo "unknown tool: $TOOL (expected nce|npd)" >&2
        exit 2
        ;;
esac

# Resolve the release binary path (.exe on Windows)
BIN="$ROOT_DIR/target/release/$BIN_NAME"
if [ -f "$BIN.exe" ]; then
    BIN="$BIN.exe"
fi

# Portable byte-size of a file (GNU stat, BSD/macOS stat, then wc fallback)
file_bytes() {
    local f="$1"
    stat -c %s "$f" 2> /dev/null || stat -f %z "$f" 2> /dev/null || wc -c < "$f" | tr -d ' '
}

if [ -n "$JSON_MODE" ]; then
    cargo build --release -p "$CRATE" --manifest-path="$ROOT_DIR/Cargo.toml" 1>&2
    if [ -f "$BIN" ]; then
        printf '%s\t%s\n' "$BIN_NAME" "$(file_bytes "$BIN")"
    fi
    exit 0
fi

echo "=== Release binary size ($BIN_NAME) ==="
cargo build --release -p "$CRATE" --manifest-path="$ROOT_DIR/Cargo.toml"

if [ -f "$BIN" ]; then
    ls -lh "$BIN"
fi

if command -v cargo-bloat &> /dev/null; then
    echo ""
    echo "=== Top 20 functions by size ==="
    cargo bloat --release -p "$CRATE" -n 20 --manifest-path="$ROOT_DIR/Cargo.toml"

    echo ""
    echo "=== Crate breakdown ==="
    cargo bloat --release -p "$CRATE" --crates --manifest-path="$ROOT_DIR/Cargo.toml"
else
    echo ""
    echo "cargo-bloat not installed. Install with: cargo install cargo-bloat"
    echo "Skipping detailed size analysis."
fi
