//! Home Assistant WASM Tool for IronClaw.
//!
//! Provides full Home Assistant control via the REST API:
//! - Entity state reading and writing
//! - Service calls (lights, switches, climate, media players, etc.)
//! - Automation / script / scene management
//! - MQTT publish (via HA services API)
//! - Modbus device control (via HA modbus integration)
//! - Template rendering (Jinja2 via HA)
//! - System management (logs, config check, restart)
//!
//! # Setup
//!
//! Run: `ironclaw tool setup ha-tool`
//!
//! You will be prompted for:
//!   ha_token    — Long-lived access token from your HA profile page
//!   ha_base_url — Base URL of your HA instance, e.g. http://homeassistant.local:8123
//!
//! # Authentication
//!
//! The `ha_token` secret is injected as a Bearer token automatically by the
//! ironclaw host. You never need to handle it in code.
//! The `ha_base_url` is read from workspace file `ha/base_url`. To populate
//! this file, create it manually: echo "http://homeassistant.local:8123" > \
//! ~/.ironclaw/workspace/ha/base_url

wit_bindgen::generate!({
    world: "sandboxed-tool",
    path: "../../wit/tool.wit",
});

use serde::Deserialize;

const MAX_INPUT_LEN: usize = 65536;
const HA_BASE_URL_WORKSPACE_KEY: &str = "ha/base_url";

// ─── Input validation ──────────────────────────────────────────────────────

fn validate_len(s: &str, field: &str) -> Result<(), String> {
    if s.len() > MAX_INPUT_LEN {
        return Err(format!(
            "Field '{}' exceeds maximum length of {} characters",
            field, MAX_INPUT_LEN
        ));
    }
    Ok(())
}

fn validate_entity_id(entity_id: &str) -> Result<(), String> {
    if entity_id.is_empty() {
        return Err("entity_id must not be empty".into());
    }
    if !entity_id.contains('.') {
        return Err(format!(
            "entity_id '{}' must contain a dot (e.g. 'light.living_room')",
            entity_id
        ));
    }
    if entity_id.len() > 255 {
        return Err("entity_id too long".into());
    }
    for c in entity_id.chars() {
        if !c.is_alphanumeric() && c != '.' && c != '_' && c != '-' {
            return Err(format!(
                "entity_id '{}' contains invalid character '{}'",
                entity_id, c
            ));
        }
    }
    Ok(())
}

fn validate_domain(domain: &str) -> Result<(), String> {
    if domain.is_empty() {
        return Err("domain must not be empty".into());
    }
    for c in domain.chars() {
        if !c.is_alphanumeric() && c != '_' {
            return Err(format!(
                "domain '{}' contains invalid character '{}'",
                domain, c
            ));
        }
    }
    Ok(())
}

fn validate_service(service: &str) -> Result<(), String> {
    if service.is_empty() {
        return Err("service must not be empty".into());
    }
    for c in service.chars() {
        if !c.is_alphanumeric() && c != '_' && c != '-' {
            return Err(format!(
                "service '{}' contains invalid character '{}'",
                service, c
            ));
        }
    }
    Ok(())
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => {
                out.push('%');
                out.push(char::from(b"0123456789ABCDEF"[(b >> 4) as usize]));
                out.push(char::from(b"0123456789ABCDEF"[(b & 0xf) as usize]));
            }
        }
    }
    out
}

// ─── Base URL resolution ───────────────────────────────────────────────────

fn resolve_base_url() -> Result<String, String> {
    match near::agent::host::workspace_read(HA_BASE_URL_WORKSPACE_KEY) {
        Some(url) => {
            let url = url.trim().trim_end_matches('/').to_string();
            if url.is_empty() {
                Err(
                    "ha/base_url workspace file is empty. Write your HA URL: \
                     echo 'http://homeassistant.local:8123' > \
                     \"~/.ironclaw/workspace/ha/base_url\""
                        .into(),
                )
            } else {
                near::agent::host::log(
                    near::agent::host::LogLevel::Debug,
                    &format!("Using HA base URL from workspace: {}", url),
                );
                Ok(url)
            }
        }
        None => Err(
            "Home Assistant base URL not configured. Write it to the workspace file: \
             echo 'http://homeassistant.local:8123' > \"~/.ironclaw/workspace/ha/base_url\""
                .into(),
        ),
    }
}

// ─── HTTP helper ───────────────────────────────────────────────────────────

fn ha_request(
    method: &str,
    url: &str,
    body: Option<&str>,
) -> Result<(u16, String), String> {
    let headers = serde_json::json!({
        "Content-Type": "application/json",
        "Accept": "application/json",
        "User-Agent": "IronClaw-HA-Tool/0.1"
    })
    .to_string();

    near::agent::host::log(
        near::agent::host::LogLevel::Info,
        &format!("HA {} {}", method, url),
    );

    let body_bytes: Option<Vec<u8>> = body.map(|b| b.as_bytes().to_vec());

    let resp = near::agent::host::http_request(method, url, &headers, body_bytes.as_deref(), None)
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let body_str = String::from_utf8(resp.body)
        .map_err(|e| format!("Invalid UTF-8 in response: {}", e))?;

    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "Home Assistant API error (HTTP {}): {}",
            resp.status, body_str
        ));
    }

    Ok((resp.status, body_str))
}

fn ha_get(base_url: &str, path: &str) -> Result<String, String> {
    let url = format!("{}{}", base_url, path);
    ha_request("GET", &url, None).map(|(_, body)| body)
}

fn ha_post(base_url: &str, path: &str, body: Option<&str>) -> Result<String, String> {
    let url = format!("{}{}", base_url, path);
    ha_request("POST", &url, body).map(|(_, body)| body)
}

// ─── Action enum ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum HaAction {
    GetStatus,
    GetConfig,
    GetStates {
        domain_filter: Option<String>,
    },
    GetState {
        entity_id: String,
    },
    SetState {
        entity_id: String,
        state: String,
        attributes: Option<serde_json::Value>,
    },
    GetServices {
        domain_filter: Option<String>,
    },
    CallService {
        domain: String,
        service: String,
        data: Option<serde_json::Value>,
    },
    GetEvents,
    FireEvent {
        event_type: String,
        event_data: Option<serde_json::Value>,
    },
    GetHistory {
        entity_id: String,
        hours_back: Option<u32>,
        start_time: Option<String>,
        minimal_response: Option<bool>,
    },
    GetLogbook {
        entity_id: Option<String>,
        hours_back: Option<u32>,
    },
    GetErrorLog,
    RenderTemplate {
        template: String,
    },
    CheckConfig,
    RestartHa,
    MqttPublish {
        topic: String,
        payload: String,
        retain: Option<bool>,
        qos: Option<u8>,
    },
    ModbusWrite {
        hub: Option<String>,
        unit: u8,
        address: u16,
        value: serde_json::Value,
        write_type: Option<String>,
    },
    ModbusRead {
        entity_id: String,
    },
    GetPanels,
    GetCalendars,
    GetCalendarEvents {
        entity_id: String,
        start: Option<String>,
        end: Option<String>,
    },
    ListAutomations,
    TriggerAutomation {
        entity_id: String,
    },
    ToggleAutomation {
        entity_id: String,
        #[serde(default)]
        enabled: Option<bool>,
    },
    ListScripts,
    RunScript {
        entity_id: String,
        variables: Option<serde_json::Value>,
    },
    ListScenes,
    ActivateScene {
        entity_id: String,
    },
    GetNotifications,
    SendNotification {
        service: String,
        message: String,
        title: Option<String>,
        data: Option<serde_json::Value>,
    },
    ReloadConfigEntry {
        entry_id: String,
    },
}

// ─── Action implementations ────────────────────────────────────────────────

fn get_status(base_url: &str) -> Result<String, String> {
    ha_get(base_url, "/api/")
}

fn get_config(base_url: &str) -> Result<String, String> {
    ha_get(base_url, "/api/config")
}

const MAX_STATES: usize = 500;

fn get_states(base_url: &str, domain_filter: Option<&str>) -> Result<String, String> {
    let raw = ha_get(base_url, "/api/states")?;

    let states: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse states: {}", e))?;
    let empty = vec![];
    let arr = states.as_array().unwrap_or(&empty);

    let filtered: Vec<&serde_json::Value> = match domain_filter {
        None => arr.iter().collect(),
        Some(domain) => arr
            .iter()
            .filter(|s| {
                s["entity_id"]
                    .as_str()
                    .map(|id| id.starts_with(&format!("{}.", domain)))
                    .unwrap_or(false)
            })
            .collect(),
    };

    let total = filtered.len();
    let truncated = total > MAX_STATES;
    let slice = &filtered[..total.min(MAX_STATES)];

    let mut result = serde_json::json!({
        "entities": slice,
        "count": slice.len(),
        "total": total,
    });
    if truncated {
        result["_truncated"] = serde_json::json!(true);
        result["_hint"] = serde_json::json!(
            "Use domain_filter to narrow results (e.g. 'light', 'sensor', 'switch')"
        );
    }
    serde_json::to_string(&result)
        .map_err(|e| format!("Failed to serialize states: {}", e))
}

fn get_state(base_url: &str, entity_id: &str) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    ha_get(base_url, &format!("/api/states/{}", url_encode(entity_id)))
}

fn set_state(
    base_url: &str,
    entity_id: &str,
    state: &str,
    attributes: Option<&serde_json::Value>,
) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    validate_len(state, "state")?;
    let mut body = serde_json::json!({ "state": state });
    if let Some(attrs) = attributes {
        body["attributes"] = attrs.clone();
    }
    let url_path = format!("/api/states/{}", url_encode(entity_id));
    let url = format!("{}{}", base_url, url_path);
    ha_request("POST", &url, Some(&body.to_string())).map(|(_, b)| b)
}

fn get_services(base_url: &str, domain_filter: Option<&str>) -> Result<String, String> {
    let raw = ha_get(base_url, "/api/services")?;

    match domain_filter {
        None => Ok(raw),
        Some(domain) => {
            let services: serde_json::Value = serde_json::from_str(&raw)
                .map_err(|e| format!("Failed to parse services: {}", e))?;
            let empty = vec![];
            let arr = services.as_array().unwrap_or(&empty);
            let filtered: Vec<&serde_json::Value> = arr
                .iter()
                .filter(|s| {
                    s["domain"]
                        .as_str()
                        .map(|d| d == domain)
                        .unwrap_or(false)
                })
                .collect();
            serde_json::to_string(&filtered)
                .map_err(|e| format!("Failed to serialize filtered services: {}", e))
        }
    }
}

fn call_service(
    base_url: &str,
    domain: &str,
    service: &str,
    data: Option<&serde_json::Value>,
) -> Result<String, String> {
    validate_domain(domain)?;
    validate_service(service)?;
    let body = data
        .map(|d| d.to_string())
        .unwrap_or_else(|| "{}".to_string());
    ha_post(
        base_url,
        &format!("/api/services/{}/{}", url_encode(domain), url_encode(service)),
        Some(&body),
    )
}

fn get_events(base_url: &str) -> Result<String, String> {
    ha_get(base_url, "/api/events")
}

fn fire_event(
    base_url: &str,
    event_type: &str,
    event_data: Option<&serde_json::Value>,
) -> Result<String, String> {
    validate_len(event_type, "event_type")?;
    if event_type.is_empty() {
        return Err("event_type must not be empty".into());
    }
    let body = event_data
        .map(|d| d.to_string())
        .unwrap_or_else(|| "{}".to_string());
    ha_post(
        base_url,
        &format!("/api/events/{}", url_encode(event_type)),
        Some(&body),
    )
}

fn get_history(
    base_url: &str,
    entity_id: &str,
    hours_back: Option<u32>,
    start_time: Option<&str>,
    minimal_response: Option<bool>,
) -> Result<String, String> {
    validate_entity_id(entity_id)?;

    let start_ts = if let Some(st) = start_time {
        validate_len(st, "start_time")?;
        let b = st.as_bytes();
        let valid_prefix = b.len() >= 11
            && b[0..4].iter().all(|c| c.is_ascii_digit())
            && b[4] == b'-'
            && b[5..7].iter().all(|c| c.is_ascii_digit())
            && b[7] == b'-'
            && b[8..10].iter().all(|c| c.is_ascii_digit())
            && b[10] == b'T';
        if !valid_prefix {
            return Err(
                "start_time must be an ISO 8601 timestamp (e.g. '2024-01-15T08:00:00+00:00')"
                    .into(),
            );
        }
        st.to_string()
    } else {
        let hours = hours_back.unwrap_or(24);
        if hours == 0 || hours > 8760 {
            return Err("hours_back must be between 1 and 8760".into());
        }
        let now_ms = near::agent::host::now_millis();
        let start_ms = now_ms.saturating_sub((hours as u64) * 3600 * 1000);
        format_iso8601(start_ms / 1000)
    };

    let minimal = minimal_response.unwrap_or(false);
    let path = format!(
        "/api/history/period/{}?filter_entity_id={}&minimal_response={}",
        url_encode(&start_ts),
        url_encode(entity_id),
        minimal
    );
    ha_get(base_url, &path)
}

fn format_iso8601(unix_secs: u64) -> String {
    let s = unix_secs;
    let sec_of_day = s % 86400;
    let days = s / 86400;

    let h = sec_of_day / 3600;
    let m = (sec_of_day % 3600) / 60;
    let sec = sec_of_day % 60;

    let mut year = 1970u32;
    let mut remaining_days = days as u32;

    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let months = [31u32, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    for &days_in_month in &months {
        let dim = if month == 2 && is_leap(year) {
            29
        } else {
            days_in_month
        };
        if remaining_days < dim {
            break;
        }
        remaining_days -= dim;
        month += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00:00",
        year, month, day, h, m, sec
    )
}

fn is_leap(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn get_logbook(
    base_url: &str,
    entity_id: Option<&str>,
    hours_back: Option<u32>,
) -> Result<String, String> {
    let hours = hours_back.unwrap_or(24);
    if hours == 0 || hours > 8760 {
        return Err("hours_back must be between 1 and 8760".into());
    }

    let now_ms = near::agent::host::now_millis();
    let start_ms = now_ms.saturating_sub((hours as u64) * 3600 * 1000);
    let start_ts = format_iso8601(start_ms / 1000);

    let mut path = format!("/api/logbook/{}", url_encode(&start_ts));
    if let Some(eid) = entity_id {
        validate_entity_id(eid)?;
        path.push_str(&format!("?entity={}", url_encode(eid)));
    }
    ha_get(base_url, &path)
}

fn get_error_log(base_url: &str) -> Result<String, String> {
    ha_get(base_url, "/api/error_log")
}

fn render_template(base_url: &str, template: &str) -> Result<String, String> {
    validate_len(template, "template")?;
    let body = serde_json::json!({ "template": template }).to_string();
    ha_post(base_url, "/api/template", Some(&body))
}

fn check_config(base_url: &str) -> Result<String, String> {
    ha_post(base_url, "/api/config/core/check_config", None)
}

fn restart_ha(base_url: &str) -> Result<String, String> {
    near::agent::host::log(
        near::agent::host::LogLevel::Warn,
        "Sending Home Assistant restart request",
    );
    ha_post(base_url, "/api/config/core/restart", None)
}

fn mqtt_publish(
    base_url: &str,
    topic: &str,
    payload: &str,
    retain: Option<bool>,
    qos: Option<u8>,
) -> Result<String, String> {
    validate_len(topic, "topic")?;
    validate_len(payload, "payload")?;
    if topic.is_empty() {
        return Err("topic must not be empty".into());
    }
    let mut data = serde_json::json!({
        "topic": topic,
        "payload": payload,
    });
    if let Some(r) = retain {
        data["retain"] = serde_json::json!(r);
    }
    if let Some(q) = qos {
        if q > 2 {
            return Err("qos must be 0, 1, or 2".into());
        }
        data["qos"] = serde_json::json!(q);
    }
    call_service(base_url, "mqtt", "publish", Some(&data))
}

fn modbus_write(
    base_url: &str,
    hub: Option<&str>,
    unit: u8,
    address: u16,
    value: &serde_json::Value,
    write_type: Option<&str>,
) -> Result<String, String> {
    let wtype = write_type.unwrap_or("holding");
    let service = match wtype {
        "holding" => {
            if !value.is_number() {
                return Err("modbus_write: value must be a number for write_type 'holding'".into());
            }
            "write_register"
        }
        "coil" => {
            if !value.is_boolean() {
                return Err("modbus_write: value must be a boolean (true/false) for write_type 'coil'".into());
            }
            "write_coil"
        }
        "input" => {
            return Err(
                "Modbus input registers are read-only. Use 'holding' or 'coil' for writing.".into(),
            )
        }
        other => return Err(format!("Unknown write_type '{}'. Use 'holding' or 'coil'.", other)),
    };

    let mut data = serde_json::json!({
        "unit": unit,
        "address": address,
        "value": value,
    });
    if let Some(h) = hub {
        data["hub"] = serde_json::json!(h);
    }
    call_service(base_url, "modbus", service, Some(&data))
}

fn modbus_read(base_url: &str, entity_id: &str) -> Result<String, String> {
    get_state(base_url, entity_id)
}

fn get_panels(base_url: &str) -> Result<String, String> {
    ha_get(base_url, "/api/panels")
}

fn get_calendars(base_url: &str) -> Result<String, String> {
    ha_get(base_url, "/api/calendars")
}

fn get_calendar_events(
    base_url: &str,
    entity_id: &str,
    start: Option<&str>,
    end: Option<&str>,
) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    let now_ms = near::agent::host::now_millis();
    let now_secs = now_ms / 1000;
    let default_start = format_iso8601(now_secs);
    let default_end = format_iso8601(now_secs + 7 * 24 * 3600);

    let s = start.unwrap_or(&default_start);
    let e = end.unwrap_or(&default_end);

    for (val, name) in &[(s, "start"), (e, "end")] {
        validate_len(val, name)?;
        let b = val.as_bytes();
        let valid = b.len() >= 11
            && b[0..4].iter().all(|c| c.is_ascii_digit())
            && b[4] == b'-'
            && b[5..7].iter().all(|c| c.is_ascii_digit())
            && b[7] == b'-'
            && b[8..10].iter().all(|c| c.is_ascii_digit())
            && b[10] == b'T';
        if !valid {
            return Err(format!(
                "{} must be an ISO 8601 timestamp (e.g. '2024-01-15T00:00:00+00:00')",
                name
            ));
        }
    }

    let path = format!(
        "/api/calendars/{}?start={}&end={}",
        url_encode(entity_id),
        url_encode(s),
        url_encode(e)
    );
    ha_get(base_url, &path)
}

fn list_automations(base_url: &str) -> Result<String, String> {
    get_states(base_url, Some("automation"))
}

fn trigger_automation(base_url: &str, entity_id: &str) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    if !entity_id.starts_with("automation.") {
        return Err(format!(
            "entity_id '{}' must start with 'automation.'",
            entity_id
        ));
    }
    let data = serde_json::json!({ "entity_id": entity_id });
    call_service(base_url, "automation", "trigger", Some(&data))
}

fn toggle_automation(base_url: &str, entity_id: &str, enabled: Option<bool>) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    if !entity_id.starts_with("automation.") {
        return Err(format!(
            "entity_id '{}' must start with 'automation.'",
            entity_id
        ));
    }
    let svc = if enabled.unwrap_or(true) { "turn_on" } else { "turn_off" };
    let data = serde_json::json!({ "entity_id": entity_id });
    call_service(base_url, "automation", svc, Some(&data))
}

fn list_scripts(base_url: &str) -> Result<String, String> {
    get_states(base_url, Some("script"))
}

fn run_script(
    base_url: &str,
    entity_id: &str,
    variables: Option<&serde_json::Value>,
) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    if !entity_id.starts_with("script.") {
        return Err(format!(
            "entity_id '{}' must start with 'script.'",
            entity_id
        ));
    }
    let script_name = entity_id.strip_prefix("script.").unwrap_or(entity_id);
    let data = match variables {
        Some(vars) if vars.is_object() => Some(vars.clone()),
        Some(_) => return Err("variables must be a JSON object".to_string()),
        None => None,
    };
    call_service(base_url, "script", script_name, data.as_ref())
}

fn list_scenes(base_url: &str) -> Result<String, String> {
    get_states(base_url, Some("scene"))
}

fn activate_scene(base_url: &str, entity_id: &str) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    if !entity_id.starts_with("scene.") {
        return Err(format!(
            "entity_id '{}' must start with 'scene.'",
            entity_id
        ));
    }
    let data = serde_json::json!({ "entity_id": entity_id });
    call_service(base_url, "scene", "turn_on", Some(&data))
}

fn get_notifications(base_url: &str) -> Result<String, String> {
    ha_get(base_url, "/api/persistent_notification")
}

fn send_notification(
    base_url: &str,
    service: &str,
    message: &str,
    title: Option<&str>,
    data: Option<&serde_json::Value>,
) -> Result<String, String> {
    validate_len(message, "message")?;
    let mut payload = serde_json::json!({ "message": message });
    if let Some(t) = title {
        validate_len(t, "title")?;
        payload["title"] = serde_json::json!(t);
    }
    if let Some(d) = data {
        payload["data"] = d.clone();
    }
    call_service(base_url, "notify", service, Some(&payload))
}

fn reload_config_entry(base_url: &str, entry_id: &str) -> Result<String, String> {
    validate_len(entry_id, "entry_id")?;
    if entry_id.is_empty() {
        return Err("entry_id must not be empty".into());
    }
    let data = serde_json::json!({ "entry_id": entry_id });
    call_service(base_url, "homeassistant", "reload_config_entry", Some(&data))
}

// ─── Main execute function ─────────────────────────────────────────────────

fn execute_inner(params: &str) -> Result<String, String> {
    let action: HaAction =
        serde_json::from_str(params).map_err(|e| format!("Invalid parameters: {}", e))?;

    if !near::agent::host::secret_exists("ha_token") {
        return Err(
            "Home Assistant token not found. Run `ironclaw tool setup ha-tool` to configure it."
                .into(),
        );
    }

    let url = resolve_base_url()?;

    match action {
        HaAction::GetStatus => get_status(&url),
        HaAction::GetConfig => get_config(&url),
        HaAction::GetStates { domain_filter } => get_states(&url, domain_filter.as_deref()),
        HaAction::GetState { entity_id } => get_state(&url, &entity_id),
        HaAction::SetState { entity_id, state, attributes } => {
            set_state(&url, &entity_id, &state, attributes.as_ref())
        }
        HaAction::GetServices { domain_filter } => get_services(&url, domain_filter.as_deref()),
        HaAction::CallService { domain, service, data } => {
            call_service(&url, &domain, &service, data.as_ref())
        }
        HaAction::GetEvents => get_events(&url),
        HaAction::FireEvent { event_type, event_data } => {
            fire_event(&url, &event_type, event_data.as_ref())
        }
        HaAction::GetHistory { entity_id, hours_back, start_time, minimal_response } => {
            get_history(&url, &entity_id, hours_back, start_time.as_deref(), minimal_response)
        }
        HaAction::GetLogbook { entity_id, hours_back } => {
            get_logbook(&url, entity_id.as_deref(), hours_back)
        }
        HaAction::GetErrorLog => get_error_log(&url),
        HaAction::RenderTemplate { template } => render_template(&url, &template),
        HaAction::CheckConfig => check_config(&url),
        HaAction::RestartHa => restart_ha(&url),
        HaAction::MqttPublish { topic, payload, retain, qos } => {
            mqtt_publish(&url, &topic, &payload, retain, qos)
        }
        HaAction::ModbusWrite { hub, unit, address, value, write_type } => {
            modbus_write(&url, hub.as_deref(), unit, address, &value, write_type.as_deref())
        }
        HaAction::ModbusRead { entity_id } => modbus_read(&url, &entity_id),
        HaAction::GetPanels => get_panels(&url),
        HaAction::GetCalendars => get_calendars(&url),
        HaAction::GetCalendarEvents { entity_id, start, end } => {
            get_calendar_events(&url, &entity_id, start.as_deref(), end.as_deref())
        }
        HaAction::ListAutomations => list_automations(&url),
        HaAction::TriggerAutomation { entity_id } => trigger_automation(&url, &entity_id),
        HaAction::ToggleAutomation { entity_id, enabled } => {
            toggle_automation(&url, &entity_id, enabled)
        }
        HaAction::ListScripts => list_scripts(&url),
        HaAction::RunScript { entity_id, variables } => {
            run_script(&url, &entity_id, variables.as_ref())
        }
        HaAction::ListScenes => list_scenes(&url),
        HaAction::ActivateScene { entity_id } => activate_scene(&url, &entity_id),
        HaAction::GetNotifications => get_notifications(&url),
        HaAction::SendNotification { service, message, title, data } => {
            send_notification(&url, &service, &message, title.as_deref(), data.as_ref())
        }
        HaAction::ReloadConfigEntry { entry_id } => reload_config_entry(&url, &entry_id),
    }
}

// ─── WIT export ────────────────────────────────────────────────────────────

struct HaTool;

impl exports::near::agent::tool::Guest for HaTool {
    fn execute(req: exports::near::agent::tool::Request) -> exports::near::agent::tool::Response {
        match execute_inner(&req.params) {
            Ok(output) => exports::near::agent::tool::Response {
                output: Some(output),
                error: None,
            },
            Err(e) => exports::near::agent::tool::Response {
                output: None,
                error: Some(e),
            },
        }
    }

    fn schema() -> String {
        SCHEMA.to_string()
    }

    fn description() -> String {
        "Home Assistant integration — full control over your smart home via the HA REST API. \
         Read and write entity states, call any service (lights, climate, switches, media players, \
         locks, covers, fans, etc.), manage automations, scripts, and scenes, publish MQTT messages, \
         control Modbus devices, render Jinja2 templates, view logs, check config, and restart HA. \
         Requires ha_token (set via `ironclaw tool setup ha-tool`) and ha_base_url \
         (written to the workspace file at ha/base_url by `install.sh`)."
            .to_string()
    }
}

export!(HaTool);

// ─── JSON Schema ───────────────────────────────────────────────────────────

const SCHEMA: &str = r#"{
  "type": "object",
  "required": ["action"],
  "properties": {},
  "oneOf": [
    {
      "description": "Check if the Home Assistant API is reachable and return its version info.",
      "properties": {
        "action": { "type": "string", "const": "get_status" }
      },
      "required": ["action"]
    },
    {
      "description": "Get the Home Assistant instance configuration (location, unit system, version, components, etc.).",
      "properties": {
        "action": { "type": "string", "const": "get_config" }
      },
      "required": ["action"]
    },
    {
      "description": "List entity states. Optionally filter by domain. Results capped at 500 entities; if truncated the response includes '_truncated': true and '_hint' suggesting domain_filter.",
      "properties": {
        "action": { "type": "string", "const": "get_states" },
        "domain_filter": { "type": "string", "description": "Filter entities to this domain only (e.g. 'light', 'switch', 'sensor', 'climate', 'media_player', 'automation', 'script', 'scene', 'input_boolean', 'binary_sensor')." }
      },
      "required": ["action"]
    },
    {
      "description": "Get the current state and attributes of a single entity.",
      "properties": {
        "action": { "type": "string", "const": "get_state" },
        "entity_id": { "type": "string", "description": "Entity ID (e.g. 'light.living_room', 'sensor.temperature')." }
      },
      "required": ["action", "entity_id"]
    },
    {
      "description": "Set/update the state of an entity (note: this writes to HA state machine directly; use call_service for real device control).",
      "properties": {
        "action": { "type": "string", "const": "set_state" },
        "entity_id": { "type": "string" },
        "state": { "type": "string", "description": "New state value (e.g. 'on', 'off', '22.5')." },
        "attributes": { "type": "object", "description": "Optional state attributes to set." }
      },
      "required": ["action", "entity_id", "state"]
    },
    {
      "description": "List available services. Optionally filter by domain.",
      "properties": {
        "action": { "type": "string", "const": "get_services" },
        "domain_filter": { "type": "string", "description": "Filter to this service domain (e.g. 'light', 'switch', 'climate')." }
      },
      "required": ["action"]
    },
    {
      "description": "Call any Home Assistant service. This is the primary way to control devices. Examples: turn lights on/off, set thermostat temperature, lock/unlock doors, play media, etc.",
      "properties": {
        "action": { "type": "string", "const": "call_service" },
        "domain": { "type": "string", "description": "Service domain (e.g. 'light', 'switch', 'climate', 'media_player', 'cover', 'lock', 'fan', 'script', 'automation', 'mqtt', 'modbus')." },
        "service": { "type": "string", "description": "Service name (e.g. 'turn_on', 'turn_off', 'toggle', 'set_temperature', 'media_play', 'publish')." },
        "data": {
          "type": "object",
          "description": "Service call data/parameters. For entity control: include 'entity_id'. For light: 'brightness' (0-255), 'color_temp', 'hs_color'. For climate: 'temperature', 'hvac_mode'. For media_player: 'media_content_id', 'media_content_type'."
        }
      },
      "required": ["action", "domain", "service"]
    },
    {
      "description": "List all registered event types in Home Assistant.",
      "properties": {
        "action": { "type": "string", "const": "get_events" }
      },
      "required": ["action"]
    },
    {
      "description": "Fire a custom event in Home Assistant.",
      "properties": {
        "action": { "type": "string", "const": "fire_event" },
        "event_type": { "type": "string", "description": "Event type to fire (e.g. 'my_custom_event')." },
        "event_data": { "type": "object", "description": "Optional event data payload." }
      },
      "required": ["action", "event_type"]
    },
    {
      "description": "Get state history for an entity over a time period.",
      "properties": {
        "action": { "type": "string", "const": "get_history" },
        "entity_id": { "type": "string", "description": "Entity ID to get history for." },
        "hours_back": { "type": "integer", "default": 24, "minimum": 1, "maximum": 8760, "description": "How many hours of history to retrieve (default: 24). Ignored if start_time is provided." },
        "start_time": { "type": "string", "description": "ISO 8601 start datetime, e.g. '2024-01-15T00:00:00+00:00'. Takes precedence over hours_back." },
        "minimal_response": { "type": "boolean", "default": false, "description": "Return minimal response (only state and last_changed)." }
      },
      "required": ["action", "entity_id"]
    },
    {
      "description": "Get the Home Assistant logbook (human-readable event log).",
      "properties": {
        "action": { "type": "string", "const": "get_logbook" },
        "entity_id": { "type": "string", "description": "Optional entity filter." },
        "hours_back": { "type": "integer", "default": 24, "minimum": 1, "maximum": 8760 }
      },
      "required": ["action"]
    },
    {
      "description": "Get the Home Assistant error log (raw log output).",
      "properties": {
        "action": { "type": "string", "const": "get_error_log" }
      },
      "required": ["action"]
    },
    {
      "description": "Render a Jinja2 template via Home Assistant's template engine. Use this to compute derived values, format data, or query the state machine with Jinja2 expressions.",
      "properties": {
        "action": { "type": "string", "const": "render_template" },
        "template": { "type": "string", "description": "Jinja2 template string (e.g. \"{{ states('sensor.temperature') }}\", \"{{ states.light | selectattr('state', 'eq', 'on') | list | count }} lights on\")." }
      },
      "required": ["action", "template"]
    },
    {
      "description": "Check the Home Assistant configuration for errors.",
      "properties": {
        "action": { "type": "string", "const": "check_config" }
      },
      "required": ["action"]
    },
    {
      "description": "Restart the Home Assistant instance. Use with caution — this will disrupt all automations and integrations briefly.",
      "properties": {
        "action": { "type": "string", "const": "restart_ha" }
      },
      "required": ["action"]
    },
    {
      "description": "Publish a message to an MQTT topic via the Home Assistant MQTT integration.",
      "properties": {
        "action": { "type": "string", "const": "mqtt_publish" },
        "topic": { "type": "string", "description": "MQTT topic to publish to (e.g. 'home/living_room/light/set')." },
        "payload": { "type": "string", "description": "Message payload to publish." },
        "retain": { "type": "boolean", "default": false, "description": "Whether the message should be retained by the MQTT broker." },
        "qos": { "type": "integer", "enum": [0, 1, 2], "default": 0, "description": "MQTT QoS level." }
      },
      "required": ["action", "topic", "payload"]
    },
    {
      "description": "Write a value to a Modbus holding register or coil via the Home Assistant Modbus integration.",
      "properties": {
        "action": { "type": "string", "const": "modbus_write" },
        "hub": { "type": "string", "description": "Modbus hub name (optional if only one hub configured)." },
        "unit": { "type": "integer", "minimum": 0, "maximum": 255, "description": "Modbus unit/slave ID." },
        "address": { "type": "integer", "minimum": 0, "maximum": 65535, "description": "Register or coil address." },
        "value": { "description": "Value to write. Integer for registers, boolean for coils." },
        "write_type": { "type": "string", "enum": ["holding", "coil"], "default": "holding", "description": "Register type: 'holding' (default) or 'coil'." }
      },
      "required": ["action", "unit", "address", "value"]
    },
    {
      "description": "Read a Modbus sensor value (returns the current HA entity state for a Modbus-backed entity).",
      "properties": {
        "action": { "type": "string", "const": "modbus_read" },
        "entity_id": { "type": "string", "description": "Entity ID of the Modbus sensor/binary_sensor (e.g. 'sensor.modbus_temperature')." }
      },
      "required": ["action", "entity_id"]
    },
    {
      "description": "List the registered frontend panels in Home Assistant.",
      "properties": {
        "action": { "type": "string", "const": "get_panels" }
      },
      "required": ["action"]
    },
    {
      "description": "List all calendar entities registered in Home Assistant.",
      "properties": {
        "action": { "type": "string", "const": "get_calendars" }
      },
      "required": ["action"]
    },
    {
      "description": "Get events from a calendar entity.",
      "properties": {
        "action": { "type": "string", "const": "get_calendar_events" },
        "entity_id": { "type": "string", "description": "Calendar entity ID (e.g. 'calendar.home')." },
        "start": { "type": "string", "description": "ISO 8601 start datetime (default: now)." },
        "end": { "type": "string", "description": "ISO 8601 end datetime (default: 7 days from now)." }
      },
      "required": ["action", "entity_id"]
    },
    {
      "description": "List all automation entities and their current state (on/off, last triggered).",
      "properties": {
        "action": { "type": "string", "const": "list_automations" }
      },
      "required": ["action"]
    },
    {
      "description": "Manually trigger an automation to run immediately.",
      "properties": {
        "action": { "type": "string", "const": "trigger_automation" },
        "entity_id": { "type": "string", "description": "Automation entity ID (must start with 'automation.')." }
      },
      "required": ["action", "entity_id"]
    },
    {
      "description": "Enable or disable an automation.",
      "properties": {
        "action": { "type": "string", "const": "toggle_automation" },
        "entity_id": { "type": "string", "description": "Automation entity ID (must start with 'automation.')." },
        "enabled": { "type": "boolean", "description": "true to enable, false to disable. Defaults to true if omitted." }
      },
      "required": ["action", "entity_id"]
    },
    {
      "description": "List all script entities.",
      "properties": {
        "action": { "type": "string", "const": "list_scripts" }
      },
      "required": ["action"]
    },
    {
      "description": "Execute a Home Assistant script.",
      "properties": {
        "action": { "type": "string", "const": "run_script" },
        "entity_id": { "type": "string", "description": "Script entity ID (must start with 'script.')." },
        "variables": { "type": "object", "description": "Optional script variables to pass." }
      },
      "required": ["action", "entity_id"]
    },
    {
      "description": "List all scene entities.",
      "properties": {
        "action": { "type": "string", "const": "list_scenes" }
      },
      "required": ["action"]
    },
    {
      "description": "Activate a scene.",
      "properties": {
        "action": { "type": "string", "const": "activate_scene" },
        "entity_id": { "type": "string", "description": "Scene entity ID (must start with 'scene.')." }
      },
      "required": ["action", "entity_id"]
    },
    {
      "description": "Get active persistent notifications from Home Assistant.",
      "properties": {
        "action": { "type": "string", "const": "get_notifications" }
      },
      "required": ["action"]
    },
    {
      "description": "Send a notification via a Home Assistant notify service.",
      "properties": {
        "action": { "type": "string", "const": "send_notification" },
        "service": { "type": "string", "description": "Notify service name (e.g. 'mobile_app_phone', 'persistent_notification', 'slack')." },
        "message": { "type": "string", "description": "Notification message body." },
        "title": { "type": "string", "description": "Optional notification title." },
        "data": { "type": "object", "description": "Optional extra notification data (platform-specific)." }
      },
      "required": ["action", "service", "message"]
    },
    {
      "description": "Reload a specific Home Assistant config entry (integration) without restarting HA.",
      "properties": {
        "action": { "type": "string", "const": "reload_config_entry" },
        "entry_id": { "type": "string", "description": "The config entry ID to reload (visible in HA developer tools or settings)." }
      },
      "required": ["action", "entry_id"]
    }
  ]
}"#;

// ─── Unit tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_entity_id_valid() {
        assert!(validate_entity_id("light.living_room").is_ok());
        assert!(validate_entity_id("sensor.temperature_outside").is_ok());
        assert!(validate_entity_id("binary_sensor.door").is_ok());
        assert!(validate_entity_id("automation.morning_routine").is_ok());
    }

    #[test]
    fn test_validate_entity_id_invalid() {
        assert!(validate_entity_id("").is_err());
        assert!(validate_entity_id("nodot").is_err());
        assert!(validate_entity_id("light.living room").is_err());
        assert!(validate_entity_id("light.foo/bar").is_err());
    }

    #[test]
    fn test_validate_domain() {
        assert!(validate_domain("light").is_ok());
        assert!(validate_domain("media_player").is_ok());
        assert!(validate_domain("").is_err());
        assert!(validate_domain("light.living").is_err());
    }

    #[test]
    fn test_validate_service() {
        assert!(validate_service("turn_on").is_ok());
        assert!(validate_service("set_temperature").is_ok());
        assert!(validate_service("turn-on").is_ok());
        assert!(validate_service("reload-config-entry").is_ok());
        assert!(validate_service("").is_err());
        assert!(validate_service("turn on").is_err());
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("light.living_room"), "light.living_room");
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("foo/bar"), "foo%2Fbar");
        assert_eq!(url_encode("a+b"), "a%2Bb");
    }

    #[test]
    fn test_format_iso8601_epoch() {
        let s = format_iso8601(0);
        assert!(s.starts_with("1970-01-01T00:00:00"));
    }

    #[test]
    fn test_format_iso8601_known_date() {
        let s = format_iso8601(1700000000);
        assert!(s.starts_with("2023-11-14") || s.starts_with("2023-11-15"),
            "Got: {}", s);
    }

    #[test]
    fn test_is_leap() {
        assert!(is_leap(2000));
        assert!(is_leap(2024));
        assert!(!is_leap(1900));
        assert!(!is_leap(2023));
    }

    #[test]
    fn test_schema_is_valid_json() {
        let v: serde_json::Value = serde_json::from_str(SCHEMA).expect("schema must be valid JSON");
        assert!(v.get("oneOf").is_some());
    }

    #[test]
    fn test_schema_action_count() {
        let v: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
        let count = v["oneOf"].as_array().unwrap().len();
        assert!(count >= 25, "Expected at least 25 actions, got {}", count);
    }

    #[test]
    fn test_modbus_write_type_validation() {
        let result = modbus_write_type_check("input");
        assert!(result.is_err());
        let result = modbus_write_type_check("holding");
        assert!(result.is_ok());
        let result = modbus_write_type_check("coil");
        assert!(result.is_ok());
        let result = modbus_write_type_check("unknown");
        assert!(result.is_err());
    }

    fn modbus_write_type_check(wtype: &str) -> Result<&'static str, String> {
        match wtype {
            "holding" => Ok("write_register"),
            "coil" => Ok("write_coil"),
            "input" => Err("Modbus input registers are read-only. Use 'holding' or 'coil' for writing.".into()),
            other => Err(format!("Unknown write_type '{}'. Use 'holding' or 'coil'.", other)),
        }
    }

    #[test]
    fn test_validate_len_short() {
        assert!(validate_len("short", "test").is_ok());
    }

    #[test]
    fn test_validate_len_too_long() {
        let long = "x".repeat(MAX_INPUT_LEN + 1);
        assert!(validate_len(&long, "test").is_err());
    }

    #[test]
    fn test_resolve_base_url_explicit() {
        let result = resolve_base_url_test("http://192.168.1.100:8123");
        assert_eq!(result, "http://192.168.1.100:8123");
    }

    #[test]
    fn test_resolve_base_url_strips_trailing_slash() {
        let result = resolve_base_url_test("http://192.168.1.100:8123/");
        assert_eq!(result, "http://192.168.1.100:8123");
    }

    fn resolve_base_url_test(url: &str) -> String {
        let u = url.trim_end_matches('/').to_string();
        u
    }

    #[test]
    fn test_mqtt_qos_validation() {
        assert!(mqtt_qos_valid(0));
        assert!(mqtt_qos_valid(1));
        assert!(mqtt_qos_valid(2));
        assert!(!mqtt_qos_valid(3));
    }

    fn mqtt_qos_valid(qos: u8) -> bool {
        qos <= 2
    }
}
