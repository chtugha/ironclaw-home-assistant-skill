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
echo "==> Installing optional HEARTBEAT.md template (background monitoring)..."
HEARTBEAT_SRC="$ROOT_DIR/heartbeat/HEARTBEAT.md"
HEARTBEAT_DEST_DIR="${HOME}/.ironclaw"
HEARTBEAT_DEST="$HEARTBEAT_DEST_DIR/HEARTBEAT.md"
if [[ -f "$HEARTBEAT_SRC" ]]; then
    if [[ -f "$HEARTBEAT_DEST" ]]; then
        echo "  HEARTBEAT.md already exists at $HEARTBEAT_DEST — leaving unchanged."
        echo "  (Merge entries from $HEARTBEAT_SRC manually if desired.)"
    else
        mkdir -p "$HEARTBEAT_DEST_DIR"
        cp "$HEARTBEAT_SRC" "$HEARTBEAT_DEST"
        echo "  Installed: $HEARTBEAT_DEST"
        echo "  Edit it and replace HA_URL with your Home Assistant base URL."
    fi
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
echo "Background monitoring (optional):"
echo "  1. Edit $HOME/.ironclaw/HEARTBEAT.md and replace HA_URL with your HA URL."
echo "  2. Ensure HEARTBEAT_ENABLED=true in your IronClaw config."
echo "  3. See heartbeat/routines.md for cron-routine prompts you can paste"
echo "     into chat to create scheduled HA health checks."
echo ""
