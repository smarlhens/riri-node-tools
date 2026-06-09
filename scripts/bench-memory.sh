#!/usr/bin/env bash
set -euo pipefail

# Peak memory (RSS) profiling for a native CLI (nce or npd).
# Usage: bench-memory.sh [nce|npd] [--json]
# Needs GNU time (`gtime` or `/usr/bin/time -v`); degrades to 0 when absent.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

TOOL="${1:-nce}"
JSON_MODE=""
[ "${2:-}" = "--json" ] && JSON_MODE="1"

case "$TOOL" in
    nce)
        CRATE="riri-nce"
        BIN_NAME="nce"
        FIXTURE_SMALL="$ROOT_DIR/fixtures/npm-v3-or-ranges-node-only"
        FIXTURE_LARGE="$ROOT_DIR/fixtures/npm-v3-500-deps"
        ;;
    npd)
        CRATE="riri-npd"
        BIN_NAME="npd"
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
    if [ -n "$JSON_MODE" ]; then
        cargo build --release -p "$CRATE" --manifest-path="$ROOT_DIR/Cargo.toml" 1>&2
    else
        echo "Building Rust binary..."
        cargo build --release -p "$CRATE" --manifest-path="$ROOT_DIR/Cargo.toml"
    fi
fi

if [ -f "$RUST_BIN.exe" ]; then
    RUST_BIN="$RUST_BIN.exe"
fi

# Resolve a GNU-compatible `time` that supports `-v` (gtime on macOS via
# coreutils/gnu-time; /usr/bin/time -v on Linux). Empty when unavailable.
TIME_BIN=""
if command -v gtime &> /dev/null; then
    TIME_BIN="gtime"
elif /usr/bin/time -v true &> /dev/null; then
    TIME_BIN="/usr/bin/time"
fi

# Peak resident set size (kbytes) for one run of the binary in $1.
# Prints nothing when no GNU time is available. Never fails the caller.
peak_rss_kb() {
    local dir="$1"
    [ -n "$TIME_BIN" ] || return 0
    (
        cd "$dir" 2> /dev/null || exit 0
        "$TIME_BIN" -v "$RUST_BIN" -q 2>&1 \
            | grep -i "maximum resident" \
            | grep -oE '[0-9]+' \
            | tail -n 1 || true
    ) || true
}

if [ -n "$JSON_MODE" ]; then
    small_kb="$(peak_rss_kb "$FIXTURE_SMALL")"
    large_kb="$(peak_rss_kb "$FIXTURE_LARGE")"
    small_kb="${small_kb:-0}"
    large_kb="${large_kb:-0}"
    peak_kb="$small_kb"
    [ "$large_kb" -gt "$peak_kb" ] && peak_kb="$large_kb"
    total_kb="$((small_kb + large_kb))"
    printf 'peak_kb=%s\n' "$peak_kb"
    printf 'total_kb=%s\n' "$total_kb"
    exit 0
fi

echo "=== Memory profiling: $BIN_NAME ==="
echo ""

measure_memory() {
    local label="$1"
    local dir="$2"

    echo "--- $label ---"
    if [ -n "$TIME_BIN" ]; then
        (cd "$dir" && "$TIME_BIN" -v "$RUST_BIN" -q 2>&1 | grep -i "maximum resident")
    else
        echo "  No GNU time available (install coreutils/gnu-time for gtime, or use Linux)."
    fi
    echo ""
}

measure_memory "Small fixture" "$FIXTURE_SMALL"
measure_memory "Large fixture (500 deps)" "$FIXTURE_LARGE"

echo "=== For detailed heap profiling, run: ==="
echo "  cargo test -p $CRATE --test memory_profile -- --ignored --nocapture --test-threads=1"
