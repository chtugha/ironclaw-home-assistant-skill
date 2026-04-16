# IronClaw Home Assistant Extension

## Configuration
- **Artifacts Path**: `.zenflow/tasks/build-an-extension-for-ironclaw-fd60`

### [x] Step 1: Research & Architecture
- Exhaustive study of IronClaw internals: WIT interface, capabilities schema, credential injection, workspace_read, UrlPath, registry, loader
- Confirmed: workspace_read returns None for standalone tools (no reader injected)
- Confirmed: UrlPath credential injection does not work for standalone tools (credentials HashMap never populated)
- Architecture decision: ha_url is a required parameter on every call (only reliable approach)
- Bearer token (ha_token) auto-injected by host via credentials config

### [x] Step 2: Complete Rebuild from Scratch
- Deleted all previous code and rebuilt following Slack tool pattern exactly
- `wit/tool.wit` — canonical copy from ironclaw upstream
- `tools-src/ha-tool/Cargo.toml` — wit-bindgen =0.36, schemars 1 (matching upstream)
- `tools-src/ha-tool/ha-tool.capabilities.json` — auth section, bearer injection, host_patterns, rate limits
- `tools-src/ha-tool/src/types.rs` — HaAction tagged enum with schemars::JsonSchema derive
- `tools-src/ha-tool/src/api.rs` — all REST API functions with input validation
- `tools-src/ha-tool/src/lib.rs` — tool interface (execute, schema, description)
- `skills/SKILL.md` — skill with activation keywords/patterns, LLM instructions for ha_url
- `scripts/build.sh` — WASM build script
- `scripts/install.sh` — ironclaw tool install from source dir + skill copy + tool auth

### [x] Step 3: Build verification and final testing
- WASM builds cleanly to wasm32-wasip2 with zero warnings
- 7 unit tests pass (url_encode, entity_id, domain, service, iso_prefix, normalize_url, days_to_ymd)
- install.sh uses `ironclaw tool install <source-dir>` (auto-builds from Cargo.toml)
- Skill copied to ~/.ironclaw/skills/home-assistant/SKILL.md
- Auth configured via `ironclaw tool auth ha-tool`

### [x] Step 4: Review fixes — security and correctness
- SSRF fix: validate_ha_url restricts to private/local addresses (localhost, 192.168.*, 10.*, 172.16-31.*, *.local, *.internal, *.lan, *.home, *.duckdns.org, *.nabu.casa)
- get_notifications: uses /api/persistent_notification (correct HA endpoint)
- hours_back bounds: validated 1-8760 in get_history and get_logbook
- Domain prefix validation: toggle_automation, trigger_automation require automation.*, run_script requires script.*, activate_scene requires scene.*
- MQTT QoS: validated 0-2
- Empty body fix: call_service, fire_event, check_config, restart_ha all send {} when no body
- Modbus hub: restored optional hub parameter for multi-hub setups
- 8 unit tests pass including new validate_ha_url test
