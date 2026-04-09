#!/usr/bin/env bash
set -euo pipefail

# Binary Size Analysis for riri-nce
# Prerequisites: cargo install cargo-bloat
# TODO: Add NAPI-RS binding size analysis once phase 7 is complete

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Release binary size ==="
cargo build --release -p riri-nce --manifest-path="$ROOT_DIR/Cargo.toml"

if [ -f "$ROOT_DIR/target/release/riri-nce.exe" ]; then
    ls -lh "$ROOT_DIR/target/release/riri-nce.exe"
elif [ -f "$ROOT_DIR/target/release/riri-nce" ]; then
    ls -lh "$ROOT_DIR/target/release/riri-nce"
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
