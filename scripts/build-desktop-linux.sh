#!/bin/bash
set -euo pipefail

if [ "$(uname -s)" != Linux ]; then
    echo "Run this script inside Linux (for example, the SwiftVM Ubuntu guest)." >&2
    exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export CODESH_ZIG="${CODESH_ZIG:-$(command -v zig || true)}"
if [ ! -x "$CODESH_ZIG" ] || [ "$CODESH_ZIG" = "$ROOT/scripts/zig/zig" ]; then
    echo "Set CODESH_ZIG to the real Zig 0.15.2 executable." >&2
    exit 1
fi
export PATH="$ROOT/scripts/zig:$PATH"
export LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target/linux-desktop}"
cd "$ROOT"
cargo build --locked -p codesh-app -p codesh-cli -p codesh-server

echo "Built desktop and matching helpers in $CARGO_TARGET_DIR/debug"
echo "From a Linux desktop terminal, run: $CARGO_TARGET_DIR/debug/codesh-app"
