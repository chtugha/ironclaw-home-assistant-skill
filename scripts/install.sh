#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TOOL_SRC="$ROOT_DIR/tools-src/ha-tool"

if ! command -v ironclaw &>/dev/null; then
    echo "ERROR: ironclaw CLI not found. Install it first: https://github.com/nearai/ironclaw"
    exit 1
fi

echo "==> Installing ha-tool from source..."
ironclaw tool install "$TOOL_SRC" --name ha-tool --force

echo ""
echo "==> Installing skill..."
SKILL_DIR="${HOME}/.ironclaw/skills/home-assistant"
mkdir -p "$SKILL_DIR"
cp "$ROOT_DIR/skills/SKILL.md" "$SKILL_DIR/SKILL.md"
echo "  Installed skill to: $SKILL_DIR/SKILL.md"

echo ""
echo "==> Configuring authentication..."
ironclaw tool auth ha-tool

echo ""
echo "============================================"
echo "  ha-tool installed successfully!"
echo "============================================"
echo ""
echo "Test the connection:"
echo "  ironclaw chat"
echo "  > Check if my Home Assistant at http://homeassistant.local:8123 is online"
echo ""
echo "Usage examples:"
echo "  > What lights are on? (ha_url: http://homeassistant.local:8123)"
echo "  > Turn off all lights in the bedroom"
echo "  > Show me the temperature history for the last 24 hours"
echo ""
