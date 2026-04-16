use crate::near::agent::host;
use crate::types::StatesResponse;

const MAX_STATES: usize = 500;
const MAX_HOURS_BACK: u32 = 8760;
const MAX_ENTITY_ID_LEN: usize = 255;
const MAX_EVENT_TYPE_LEN: usize = 255;
const MAX_STATE_LEN: usize = 255;

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(char::from(b"0123456789ABCDEF"[(b >> 4) as usize]));
                out.push(char::from(b"0123456789ABCDEF"[(b & 0xf) as usize]));
            }
        }
    }
    out
}

fn validate_ha_url(ha_url: &str) -> Result<(), String> {
    let lower = ha_url.to_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err("ha_url must start with http:// or https://".into());
    }
    let host_part = if let Some(s) = lower.strip_prefix("http://") {
        s
    } else {
        lower.strip_prefix("https://").unwrap_or("")
    };
    let host = host_part.split('/').next().unwrap_or("");
    let host_no_port = host.split(':').next().unwrap_or("");
    if host_no_port.is_empty() {
        return Err("ha_url must contain a hostname".into());
    }
    let is_private = host_no_port == "localhost"
        || host_no_port == "127.0.0.1"
        || is_private_ip(host_no_port, "192.168.")
        || is_private_ip(host_no_port, "10.")
        || is_private_172(host_no_port)
        || host_no_port.ends_with(".local")
        || host_no_port.ends_with(".internal")
        || host_no_port.ends_with(".lan")
        || host_no_port.ends_with(".home")
        || host_no_port.ends_with(".duckdns.org")
        || host_no_port.ends_with(".nabu.casa");
    if !is_private {
        return Err(format!(
            "ha_url host '{}' is not a recognized private/local address. \
             Allowed: localhost, 127.0.0.1, 192.168.*, 10.*, 172.16-31.*, \
             *.local, *.internal, *.lan, *.home, *.duckdns.org, *.nabu.casa",
            host_no_port
        ));
    }
    Ok(())
}

fn is_ip_only(s: &str) -> bool {
    if s.is_empty() || s.starts_with('.') || s.ends_with('.') || s.contains("..") {
        return false;
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()))
}

fn is_private_ip(host: &str, prefix: &str) -> bool {
    host.starts_with(prefix) && is_ip_only(host)
}

fn is_private_172(host: &str) -> bool {
    if let Some(rest) = host.strip_prefix("172.") {
        if !is_ip_only(host) {
            return false;
        }
        if let Some(second) = rest.split('.').next() {
            if let Ok(n) = second.parse::<u8>() {
                return (16..=31).contains(&n);
            }
        }
    }
    false
}

fn normalize_url(ha_url: &str) -> String {
    ha_url.trim_end_matches('/').to_string()
}

fn validate_entity_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("entity_id must not be empty".into());
    }
    if !id.contains('.') {
        return Err(format!("entity_id '{}' must contain a dot (e.g. 'light.living_room')", id));
    }
    if id.len() > MAX_ENTITY_ID_LEN {
        return Err("entity_id too long".into());
    }
    for c in id.chars() {
        if !c.is_alphanumeric() && c != '.' && c != '_' && c != '-' {
            return Err(format!("entity_id contains invalid character '{}'", c));
        }
    }
    Ok(())
}

fn validate_domain(d: &str) -> Result<(), String> {
    if d.is_empty() {
        return Err("domain must not be empty".into());
    }
    for c in d.chars() {
        if !c.is_alphanumeric() && c != '_' {
            return Err(format!("domain contains invalid character '{}'", c));
        }
    }
    Ok(())
}

fn validate_service(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("service must not be empty".into());
    }
    for c in s.chars() {
        if !c.is_alphanumeric() && c != '_' && c != '-' {
            return Err(format!("service contains invalid character '{}'", c));
        }
    }
    Ok(())
}

fn validate_iso_prefix(s: &str, field: &str) -> Result<(), String> {
    let b = s.as_bytes();
    if b.len() < 11
        || !b[0..4].iter().all(|c| c.is_ascii_digit())
        || b[4] != b'-'
        || !b[5..7].iter().all(|c| c.is_ascii_digit())
        || b[7] != b'-'
        || !b[8..10].iter().all(|c| c.is_ascii_digit())
        || b[10] != b'T'
    {
        return Err(format!("{} must be ISO 8601 format (YYYY-MM-DDThh:mm:ss)", field));
    }
    Ok(())
}

fn ha_get(base: &str, path: &str) -> Result<String, String> {
    validate_ha_url(base)?;
    let url = format!("{}{}", normalize_url(base), path);
    host::log(host::LogLevel::Debug, &format!("GET {}", path));
    let resp = host::http_request("GET", &url, "{}", None, None)?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!("HA API {} returned {}: {}", path, resp.status, String::from_utf8_lossy(&resp.body)));
    }
    String::from_utf8(resp.body).map_err(|e| format!("Invalid UTF-8: {}", e))
}

fn ha_post(base: &str, path: &str, body: Option<&str>) -> Result<String, String> {
    validate_ha_url(base)?;
    let url = format!("{}{}", normalize_url(base), path);
    let body_str = body.unwrap_or("{}");
    let body_bytes = body_str.as_bytes().to_vec();
    host::log(host::LogLevel::Debug, &format!("POST {}", path));
    let resp = host::http_request(
        "POST",
        &url,
        r#"{"Content-Type": "application/json"}"#,
        Some(&body_bytes),
        None,
    )?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!("HA API {} returned {}: {}", path, resp.status, String::from_utf8_lossy(&resp.body)));
    }
    String::from_utf8(resp.body).map_err(|e| format!("Invalid UTF-8: {}", e))
}

pub fn get_status(base: &str) -> Result<String, String> {
    ha_get(base, "/api/")
}

pub fn get_config(base: &str) -> Result<String, String> {
    ha_get(base, "/api/config")
}

pub fn get_states(base: &str, domain_filter: Option<&str>) -> Result<String, String> {
    let raw = ha_get(base, "/api/states")?;
    let all: Vec<serde_json::Value> = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse states: {}", e))?;

    let filtered: Vec<serde_json::Value> = if let Some(domain) = domain_filter {
        let prefix = format!("{}.", domain);
        all.into_iter()
            .filter(|e| {
                e.get("entity_id")
                    .and_then(|v| v.as_str())
                    .map(|id| id.starts_with(&prefix))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        all
    };

    let total = filtered.len();
    let (entities, truncated) = if total > MAX_STATES {
        (filtered[..MAX_STATES].to_vec(), Some(true))
    } else {
        (filtered, None)
    };

    let resp = StatesResponse {
        count: entities.len(),
        total,
        entities,
        truncated,
    };
    serde_json::to_string(&resp).map_err(|e| e.to_string())
}

pub fn get_state(base: &str, entity_id: &str) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    ha_get(base, &format!("/api/states/{}", url_encode(entity_id)))
}

pub fn set_state(base: &str, entity_id: &str, state: &str, attributes: Option<&serde_json::Value>) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    validate_not_empty(state, "state")?;
    if state.len() > MAX_STATE_LEN {
        return Err(format!("state value too long (max {} characters)", MAX_STATE_LEN));
    }
    let mut body = serde_json::json!({"state": state});
    if let Some(attrs) = attributes {
        body["attributes"] = attrs.clone();
    }
    let body_str = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    ha_post(base, &format!("/api/states/{}", url_encode(entity_id)), Some(&body_str))
}

pub fn call_service(base: &str, domain: &str, service: &str, data: Option<&serde_json::Value>) -> Result<String, String> {
    validate_domain(domain)?;
    validate_service(service)?;
    let path = format!("/api/services/{}/{}", url_encode(domain), url_encode(service));
    let body_str = match data {
        Some(d) => serde_json::to_string(d).unwrap_or_else(|_| "{}".to_string()),
        None => "{}".to_string(),
    };
    ha_post(base, &path, Some(&body_str))
}

pub fn get_services(base: &str) -> Result<String, String> {
    ha_get(base, "/api/services")
}

fn validate_event_type(s: &str) -> Result<(), String> {
    if s.is_empty() || s.len() > MAX_EVENT_TYPE_LEN {
        return Err(format!("event_type must be 1-{} characters", MAX_EVENT_TYPE_LEN));
    }
    for c in s.chars() {
        if !c.is_alphanumeric() && c != '_' && c != '.' && c != '-' {
            return Err(format!("event_type contains invalid character '{}'", c));
        }
    }
    Ok(())
}

fn validate_not_empty(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{} must not be empty", field));
    }
    Ok(())
}

pub fn fire_event(base: &str, event_type: &str, event_data: Option<&serde_json::Value>) -> Result<String, String> {
    validate_event_type(event_type)?;
    let path = format!("/api/events/{}", url_encode(event_type));
    let body_str = match event_data {
        Some(d) => serde_json::to_string(d).unwrap_or_else(|_| "{}".to_string()),
        None => "{}".to_string(),
    };
    ha_post(base, &path, Some(&body_str))
}

pub fn render_template(base: &str, template: &str) -> Result<String, String> {
    validate_not_empty(template, "template")?;
    if template.len() > 65536 {
        return Err("template too large (max 64KB)".into());
    }
    let body = serde_json::json!({"template": template});
    let body_str = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    ha_post(base, "/api/template", Some(&body_str))
}

pub fn get_history(base: &str, entity_id: &str, hours_back: u32, start_time: Option<&str>) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    if start_time.is_none() && (hours_back == 0 || hours_back > MAX_HOURS_BACK) {
        return Err(format!("hours_back must be between 1 and {}", MAX_HOURS_BACK));
    }
    let ts = if let Some(st) = start_time {
        validate_iso_prefix(st, "start_time")?;
        st.to_string()
    } else {
        let now_ms = host::now_millis();
        let start_ms = now_ms.saturating_sub((hours_back as u64) * 3600 * 1000);
        let secs = start_ms / 1000;
        let d = secs / 86400;
        let rem = secs % 86400;
        let h = rem / 3600;
        let m = (rem % 3600) / 60;
        let s = rem % 60;
        let (y, mo, day) = days_to_ymd(d as i64);
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, day, h, m, s)
    };
    let path = format!("/api/history/period/{}?filter_entity_id={}", url_encode(&ts), url_encode(entity_id));
    ha_get(base, &path)
}

pub fn get_logbook(base: &str, entity_id: Option<&str>, hours_back: u32) -> Result<String, String> {
    if let Some(eid) = entity_id {
        validate_entity_id(eid)?;
    }
    if hours_back == 0 || hours_back > MAX_HOURS_BACK {
        return Err(format!("hours_back must be between 1 and {}", MAX_HOURS_BACK));
    }
    let now_ms = host::now_millis();
    let start_ms = now_ms.saturating_sub((hours_back as u64) * 3600 * 1000);
    let secs = start_ms / 1000;
    let d = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (y, mo, day) = days_to_ymd(d as i64);
    let ts = format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, day, h, m, s);
    let mut path = format!("/api/logbook/{}", url_encode(&ts));
    if let Some(eid) = entity_id {
        path.push_str(&format!("?entity={}", url_encode(eid)));
    }
    ha_get(base, &path)
}

pub fn get_calendar_events(base: &str, entity_id: &str, start: &str, end: &str) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    validate_iso_prefix(start, "start")?;
    validate_iso_prefix(end, "end")?;
    let path = format!(
        "/api/calendars/{}?start={}&end={}",
        url_encode(entity_id),
        url_encode(start),
        url_encode(end)
    );
    ha_get(base, &path)
}

pub fn toggle_automation(base: &str, entity_id: &str, enabled: bool) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    if !entity_id.starts_with("automation.") {
        return Err(format!("entity_id '{}' must start with 'automation.'", entity_id));
    }
    let service = if enabled { "turn_on" } else { "turn_off" };
    call_service(base, "automation", service, Some(&serde_json::json!({"entity_id": entity_id})))
}

pub fn trigger_automation(base: &str, entity_id: &str) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    if !entity_id.starts_with("automation.") {
        return Err(format!("entity_id '{}' must start with 'automation.'", entity_id));
    }
    call_service(base, "automation", "trigger", Some(&serde_json::json!({"entity_id": entity_id})))
}

pub fn run_script(base: &str, entity_id: &str, variables: Option<&serde_json::Value>) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    if !entity_id.starts_with("script.") {
        return Err(format!("entity_id '{}' must start with 'script.'", entity_id));
    }
    let script_id = entity_id.strip_prefix("script.").unwrap_or(entity_id);
    let data = variables.cloned().unwrap_or(serde_json::json!({}));
    call_service(base, "script", script_id, Some(&data))
}

pub fn activate_scene(base: &str, entity_id: &str) -> Result<String, String> {
    validate_entity_id(entity_id)?;
    if !entity_id.starts_with("scene.") {
        return Err(format!("entity_id '{}' must start with 'scene.'", entity_id));
    }
    call_service(base, "scene", "turn_on", Some(&serde_json::json!({"entity_id": entity_id})))
}

pub fn mqtt_publish(base: &str, topic: &str, payload: &str, qos: Option<u8>, retain: Option<bool>) -> Result<String, String> {
    validate_not_empty(topic, "topic")?;
    if topic.len() > 65535 {
        return Err("topic too long (max 65535 bytes per MQTT spec)".into());
    }
    if topic.contains('\0') {
        return Err("topic must not contain null characters".into());
    }
    let mut data = serde_json::json!({"topic": topic, "payload": payload});
    if let Some(q) = qos {
        if q > 2 {
            return Err("qos must be 0, 1, or 2".into());
        }
        data["qos"] = serde_json::json!(q);
    }
    if let Some(r) = retain {
        data["retain"] = serde_json::json!(r);
    }
    call_service(base, "mqtt", "publish", Some(&data))
}

pub fn modbus_write(base: &str, hub: Option<&str>, unit: u16, address: u16, value: &serde_json::Value, write_type: &str) -> Result<String, String> {
    let mut svc_data = serde_json::json!({"unit": unit, "address": address, "value": value});
    if let Some(h) = hub {
        svc_data["hub"] = serde_json::json!(h);
    }
    match write_type {
        "coil" => {
            if !value.is_boolean() {
                return Err("value must be boolean for coil writes".into());
            }
            call_service(base, "modbus", "write_coil", Some(&svc_data))
        }
        "holding" => {
            if !value.is_number() {
                return Err("value must be a number for holding register writes".into());
            }
            call_service(base, "modbus", "write_register", Some(&svc_data))
        }
        _ => Err(format!("write_type must be 'coil' or 'holding', got '{}'", write_type)),
    }
}

pub fn get_notifications(base: &str) -> Result<String, String> {
    ha_get(base, "/api/persistent_notification")
}

pub fn dismiss_notification(base: &str, notification_id: &str) -> Result<String, String> {
    validate_not_empty(notification_id, "notification_id")?;
    if notification_id.len() > MAX_ENTITY_ID_LEN {
        return Err("notification_id too long".into());
    }
    call_service(base, "persistent_notification", "dismiss", Some(&serde_json::json!({"notification_id": notification_id})))
}

pub fn check_config(base: &str) -> Result<String, String> {
    ha_post(base, "/api/config/core/check_config", Some("{}"))
}

pub fn get_error_log(base: &str) -> Result<String, String> {
    ha_get(base, "/api/error_log")
}

pub fn restart_ha(base: &str) -> Result<String, String> {
    ha_post(base, "/api/services/homeassistant/restart", Some("{}"))
}

fn days_to_ymd(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a/b"), "a%2Fb");
        assert_eq!(url_encode("simple"), "simple");
    }

    #[test]
    fn test_validate_entity_id() {
        assert!(validate_entity_id("light.living_room").is_ok());
        assert!(validate_entity_id("sensor.temp-1").is_ok());
        assert!(validate_entity_id("").is_err());
        assert!(validate_entity_id("nodot").is_err());
        assert!(validate_entity_id("bad;id.x").is_err());
    }

    #[test]
    fn test_validate_domain() {
        assert!(validate_domain("light").is_ok());
        assert!(validate_domain("media_player").is_ok());
        assert!(validate_domain("").is_err());
        assert!(validate_domain("bad.domain").is_err());
    }

    #[test]
    fn test_validate_service() {
        assert!(validate_service("turn_on").is_ok());
        assert!(validate_service("turn-on").is_ok());
        assert!(validate_service("").is_err());
        assert!(validate_service("bad service").is_err());
    }

    #[test]
    fn test_validate_iso_prefix() {
        assert!(validate_iso_prefix("2024-01-15T10:30:00Z", "test").is_ok());
        assert!(validate_iso_prefix("not-a-date", "test").is_err());
        assert!(validate_iso_prefix("short", "test").is_err());
    }

    #[test]
    fn test_validate_ha_url() {
        assert!(validate_ha_url("http://192.168.1.100:8123").is_ok());
        assert!(validate_ha_url("http://homeassistant.local:8123").is_ok());
        assert!(validate_ha_url("https://ha.duckdns.org").is_ok());
        assert!(validate_ha_url("http://10.0.0.1:8123").is_ok());
        assert!(validate_ha_url("http://172.16.0.1:8123").is_ok());
        assert!(validate_ha_url("http://localhost:8123").is_ok());
        assert!(validate_ha_url("http://127.0.0.1:8123").is_ok());
        assert!(validate_ha_url("https://my.nabu.casa").is_ok());
        assert!(validate_ha_url("http://myha.internal:8123").is_ok());
        assert!(validate_ha_url("http://attacker.com").is_err());
        assert!(validate_ha_url("http://evil.example.org").is_err());
        assert!(validate_ha_url("ftp://192.168.1.1").is_err());
        assert!(validate_ha_url("not-a-url").is_err());
        assert!(validate_ha_url("http://192.168.1.1.evil.com").is_err());
        assert!(validate_ha_url("http://10.0.0.1.attacker.com").is_err());
        assert!(validate_ha_url("http://172.16.0.1.evil.com").is_err());
        assert!(validate_ha_url("https://https://foo.local").is_err());
    }

    #[test]
    fn test_is_ip_only() {
        assert!(is_ip_only("192.168.1.1"));
        assert!(is_ip_only("10.0.0.1"));
        assert!(is_ip_only("172.16.0.1"));
        assert!(is_ip_only("255.255.255.255"));
        assert!(!is_ip_only(""));
        assert!(!is_ip_only("."));
        assert!(!is_ip_only("..."));
        assert!(!is_ip_only("192.168."));
        assert!(!is_ip_only("192.168.1"));
        assert!(!is_ip_only("192.168.1.1.1"));
        assert!(!is_ip_only("192.168.1.1.evil.com"));
        assert!(!is_ip_only("abc.def.ghi.jkl"));
        assert!(!is_ip_only("192..168.1"));
    }

    #[test]
    fn test_validate_event_type() {
        assert!(validate_event_type("custom_event").is_ok());
        assert!(validate_event_type("my.event").is_ok());
        assert!(validate_event_type("event123").is_ok());
        assert!(validate_event_type("my-integration-event").is_ok());
        assert!(validate_event_type("").is_err());
        assert!(validate_event_type("bad/event").is_err());
        assert!(validate_event_type("bad event").is_err());
        assert!(validate_event_type(&"x".repeat(256)).is_err());
    }

    #[test]
    fn test_validate_not_empty() {
        assert!(validate_not_empty("value", "field").is_ok());
        assert!(validate_not_empty("", "field").is_err());
    }

    #[test]
    fn test_normalize_url() {
        assert_eq!(normalize_url("http://ha:8123/"), "http://ha:8123");
        assert_eq!(normalize_url("http://ha:8123"), "http://ha:8123");
    }

    #[test]
    fn test_days_to_ymd() {
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
        assert_eq!(days_to_ymd(365), (1971, 1, 1));
        assert_eq!(days_to_ymd(19723), (2024, 1, 1));
    }
}
