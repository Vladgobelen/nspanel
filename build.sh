#!/bin/bash
# build.sh — сборка nspanel.
# Использование:
#   ./build.sh           — release
#   ./build.sh debug     — debug
#   ./build.sh run       — release + запуск
#   ./build.sh debug-run — debug + запуск

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

MODE="${1:-release}"

case "$MODE" in
    release)
        CARGO_FLAGS="--release"
        RUN=0
        ;;
    debug)
        CARGO_FLAGS=""
        RUN=0
        ;;
    run)
        CARGO_FLAGS="--release"
        RUN=1
        ;;
    debug-run)
        CARGO_FLAGS=""
        RUN=1
        ;;
    *)
        echo "Usage: $0 [release|debug|run|debug-run]"
        exit 1
        ;;
esac

echo "════════════════════════════════════════════════════════"
echo "  Building nspanel ($MODE)"
echo "════════════════════════════════════════════════════════"

if ! cargo build $CARGO_FLAGS; then
    echo "❌ nspanel: FAILED"
    exit 1
fi

echo "✅ nspanel: OK"

if [ "$MODE" = "release" ] || [ "$MODE" = "run" ]; then
    BIN="$SCRIPT_DIR/target/release/nspanel"
else
    BIN="$SCRIPT_DIR/target/debug/nspanel"
fi

echo ""
echo "Binary: $BIN"

if [ "$RUN" = "1" ]; then
    echo ""
    echo "════════════════════════════════════════════════════════"
    echo "  Running nspanel"
    echo "════════════════════════════════════════════════════════"
    "$BIN"
fi