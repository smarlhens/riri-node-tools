#!/usr/bin/env bash
set -euo pipefail

# CLI benchmark: native Rust binary vs the published JS package.
# Usage: bench-cli.sh [nce|npd] [json_prefix]
# Prerequisites: cargo build --release, hyperfine

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

TOOL="${1:-nce}"
JSON_OUT="${2:-}" # optional: path prefix for --export-json

case "$TOOL" in
    nce)
        CRATE="riri-nce"
        BIN_NAME="nce"
        JS_CMD="npx --yes @smarlhens/npm-check-engines -q"
        FIXTURE_SMALL="$ROOT_DIR/fixtures/npm-v3-or-ranges-node-only"
        FIXTURE_LARGE="$ROOT_DIR/fixtures/npm-v3-500-deps"
        ;;
    npd)
        CRATE="riri-npd"
        BIN_NAME="npd"
        JS_CMD="npx --yes @smarlhens/npm-pin-dependencies -q"
        FIXTURE_SMALL="$ROOT_DIR/fixtures/npd-npm-v3-unpinned-deps"
        FIXTURE_LARGE="$ROOT_DIR/fixtures/npd-npm-v3-500-deps"
        ;;
    *)
        echo "unknown tool: $TOOL (expected nce|npd)" >&2
        exit 2
        ;;
esac

RUST_BIN="$ROOT_DIR/target/release/$BIN_NAME"

if [ ! -f "$RUST_BIN" ] && [ ! -f "$RUST_BIN.exe" ]; then
    echo "Building Rust binary..."
    cargo build --release -p "$CRATE" --manifest-path="$ROOT_DIR/Cargo.toml"
fi

# Use .exe on Windows
if [ -f "$RUST_BIN.exe" ]; then
    RUST_BIN="$RUST_BIN.exe"
fi

if ! command -v hyperfine &> /dev/null; then
    echo "Error: hyperfine not found. Install with: cargo install hyperfine" >&2
    exit 1
fi

run_one() {
    local label="$1" dir="$2" min_runs="$3" export_path="$4"
    echo ""
    echo "=== $label ==="
    cd "$dir"
    local export_args=()
    [ -n "$export_path" ] && export_args=(--export-json "$export_path")
    # --ignore-failure: nce/npd signal pending changes via a non-zero exit code
    # (1 = changes pending); we are timing execution, not asserting success.
    hyperfine \
        --warmup 5 \
        --min-runs "$min_runs" \
        --ignore-failure \
        "${export_args[@]}" \
        -n "rust $TOOL" "$RUST_BIN -q" \
        -n "js $TOOL" "$JS_CMD"
}

SMALL_EXPORT=""
LARGE_EXPORT=""
if [ -n "$JSON_OUT" ]; then
    SMALL_EXPORT="${JSON_OUT}-small.json"
    LARGE_EXPORT="${JSON_OUT}-large.json"
fi

run_one "Small fixture" "$FIXTURE_SMALL" 50 "$SMALL_EXPORT"
run_one "Large fixture (500 deps)" "$FIXTURE_LARGE" 20 "$LARGE_EXPORT"
