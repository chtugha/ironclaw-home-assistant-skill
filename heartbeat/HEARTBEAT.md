# Home Assistant Heartbeat Checklist

Copy this file to your IronClaw workspace root as `HEARTBEAT.md` to enable
background monitoring and semi-autonomous maintenance of your Home Assistant
instance. The agent reads it on every heartbeat tick
(default: every 30 minutes — see `HEARTBEAT_INTERVAL_SECS`).

Replace `HA_URL` below with your actual Home Assistant base URL
(e.g. `http://homeassistant.local:8123`, `http://192.168.1.50:8123`).

## Confirmation Rules (MANDATORY)

- **NEVER** call `restart_ha`, `call_service`, `set_state`, `fire_event`,
  `toggle_automation`, `run_script`, `activate_scene`, `mqtt_publish`,
  `modbus_write`, `reload_config_entry`, `reload_core_config`, `reload_automations`,
  `reload_scripts`, `reload_scenes`, `reload_themes`, or `dismiss_notification`
  during a heartbeat tick without explicit user confirmation in the notification.
- Heartbeat ticks are read-only by default: they **detect** problems and
  **propose** remediations; the user confirms before anything is executed.
- If a proposed remediation is confirmed by the user, execute it in the next
  regular chat turn — not inside the heartbeat job.

## Read-only Checks (safe every tick)

- [ ] `ha-tool get_status ha_url=HA_URL` — confirm HA is reachable. If the call
      fails or returns non-200, notify the user immediately with the error.
- [ ] `ha-tool check_config ha_url=HA_URL` — validate HA configuration. If
      `result` is not `"valid"`, notify the user with the `errors` field.
- [ ] `ha-tool get_notifications ha_url=HA_URL` — list persistent notifications.
      If any are present, summarize `title` + `message` + `notification_id`.
- [ ] `ha-tool get_error_log ha_url=HA_URL` — fetch the error log.
      Report only NEW error/warning lines since the last tick
      (compare against `heartbeat/ha-last-log.md` in memory).
- [ ] `ha-tool get_states ha_url=HA_URL domain_filter=automation` —
      flag any automation whose `state` is `"unavailable"` or whose
      `attributes.last_triggered` is older than 30 days (possibly stuck).
- [ ] `ha-tool get_states ha_url=HA_URL domain_filter=binary_sensor` —
      flag any `connectivity` / `problem` / `battery_low` sensor that is `on`.
- [ ] `ha-tool get_states ha_url=HA_URL domain_filter=sensor` —
      flag any sensor in state `"unavailable"` or `"unknown"`.
- [ ] `ha-tool get_states ha_url=HA_URL domain_filter=update` —
      flag any `update.*` entity whose state is `"on"` (update available).

## Analysis & Proposal

- [ ] If any read-only check surfaced issues, write a concise summary to
      memory at `heartbeat/ha-latest.md` with:
      - `time`, `status` (ok|warn|error)
      - `findings` — list of `{entity_id, issue, severity}`
      - `proposed_remediations` — list of `{action, params, rationale}` drawn
        from the extension actions (e.g. `reload_config_entry`, `toggle_automation`,
        `call_service homeassistant reload_config_entry`).
- [ ] Save the raw error-log snapshot to `heartbeat/ha-last-log.md` so the
      next tick can diff against it.

## Notification

- [ ] Send a notification **only if** findings exist. Format:
      `HA heartbeat: N findings — [brief summary]. Propose: [list actions].
       Reply "apply <n>" to execute action n, or "ignore" to dismiss.`
- [ ] Do **not** send a notification if all checks pass — heartbeat is silent
      on healthy systems.

## Remediation Dispatch (executed only after user confirms in chat)

When the user replies with "apply N" or an equivalent confirmation, look up
the N-th proposed remediation from `heartbeat/ha-latest.md` and call the
corresponding `ha-tool` action with the stored params. Common remediations:

- Config edits were made externally → `reload_core_config` / `reload_automations`
  / `reload_scripts` / `reload_scenes` / `reload_themes`.
- Single integration is broken → `reload_config_entry entry_id=<id>`.
- Automation is stuck disabled → `toggle_automation entity_id=<id> enabled=true`.
- Stale sensor from integration restart → `reload_config_entry` (preferred)
  or `restart_ha` (last resort, always ask twice).

## Rate Limits

- Use at most 8 tool calls per heartbeat tick to stay within typical LLM
  budgets. Batch via `get_states` with `domain_filter` rather than looping
  individual `get_state` calls.
