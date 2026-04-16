# IronClaw — Home Assistant Extension

A [WASM tool](https://github.com/nearai/ironclaw) that gives [IronClaw](https://github.com/nearai/ironclaw) full control over a [Home Assistant](https://www.home-assistant.io) instance via the HA REST API.

Once installed, you can talk to IronClaw in plain language:

> "Turn off all lights in the bedroom."
> "What's the current temperature in the living room?"
> "Trigger the morning routine automation."
> "Publish MQTT payload ON to home/bedroom/light/set."
> "Show me the temperature sensor history for the last 48 hours."

---

## What it can do

| Category | Actions |
|---|---|
| **Entities** | Get all states, get/set individual state, filter by domain |
| **Services** | List services, call any HA service (lights, climate, locks, covers, fans, media, …) |
| **Automations** | List, trigger, enable/disable |
| **Scripts** | List, run with variables |
| **Scenes** | List, activate |
| **MQTT** | Publish to any topic (via HA MQTT integration) |
| **Modbus** | Read holding/input registers via entity, write holding/coil registers |
| **Templates** | Render any Jinja2 template through HA |
| **Events** | List event types, fire custom events |
| **History** | Entity state history by hours or ISO 8601 start time |
| **Logbook** | HA logbook filtered by entity |
| **Logs** | Read HA error log |
| **Notifications** | Send via any notify service (mobile app, persistent, etc.) |
| **Calendars** | List calendars, fetch events in a time range |
| **System** | Check config, reload a config entry, restart HA |

---

## Requirements

| Requirement | Version |
|---|---|
| [IronClaw](https://github.com/nearai/ironclaw) | latest |
| [Rust](https://rustup.rs) | stable (1.75+) |
| `wasm32-wasip2` target | added automatically by build script |
| Home Assistant | 2023.x or later (REST API must be enabled — it is by default) |

---

## Installation

### Step 1 — Clone this repository

```bash
git clone https://github.com/chtugha/ironclaw-home-assistant-skill.git
cd ironclaw-home-assistant-skill
```

### Step 2 — Build and install

Run the single install script. It builds the WASM binary, registers the tool and skill with IronClaw, prompts for your HA access token, and writes the HA base URL to the IronClaw workspace:

```bash
./scripts/install.sh
```

The script does the following automatically:

1. **Builds** `tools-src/ha-tool` as a `wasm32-wasip2` WASM component → `dist/ha_tool.wasm`
2. **Installs** the tool into IronClaw: `ironclaw tool install --name ha-tool --wasm dist/ha_tool.wasm --capabilities …`
3. **Installs** the agent skill: `ironclaw skill install --name home-assistant --file skills/home-assistant.md`
4. **Runs** `ironclaw tool setup ha-tool` — you will be prompted to enter your HA long-lived access token
5. **Prompts** for your HA base URL and writes it to `$(ironclaw workspace path)/ha/base_url`

### Step 3 — Get a Home Assistant long-lived access token

1. Open Home Assistant in your browser
2. Go to your **Profile** page: `http://<your-ha-url>:8123/profile`
3. Scroll down to **Long-Lived Access Tokens**
4. Click **Create Token**, give it a name (e.g. `ironclaw`), and copy the token
5. Paste it when prompted by the install script

### Step 4 — Verify

```bash
ironclaw tool run ha-tool '{"action": "get_status"}'
```

Expected output: a JSON object with your HA version and API status.

---

## Manual installation (step by step)

If you prefer to install each piece manually:

### Build the WASM binary

```bash
./scripts/build.sh
```

Output: `dist/ha_tool.wasm`

### Install the tool

```bash
ironclaw tool install \
    --name ha-tool \
    --wasm dist/ha_tool.wasm \
    --capabilities tools-src/ha-tool/ha-tool.capabilities.json
```

### Install the skill

```bash
ironclaw skill install \
    --name home-assistant \
    --file skills/home-assistant.md
```

### Store the access token

```bash
ironclaw tool setup ha-tool
# You will be prompted for: ha_token
```

### Write the base URL

```bash
echo 'http://homeassistant.local:8123' > "$(ironclaw workspace path)/ha/base_url"
```

Replace `http://homeassistant.local:8123` with your actual HA address. This can be:
- `http://homeassistant.local:8123` — mDNS name (works on most local networks)
- `http://192.168.1.100:8123` — direct IP
- `https://your-home.duckdns.org` — remote access via DuckDNS or Nabu Casa

---

## Configuration details

### How credentials work

IronClaw WASM tools run in a strict sandbox and **cannot read secret values** — they can only check whether a secret exists. The HA bearer token (`ha_token`) is injected by the IronClaw host directly into the `Authorization` header of every outgoing HTTP request. The WASM code never sees it.

Because of this, the HA base URL cannot be stored as a secret either — it is instead written to a plain file in the IronClaw workspace at `ha/base_url`. The tool reads this file at runtime via the `workspace-read` capability.

### Credential storage summary

| Value | Stored as | Where |
|---|---|---|
| `ha_token` | Secret (injected at HTTP boundary) | IronClaw secret store |
| HA base URL | Workspace file | `$(ironclaw workspace path)/ha/base_url` |

### Changing the base URL

```bash
echo 'https://new-address.duckdns.org' > "$(ironclaw workspace path)/ha/base_url"
```

### Changing the token

```bash
ironclaw tool setup ha-tool
```

---

## Usage

Start a chat session:

```bash
ironclaw chat
```

IronClaw will automatically use the `ha-tool` when you ask about your smart home. Examples:

```
> What lights are on right now?
> Set the living room thermostat to 21 degrees.
> Turn off all switches in the kitchen.
> Show me the door sensor state history for the last 24 hours.
> Trigger the bedtime automation.
> Publish MQTT payload {"state": "ON"} to zigbee2mqtt/light1/set
> Write Modbus holding register 40010 on unit 1 to value 2200.
> Check the HA config and restart if it's valid.
```

---

## Supported actions (tool API reference)

All actions are invoked as JSON objects with an `"action"` field. You can also call the tool directly for testing:

```bash
ironclaw tool run ha-tool '<JSON>'
```

### Entity state

| Action | Required fields | Optional fields |
|---|---|---|
| `get_status` | — | — |
| `get_config` | — | — |
| `get_states` | — | `domain_filter` |
| `get_state` | `entity_id` | — |
| `set_state` | `entity_id`, `state` | `attributes` |

`get_states` response shape (always an object):
```json
{"entities": [...], "count": 42}
```
When capped at 500: `"_truncated": true` and `"_hint"` are added.

### Services

| Action | Required fields | Optional fields |
|---|---|---|
| `get_services` | — | `domain_filter` |
| `call_service` | `domain`, `service` | `data` |

### Events

| Action | Required fields | Optional fields |
|---|---|---|
| `get_events` | — | — |
| `fire_event` | `event_type` | `event_data` |

### History and logs

| Action | Required fields | Optional fields |
|---|---|---|
| `get_history` | `entity_id` | `hours_back` (default 24), `start_time` (ISO 8601), `minimal_response` |
| `get_logbook` | — | `entity_id`, `hours_back` |
| `get_error_log` | — | — |

`start_time` format: `2024-03-15T08:00:00+00:00`

### Automations

| Action | Required fields | Optional fields |
|---|---|---|
| `list_automations` | — | — |
| `trigger_automation` | `entity_id` | — |
| `toggle_automation` | `entity_id`, `enabled` | — |

### Scripts

| Action | Required fields | Optional fields |
|---|---|---|
| `list_scripts` | — | — |
| `run_script` | `entity_id` | `variables` |

### Scenes

| Action | Required fields | Optional fields |
|---|---|---|
| `list_scenes` | — | — |
| `activate_scene` | `entity_id` | — |

### Templates

| Action | Required fields | Optional fields |
|---|---|---|
| `render_template` | `template` | — |

### MQTT

| Action | Required fields | Optional fields |
|---|---|---|
| `mqtt_publish` | `topic`, `payload` | `retain`, `qos` (0/1/2) |

Requires the HA MQTT integration to be installed and configured.

### Modbus

| Action | Required fields | Optional fields |
|---|---|---|
| `modbus_read` | `entity_id` | — |
| `modbus_write` | `unit`, `address`, `value` | `hub`, `write_type` (`holding`/`coil`) |

`write_type` defaults to `holding`. Input registers are read-only and will return an error.

### Notifications

| Action | Required fields | Optional fields |
|---|---|---|
| `send_notification` | `service`, `message` | `title`, `data` |
| `get_notifications` | — | — |

`service` is the notify service name, e.g. `mobile_app_my_phone`, `persistent_notification`.

### Calendars

| Action | Required fields | Optional fields |
|---|---|---|
| `get_calendars` | — | — |
| `get_calendar_events` | `entity_id` | `start`, `end` (ISO 8601) |

### System

| Action | Required fields | Optional fields |
|---|---|---|
| `check_config` | — | — |
| `reload_config_entry` | `entry_id` | — |
| `restart_ha` | — | — |
| `get_panels` | — | — |

> ⚠️ `restart_ha` restarts the Home Assistant process. All automations and devices will be briefly unavailable. Always run `check_config` first.

---

## Architecture

```
ironclaw-home-assistant-skill/
├── wit/
│   └── tool.wit                    # WIT interface (near:agent@0.3.0)
├── tools-src/
│   └── ha-tool/
│       ├── Cargo.toml
│       ├── ha-tool.capabilities.json   # Sandbox capabilities manifest
│       └── src/
│           └── lib.rs              # WASM tool implementation (~1300 lines)
├── skills/
│   └── home-assistant.md           # Agent skill loaded by IronClaw
├── scripts/
│   ├── build.sh                    # Builds WASM binary to dist/
│   └── install.sh                  # Full install: build + register + configure
└── dist/
    └── ha_tool.wasm                # Compiled output (generated, not committed)
```

### WASM sandbox model

The tool is compiled to a [WASM Component](https://component-model.bytecodealliance.org) targeting `wasm32-wasip2`. IronClaw loads it into a strict sandbox with the following capabilities declared in `ha-tool.capabilities.json`:

- **HTTP**: outbound requests to any host (wildcard — required because the HA URL is user-defined). The bearer token is injected by the host; the WASM code never has access to it.
- **Workspace read**: reads `ha/base_url` from the IronClaw workspace directory.
- **Secrets check**: can verify that `ha_token` exists (but cannot read its value).
- **Rate limiting**: 120 req/min, 3600 req/hour.
- **Timeout**: 30 seconds per request.

### Why REST-only (no CLI or MCP dispatch)

WASM sandboxes have no `exec` capability — they cannot spawn subprocesses. This means tools like `hass-cli`, `mosquitto_pub`, or `modpoll` cannot be invoked from within WASM. The Home Assistant REST API covers the same functionality (service calls, MQTT publish via HA's broker, Modbus writes via the HA modbus integration) and is the correct interface for this use case.

---

## Development

### Run tests

```bash
cd tools-src/ha-tool
cargo test
```

### Build only (no install)

```bash
./scripts/build.sh
```

### Rebuild and reinstall

```bash
./scripts/install.sh
```

The install script is idempotent — it can be run again to update the tool and skill after code changes.

---

## Troubleshooting

**`Home Assistant base URL not configured`**
The workspace file is missing. Write it:
```bash
echo 'http://homeassistant.local:8123' > "$(ironclaw workspace path)/ha/base_url"
```

**`Home Assistant token not found`**
The token secret was not stored. Run:
```bash
ironclaw tool setup ha-tool
```

**`Home Assistant API error (HTTP 401)`**
The token is invalid or expired. Create a new long-lived token on your HA profile page and re-run `ironclaw tool setup ha-tool`.

**`Home Assistant API error (HTTP 404)`**
The entity ID or service does not exist. Use `get_states` and `get_services` to discover what is available.

**Connection refused / timeout**
- Confirm HA is running: open `http://<your-ha-url>:8123` in a browser
- If accessing remotely, ensure port 8123 is reachable or use your external URL (Nabu Casa / DuckDNS)
- If using HTTPS, ensure the certificate is valid

**MQTT publish fails**
The HA MQTT integration must be installed and connected to a broker. Check **Settings → Devices & Services → MQTT** in HA.

**Modbus write fails**
Confirm the modbus integration is configured in HA and the hub name matches your `configuration.yaml`. Input registers (`write_type: "input"`) are read-only.

---

## License

MIT OR Apache-2.0
