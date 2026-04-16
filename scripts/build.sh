#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TOOL_SRC="$ROOT_DIR/tools-src/ha-tool"
OUT_DIR="$ROOT_DIR/dist"

echo "==> Checking Rust toolchain..."
if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found. Install Rust: https://rustup.rs"
    exit 1
fi

echo "==> Adding WASM target (wasm32-wasip2)..."
rustup target add wasm32-wasip2 2>/dev/null || true

echo "==> Building ha-tool (release, wasm32-wasip2)..."
cargo build --manifest-path "$TOOL_SRC/Cargo.toml" --target wasm32-wasip2 --release

mkdir -p "$OUT_DIR"
cp "$TOOL_SRC/target/wasm32-wasip2/release/ha_tool.wasm" "$OUT_DIR/ha_tool.wasm"

SIZE=$(wc -c < "$OUT_DIR/ha_tool.wasm" | tr -d ' ')
echo ""
echo "Built: $OUT_DIR/ha_tool.wasm ($SIZE bytes)"
