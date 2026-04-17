#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TOOL_SRC="$ROOT_DIR/tools-src/ha-tool"

if ! command -v ironclaw &>/dev/null; then
    echo "ERROR: ironclaw CLI not found. Install it first: https://github.com/nearai/ironclaw"
    exit 1
fi

echo "==> Installing ha-tool from source (IronClaw will build the WASM)..."
# IronClaw auto-discovers the tool via its Tool Registry — no skill required.
# Matches upstream pattern: `ironclaw tool install ./tool-src-dir`.
ironclaw tool install "$TOOL_SRC"

echo ""
echo "==> Installing optional skill file (agent hint — not required)..."
SKILL_SRC="$ROOT_DIR/skills/SKILL.md"
SKILL_DEST_DIR="${HOME}/.ironclaw/skills"
SKILL_DEST="$SKILL_DEST_DIR/home-assistant.SKILL.md"
if [[ -f "$SKILL_SRC" ]]; then
    mkdir -p "$SKILL_DEST_DIR"
    cp "$SKILL_SRC" "$SKILL_DEST"
    echo "  Installed skill: $SKILL_DEST"
else
    echo "  (No SKILL.md found — skipping; tool still works via auto-discovery)"
fi

echo ""
echo "==> Configuring Home Assistant access token..."
# `ironclaw tool auth` stores the ha_token secret; IronClaw injects it as a
# Bearer header on HA API requests. The token never enters the WASM sandbox.
ironclaw tool auth ha-tool

echo ""
echo "============================================"
echo "  ha-tool installed successfully!"
echo "============================================"
echo ""
echo "Verify with:"
echo "  ironclaw tool list"
echo "  ironclaw tool info ha-tool"
echo ""
echo "Test in chat (remember: every call needs your HA base URL):"
echo "  ironclaw chat"
echo "  > Is my Home Assistant at http://homeassistant.local:8123 online?"
echo "  > Turn off light.living_room on http://192.168.1.50:8123"
echo ""
