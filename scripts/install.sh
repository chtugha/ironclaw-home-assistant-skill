#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$ROOT_DIR/dist"
TOOL_SRC="$ROOT_DIR/tools-src/ha-tool"

TOOL_NAME="ha-tool"
WASM_FILE="$OUT_DIR/ha_tool.wasm"
CAPS_FILE="$TOOL_SRC/$TOOL_NAME.capabilities.json"
SKILL_FILE="$ROOT_DIR/skills/home-assistant.md"

if ! command -v ironclaw &>/dev/null; then
    echo "ERROR: ironclaw CLI not found. Install it first: https://github.com/nearai/ironclaw"
    exit 1
fi

if [ ! -f "$WASM_FILE" ]; then
    echo "WASM not found. Running build first..."
    "$SCRIPT_DIR/build.sh"
fi

echo "==> Installing $TOOL_NAME tool..."
ironclaw tool install "$WASM_FILE" \
    --capabilities "$CAPS_FILE" \
    --name "$TOOL_NAME"

echo "==> Installing skill..."
SKILL_DIR="${HOME}/.ironclaw/skills/home-assistant"
mkdir -p "$SKILL_DIR"
cp "$SKILL_FILE" "$SKILL_DIR/SKILL.md"
echo "  Installed skill to: $SKILL_DIR/SKILL.md"

echo ""
echo "==> Configuring ha_token secret..."
ironclaw tool setup "$TOOL_NAME"

echo ""
printf "==> Enter your Home Assistant base URL (e.g. http://homeassistant.local:8123): "
read -r HA_URL

WORKSPACE_PATH="${HOME}/.ironclaw/workspace"

if [ -z "$HA_URL" ]; then
    echo "WARNING: No URL entered. You must write it manually before using the tool:"
    echo "  mkdir -p \"${WORKSPACE_PATH}/ha\" && echo 'http://homeassistant.local:8123' > \"${WORKSPACE_PATH}/ha/base_url\""
else
    case "$HA_URL" in
        http://*|https://*)
            mkdir -p "${WORKSPACE_PATH}/ha"
            printf '%s' "${HA_URL%/}" > "${WORKSPACE_PATH}/ha/base_url"
            echo "  Wrote HA base URL to: ${WORKSPACE_PATH}/ha/base_url"
            ;;
        *)
            echo "ERROR: URL must start with http:// or https:// (got: $HA_URL)"
            echo "  Re-run install.sh or write manually:"
            echo "  mkdir -p \"${WORKSPACE_PATH}/ha\" && echo 'http://homeassistant.local:8123' > \"${WORKSPACE_PATH}/ha/base_url\""
            exit 1
            ;;
    esac
fi

echo ""
echo "✓ $TOOL_NAME installed successfully."
echo ""
echo "Test the connection:"
echo "  ironclaw tool run $TOOL_NAME '{\"action\": \"get_status\"}'"
echo ""
echo "Start using in chat:"
echo "  ironclaw chat"
echo "  > What lights are on in my house?"
echo "  > Turn off all lights in the bedroom"
echo "  > Show me the temperature history for the last 24 hours"
