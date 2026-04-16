# IronClaw Home Assistant Extension

## Configuration
- **Artifacts Path**: `.zenflow/tasks/build-an-extension-for-ironclaw-fd60`

### [x] Step 1: Research & Architecture
- Studied IronClaw WASM tool system (WIT interface, capabilities.json, secrets model)
- Studied Home Assistant REST API surface
- Designed single-tool architecture with wildcard HTTP allowlist + two secrets (ha_token, ha_base_url)

### [x] Step 2: Core infrastructure
- `wit/tool.wit` — WIT interface (near:agent@0.3.0 package)
- `.gitignore`
- `tools-src/ha-tool/Cargo.toml`

### [x] Step 3: Capabilities manifest
- `tools-src/ha-tool/ha-tool.capabilities.json`
- Bearer token injection, workspace read, setup flow, wildcard allowlist

### [x] Step 4: WASM tool implementation
- `tools-src/ha-tool/src/lib.rs`
- 30 actions covering: entity states, services, automations, scripts, scenes,
  MQTT publish, Modbus read/write, templates, history, logbook, error log,
  config check, restart, notifications, calendars, reload config entry
- Input validation, URL encoding, ISO 8601 date formatting (no_std-compatible)
- JSON Schema exported via `schema()` function
- 16 unit tests (all passing)
- Compiles cleanly to wasm32-wasip2

### [x] Step 5: Skill file & build scripts
- `skills/home-assistant.md` — comprehensive agent skill with usage patterns
- `scripts/build.sh` — builds WASM to dist/
- `scripts/install.sh` — installs tool + skill into ironclaw, prints setup instructions

### [x] Step 6: Code review bug fix pass
- #1: Fixed misleading resolve_base_url comment (ha_base_url is workspace file, not secret)
- #2: Removed base_url from LLM-facing JSON schema; added _security_note to capabilities.json
- #4: Fixed get_notifications — was querying /api/states, now correctly calls /api/persistent_notification
- #5: Fixed reload_config_entry — replaced non-existent REST endpoint with call_service(homeassistant, reload_config_entry)
- #6: Added MAX_STATES=500 truncation to get_states with _truncated/_hint response fields
- #7: Removed dead ha_delete function
- #8: Added start_time: Option<String> to GetHistory; overrides hours_back when provided
- #9: validate_service now allows hyphen (-) characters for third-party service names
- #10: WIT verified verbatim against ironclaw repo (near:agent@0.3.0)
- All 16 tests pass; WASM builds cleanly to wasm32-wasip2
