#!/usr/bin/env bash
set -euo pipefail

# Memory Profiling for riri-nce
# Measures peak memory usage (RSS) during execution
# TODO: Add NAPI-RS binding memory comparison once phase 7 is complete

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

FIXTURE_SMALL="$ROOT_DIR/fixtures/npm-v3-or-ranges-node-only"
FIXTURE_LARGE="$ROOT_DIR/fixtures/npm-v3-500-deps"
RUST_BIN="$ROOT_DIR/target/release/riri-nce"

if [ ! -f "$RUST_BIN" ] && [ ! -f "$RUST_BIN.exe" ]; then
    echo "Building Rust binary..."
    cargo build --release -p riri-nce --manifest-path="$ROOT_DIR/Cargo.toml"
fi

if [ -f "$RUST_BIN.exe" ]; then
    RUST_BIN="$RUST_BIN.exe"
fi

echo "=== Memory profiling: Rust nce ==="
echo ""

measure_memory() {
    local label="$1"
    local dir="$2"

    echo "--- $label ---"
    cd "$dir"

    if command -v /usr/bin/time &> /dev/null; then
        # Linux/macOS: use GNU time for peak RSS
        /usr/bin/time -v "$RUST_BIN" -q 2>&1 | grep -i "maximum resident"
    elif command -v powershell &> /dev/null; then
        # Windows: use PowerShell to measure peak working set
        powershell -Command "
            \$proc = Start-Process -FilePath '$RUST_BIN' -ArgumentList '-q' -PassThru -NoNewWindow -Wait
            # Peak working set not available after process exits; use Get-Process during execution
        " 2>/dev/null || echo "  (PowerShell measurement not supported for short-lived processes)"
        # Fallback: just run and report that manual profiling is needed
        echo "  Tip: Use 'dhat' benchmark below for detailed heap profiling"
    else
        echo "  No memory measurement tool available."
        echo "  On Linux: install GNU time (apt install time)"
        echo "  On Windows: use the dhat benchmark below"
    fi
    echo ""
}

measure_memory "Small fixture (7 deps)" "$FIXTURE_SMALL"
measure_memory "Large fixture (500 deps)" "$FIXTURE_LARGE"

echo "=== For detailed heap profiling, run: ==="
echo "  cargo test -p riri-nce --test memory_profile -- --ignored --nocapture --test-threads=1"
