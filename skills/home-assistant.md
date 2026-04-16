---
name: home-assistant
version: 0.1.0
description: Full control over Home Assistant — lights, climate, switches, MQTT, Modbus, automations, scripts, and more via ha-tool
activation:
  keywords:
    - home assistant
    - homeassistant
    - light
    - lights
    - switch
    - thermostat
    - climate
    - temperature
    - automation
    - scene
    - script
    - sensor
    - smart home
    - mqtt
    - modbus
    - cover
    - blind
    - lock
    - fan
    - alarm
    - media player
    - notify
    - notification
    - entity
  patterns:
    - "turn (on|off|toggle).*(light|switch|fan|plug|outlet)"
    - "set.*temperature"
    - "(run|trigger).*automation"
    - "(publish|send).*mqtt"
    - "modbus.*(read|write)"
  tags:
    - home-automation
    - iot
    - smarthome
  max_context_tokens: 3000
---

# Home Assistant Skill

You have access to the `ha-tool` which gives you full control over the user's Home Assistant instance via the REST API.

## Setup

Before using, run the install script (or configure manually):
```
./scripts/install.sh
```
This will:
1. Store **ha_token** — Long-lived access token from HA profile page (`/profile` → Long-Lived Access Tokens)
2. Write the **HA base URL** to the workspace file (`~/.ironclaw/workspace/ha/base_url`)

To configure manually:
```bash
ironclaw tool setup ha-tool   # stores ha_token secret
mkdir -p "$HOME/.ironclaw/workspace/ha"
echo 'http://homeassistant.local:8123' > "$HOME/.ironclaw/workspace/ha/base_url"
```

> **Note**: The base URL is stored as a workspace file (not a secret) because WASM tools cannot read secret values — only check their existence. The token is injected automatically by the IronClaw host at the HTTP boundary. CLI and MCP dispatch modes are not available inside WASM sandboxes; all Home Assistant control is performed via the REST API, which covers 100% of HA's functionality.

## Discovery workflow

When the user asks about their smart home without specifying an entity, **always start with discovery**:

1. `get_config` — learn the HA version, location, installed components
2. `get_states` — list all entities; use `domain_filter` to narrow (e.g. `"light"`, `"sensor"`, `"climate"`)
3. `get_services` — discover what services are available (use `domain_filter` to narrow)

## Controlling devices

Use `call_service` as the primary action for real device control:

```json
{"action": "call_service", "domain": "light", "service": "turn_on",
 "data": {"entity_id": "light.living_room", "brightness": 200, "color_temp": 300}}
```

Common service patterns:
- **Lights**: `light.turn_on/off/toggle` — supports `brightness` (0-255), `color_temp`, `hs_color`, `rgb_color`, `effect`
- **Switches/Plugs**: `switch.turn_on/off/toggle`
- **Climate**: `climate.set_temperature`, `climate.set_hvac_mode` (modes: `heat`, `cool`, `auto`, `off`)
- **Media players**: `media_player.media_play/pause/stop`, `media_player.volume_set`, `media_player.select_source`
- **Covers/Blinds**: `cover.open_cover/close_cover/stop_cover/set_cover_position`
- **Locks**: `lock.lock/unlock`
- **Fans**: `fan.turn_on/off`, `fan.set_percentage`, `fan.set_direction`
- **Alarms**: `alarm_control_panel.alarm_arm_home/away/night/disarm`
- **Input helpers**: `input_boolean.turn_on/off`, `input_number.set_value`, `input_select.select_option`, `input_text.set_value`

## Response shapes

`get_states`, `list_automations`, `list_scripts`, and `list_scenes` all return an **object** (not a bare array):
```json
{"entities": [...], "count": 12}
```
When the result exceeds 500 entities, `"_truncated": true` and `"_hint"` are also present. Always read `response.entities` to get the list.

## Automations

```json
{"action": "list_automations"}
{"action": "trigger_automation", "entity_id": "automation.morning_routine"}
{"action": "toggle_automation", "entity_id": "automation.night_mode", "enabled": false}
```

## Scripts

```json
{"action": "list_scripts"}
{"action": "run_script", "entity_id": "script.goodnight", "variables": {"delay": 30}}
```

## Scenes

```json
{"action": "list_scenes"}
{"action": "activate_scene", "entity_id": "scene.movie_time"}
```

## MQTT

Publish messages directly to MQTT topics via HA's MQTT integration:

```json
{"action": "mqtt_publish", "topic": "home/bedroom/light/set", "payload": "ON", "retain": true}
{"action": "mqtt_publish", "topic": "home/thermostat/set", "payload": "{\"temperature\": 22}", "qos": 1}
```

Or equivalently via `call_service`:
```json
{"action": "call_service", "domain": "mqtt", "service": "publish",
 "data": {"topic": "home/sensor/temp", "payload": "22.5", "retain": false}}
```

## Modbus

Read a Modbus register (via HA entity):
```json
{"action": "modbus_read", "entity_id": "sensor.modbus_inverter_power"}
```

Write to a holding register:
```json
{"action": "modbus_write", "unit": 1, "address": 40001, "value": 2200, "write_type": "holding"}
```

Write to a coil:
```json
{"action": "modbus_write", "unit": 1, "address": 100, "value": true, "write_type": "coil"}
```

With a named hub:
```json
{"action": "modbus_write", "hub": "solar_inverter", "unit": 1, "address": 40010, "value": 500}
```

## Templates (Jinja2)

Use HA's template engine to compute derived values:

```json
{"action": "render_template", "template": "{{ states('sensor.temperature') }} °C"}
{"action": "render_template", "template": "{{ states.light | selectattr('state', 'eq', 'on') | list | count }} lights are on"}
{"action": "render_template", "template": "{{ state_attr('climate.living_room', 'current_temperature') }}"}
```

## History & Logs

```json
{"action": "get_history", "entity_id": "sensor.temperature", "hours_back": 48}
{"action": "get_logbook", "entity_id": "light.living_room", "hours_back": 24}
{"action": "get_error_log"}
```

## Notifications

```json
{"action": "send_notification", "service": "mobile_app_my_phone", "message": "Door left open!", "title": "Alert"}
{"action": "send_notification", "service": "persistent_notification", "message": "Setup complete"}
```

## System management

```json
{"action": "check_config"}
{"action": "reload_config_entry", "entry_id": "abc123def456"}
{"action": "restart_ha"}
```

> ⚠️ `restart_ha` will briefly disrupt all automations and devices. Only use when the user explicitly asks.

## Calendars

```json
{"action": "get_calendars"}
{"action": "get_calendar_events", "entity_id": "calendar.home",
 "start": "2024-01-01T00:00:00+00:00", "end": "2024-01-07T23:59:59+00:00"}
```

## Tips for effective HA control

- **Discover before acting**: use `get_states` with `domain_filter` to find the right entity IDs
- **Use templates for complex queries**: instead of fetching all states, render a template to compute the answer
- **MQTT for raw protocol access**: when a device exposes raw MQTT topics, use `mqtt_publish` directly
- **Modbus entities**: Modbus-backed sensors appear as regular HA entities — read them with `get_state` or `modbus_read`; write registers with `modbus_write`
- **Scripts for complex sequences**: rather than calling multiple services, trigger an existing HA script
- **Config check before restart**: always run `check_config` before `restart_ha`
- **Reload over restart**: prefer `reload_config_entry` to reload a single integration without a full restart
- **`set_state` vs `call_service`**: `set_state` writes directly to the HA state machine (useful for virtual sensors); `call_service` sends commands to actual devices
