#!/usr/bin/env bash
set -euo pipefail

# CLI Benchmark: Rust nce vs JS npm-check-engines
# Prerequisites: cargo build --release, npm install -g @smarlhens/npm-check-engines, hyperfine
# TODO: Add NAPI-RS binding comparison once phase 7 is complete

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

FIXTURE_SMALL="$ROOT_DIR/fixtures/npm-v3-or-ranges-node-only"
FIXTURE_LARGE="$ROOT_DIR/fixtures/npm-v3-500-deps"
RUST_BIN="$ROOT_DIR/target/release/riri-nce"

if [ ! -f "$RUST_BIN" ] && [ ! -f "$RUST_BIN.exe" ]; then
    echo "Building Rust binary..."
    cargo build --release -p riri-nce --manifest-path="$ROOT_DIR/Cargo.toml"
fi

# Use .exe on Windows
if [ -f "$RUST_BIN.exe" ]; then
    RUST_BIN="$RUST_BIN.exe"
fi

if ! command -v hyperfine &> /dev/null; then
    echo "Error: hyperfine not found. Install with: cargo install hyperfine"
    exit 1
fi

JS_CMD="npx --yes @smarlhens/npm-check-engines -q"

echo ""
echo "=== Small fixture (7 deps) ==="
cd "$FIXTURE_SMALL"
hyperfine \
    --warmup 5 \
    --min-runs 50 \
    -n "rust nce" "$RUST_BIN -q" \
    -n "js nce" "$JS_CMD"

echo ""
echo "=== Large fixture (500 deps) ==="
cd "$FIXTURE_LARGE"
hyperfine \
    --warmup 5 \
    --min-runs 20 \
    -n "rust nce" "$RUST_BIN -q" \
    -n "js nce" "$JS_CMD"
