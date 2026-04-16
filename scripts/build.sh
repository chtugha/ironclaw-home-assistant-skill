#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TOOL_DIR="$ROOT_DIR/tools-src/ha-tool"
OUT_DIR="$ROOT_DIR/dist"

TARGET="wasm32-wasip2"
CRATE="ha_tool"
BINARY="$CRATE.wasm"

echo "==> Checking Rust toolchain..."
if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found. Install Rust: https://rustup.rs"
    exit 1
fi

echo "==> Adding WASM target ($TARGET)..."
rustup target add "$TARGET" 2>/dev/null || true

echo "==> Building $CRATE (release, $TARGET)..."
cd "$TOOL_DIR"
cargo build --release --target "$TARGET"

mkdir -p "$OUT_DIR"

WASM_SRC="$TOOL_DIR/target/$TARGET/release/$BINARY"
if [ ! -f "$WASM_SRC" ]; then
    echo "ERROR: Expected WASM output not found: $WASM_SRC"
    exit 1
fi

cp "$WASM_SRC" "$OUT_DIR/$BINARY"

echo ""
echo "✓ Built: $OUT_DIR/$BINARY"
echo "  Size: $(du -sh "$OUT_DIR/$BINARY" | cut -f1)"
echo ""
echo "Run './scripts/install.sh' to install into ironclaw."
