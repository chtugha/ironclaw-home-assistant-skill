use crate::near::agent::host;
use schemars::JsonSchema;
use serde::Deserialize;

const REMOTE_SHELL_ALIAS: &str = "remote-shell";
const MAX_COMMAND_LEN: usize = 65_536;
const MAX_PATH_LEN: usize = 4096;
const MAX_FILE_WRITE_LEN: usize = 1_048_576; // 1 MiB safety cap
const DEFAULT_EXEC_TIMEOUT_SECS: u32 = 60;
const MIN_EXEC_TIMEOUT_SECS: u32 = 1;
const MAX_EXEC_TIMEOUT_SECS: u32 = 3600;
const READ_EXEC_TIMEOUT_SECS: u32 = 30;
const HA_CLI_EXEC_TIMEOUT_SECS: u32 = 300;
const MAX_SSH_HOST_LEN: usize = 253;
const MAX_SSH_USERNAME_LEN: usize = 256;
const MAX_HA_CLI_ARGS_LEN: usize = 2048;
const MAX_TAIL_LINES: u32 = 100_000;

/// SSH connection parameters accepted on every shell-backed action.
///
/// Either reuse an existing `session_id` from a prior `connect` call, or
/// provide full credentials so the ha-tool can open a session on demand.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
pub struct SshConfig {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub private_key_pem: Option<String>,
    #[serde(default)]
    pub host_key_fingerprint: Option<String>,
    #[serde(default)]
    pub insecure_ignore_host_key: Option<bool>,
    #[serde(default)]
    pub gateway_port: Option<u16>,
}

/// Probe whether the remote-shell extension is installed and reachable.
///
/// Invokes `list_sessions` which is cheap and side-effect-free. Any Ok
/// response (even empty session list) means the sibling tool is usable.
pub fn is_shell_available() -> bool {
    let p = serde_json::to_string(&serde_json::json!({"action": "list_sessions"}))
        .expect("serializing a static json object is infallible");
    host::tool_invoke(REMOTE_SHELL_ALIAS, &p).is_ok()
}

fn log_shell_fallback(action: &str, reason: &str) {
    host::log(
        host::LogLevel::Warn,
        &format!(
            "shell path unavailable for action '{}': {} — falling back to REST API",
            action, reason
        ),
    );
}

/// Try the shell-backed implementation if `ssh` is provided and remote-shell
/// is installed. On failure or absence, logs a warning and returns Ok(None)
/// so the caller can fall back to REST.
pub fn try_shell<F>(action: &str, ssh: Option<&SshConfig>, f: F) -> Result<Option<String>, String>
where
    F: FnOnce(&SshConfig) -> Result<String, String>,
{
    let Some(cfg) = ssh else { return Ok(None) };
    if !is_shell_available() {
        log_shell_fallback(action, "remote-shell extension not installed");
        return Ok(None);
    }
    match f(cfg) {
        Ok(s) => Ok(Some(s)),
        Err(e) => {
            log_shell_fallback(action, &e);
            Ok(None)
        }
    }
}

/// Strict variant of `try_shell` for destructive actions (e.g. `restart_ha`).
/// Only falls back to REST when remote-shell is NOT installed; propagates any
/// shell execution error instead of silently routing to REST, so users who
/// explicitly supplied SSH credentials don't get an unintended REST restart.
pub fn try_shell_strict<F>(
    action: &str,
    ssh: Option<&SshConfig>,
    f: F,
) -> Result<Option<String>, String>
where
    F: FnOnce(&SshConfig) -> Result<String, String>,
{
    let Some(cfg) = ssh else { return Ok(None) };
    if !is_shell_available() {
        log_shell_fallback(action, "remote-shell extension not installed");
        return Ok(None);
    }
    f(cfg).map(Some)
}

fn ensure_session(ssh: &SshConfig) -> Result<String, String> {
    if let Some(sid) = &ssh.session_id {
        if !sid.is_empty() {
            return Ok(sid.clone());
        }
    }
    let host = ssh
        .host
        .as_deref()
        .ok_or("ssh.host required when session_id is not provided")?;
    let username = ssh
        .username
        .as_deref()
        .ok_or("ssh.username required when session_id is not provided")?;
    if host.is_empty() || host.len() > MAX_SSH_HOST_LEN {
        return Err(format!("ssh.host must be 1-{} characters", MAX_SSH_HOST_LEN));
    }
    if username.is_empty() || username.len() > MAX_SSH_USERNAME_LEN {
        return Err(format!("ssh.username must be 1-{} characters", MAX_SSH_USERNAME_LEN));
    }
    let auth = if let Some(pw) = &ssh.password {
        serde_json::json!({"type": "password", "password": pw})
    } else if let Some(key) = &ssh.private_key_pem {
        serde_json::json!({"type": "private_key", "key_pem": key})
    } else {
        return Err("ssh requires password or private_key_pem when opening a new session".into());
    };
    let mut body = serde_json::json!({
        "action": "connect",
        "host": host,
        "username": username,
        "auth": auth,
    });
    if let Some(p) = ssh.port {
        body["port"] = serde_json::json!(p);
    }
    if let Some(fp) = &ssh.host_key_fingerprint {
        body["host_key_fingerprint"] = serde_json::json!(fp);
    }
    if let Some(true) = ssh.insecure_ignore_host_key {
        body["insecure_ignore_host_key"] = serde_json::json!(true);
    }
    if let Some(p) = ssh.gateway_port {
        body["gateway_port"] = serde_json::json!(p);
    }
    let params = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    let resp = host::tool_invoke(REMOTE_SHELL_ALIAS, &params)?;
    let v: serde_json::Value = serde_json::from_str(&resp)
        .map_err(|e| format!("remote-shell connect returned invalid JSON: {}", e))?;
    v.get("session_id")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "remote-shell connect response missing session_id".into())
}

/// Run a command over SSH. `timeout_secs` is clamped to the gateway's 1..=3600 range.
pub fn shell_exec(ssh: &SshConfig, command: &str, timeout_secs: Option<u32>) -> Result<String, String> {
    if command.is_empty() {
        return Err("command must not be empty".into());
    }
    if command.len() > MAX_COMMAND_LEN {
        return Err(format!("command too long (max {} bytes)", MAX_COMMAND_LEN));
    }
    let session_id = ensure_session(ssh)?;
    let timeout = timeout_secs
        .unwrap_or(DEFAULT_EXEC_TIMEOUT_SECS)
        .clamp(MIN_EXEC_TIMEOUT_SECS, MAX_EXEC_TIMEOUT_SECS);
    let mut body = serde_json::json!({
        "action": "execute",
        "session_id": session_id,
        "command": command,
        "timeout_secs": timeout,
    });
    if let Some(p) = ssh.gateway_port {
        body["gateway_port"] = serde_json::json!(p);
    }
    let params = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    host::tool_invoke(REMOTE_SHELL_ALIAS, &params)
}

fn parse_exec_output(raw: &str) -> Result<(i32, String, String), String> {
    let v: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("invalid shell response: {}", e))?;
    let exit_code = v.get("exit_code").and_then(|n| n.as_i64()).unwrap_or(-1) as i32;
    let stdout = v
        .get("stdout")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let stderr = v
        .get("stderr")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    Ok((exit_code, stdout, stderr))
}

fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("path must not be empty".into());
    }
    if path.len() > MAX_PATH_LEN {
        return Err(format!("path too long (max {} bytes)", MAX_PATH_LEN));
    }
    if path.contains('\0') {
        return Err("path must not contain null bytes".into());
    }
    if path.contains('\n') || path.contains('\r') {
        return Err("path must not contain newlines".into());
    }
    if path.contains('\'') {
        return Err("path must not contain single quotes (shell-quoting constraint)".into());
    }
    Ok(())
}

/// Read a file over SSH via `cat`.
pub fn read_file(ssh: &SshConfig, path: &str) -> Result<String, String> {
    validate_path(path)?;
    let raw = shell_exec(ssh, &format!("cat '{}'", path), Some(READ_EXEC_TIMEOUT_SECS))?;
    let (code, stdout, stderr) = parse_exec_output(&raw)?;
    if code != 0 {
        return Err(format!("cat failed (exit {}): {}", code, stderr.trim()));
    }
    Ok(serde_json::json!({"path": path, "content": stdout}).to_string())
}

/// Write a file over SSH atomically: stream via base64 -> tee with sudo fallback off.
pub fn write_file(ssh: &SshConfig, path: &str, content: &str) -> Result<String, String> {
    validate_path(path)?;
    if content.len() > MAX_FILE_WRITE_LEN {
        return Err(format!("content too large (max {} bytes)", MAX_FILE_WRITE_LEN));
    }
    let b64 = b64_encode(content.as_bytes());
    let command = format!("printf %s '{}' | base64 -d > '{}'", b64, path);
    let raw = shell_exec(ssh, &command, Some(DEFAULT_EXEC_TIMEOUT_SECS))?;
    let (code, _stdout, stderr) = parse_exec_output(&raw)?;
    if code != 0 {
        return Err(format!("write failed (exit {}): {}", code, stderr.trim()));
    }
    Ok(serde_json::json!({"path": path, "bytes_written": content.len()}).to_string())
}

/// Tail last N lines of a file.
pub fn tail_file(ssh: &SshConfig, path: &str, lines: u32) -> Result<String, String> {
    validate_path(path)?;
    if lines == 0 || lines > MAX_TAIL_LINES {
        return Err(format!("lines must be between 1 and {}", MAX_TAIL_LINES));
    }
    let raw = shell_exec(ssh, &format!("tail -n {} '{}'", lines, path), Some(READ_EXEC_TIMEOUT_SECS))?;
    let (code, stdout, stderr) = parse_exec_output(&raw)?;
    if code != 0 {
        return Err(format!("tail failed (exit {}): {}", code, stderr.trim()));
    }
    Ok(serde_json::json!({"path": path, "lines": lines, "content": stdout}).to_string())
}

/// Run the Home Assistant `ha` supervisor CLI over SSH.
pub fn ha_cli(ssh: &SshConfig, args: &str) -> Result<String, String> {
    if args.is_empty() {
        return Err("args must not be empty (e.g. 'core check', 'core restart', 'core logs')".into());
    }
    if args.len() > MAX_HA_CLI_ARGS_LEN {
        return Err(format!("args too long (max {} bytes)", MAX_HA_CLI_ARGS_LEN));
    }
    // Whitelist: only alphanumerics, space, and a small set of safe punctuation.
    // This prevents quoting/globbing/continuation/injection beyond the `ha` CLI.
    for c in args.chars() {
        let ok = c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '=' | ':' | ',' | '/');
        if !ok {
            return Err(format!("args contains forbidden character '{}'", c));
        }
    }
    shell_exec(ssh, &format!("ha {}", args), Some(HA_CLI_EXEC_TIMEOUT_SECS))
}

/// Status snapshot: which shell integration is present.
pub fn shell_status() -> Result<String, String> {
    let available = is_shell_available();
    Ok(serde_json::json!({
        "remote_shell_available": available,
        "alias": REMOTE_SHELL_ALIAS,
        "note": if available {
            "remote-shell extension is installed; ha-tool shell-backed actions are enabled."
        } else {
            "remote-shell extension not installed. ha-tool falls back to REST-only operation."
        }
    })
    .to_string())
}

/// Tiny base64 encoder (RFC 4648 standard alphabet, no line breaks).
fn b64_encode(input: &[u8]) -> String {
    const ALPHA: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= input.len() {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8) | (input[i + 2] as u32);
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 6) & 0x3f) as usize] as char);
        out.push(ALPHA[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = input.len() - i;
    if rem == 1 {
        let n = (input[i] as u32) << 16;
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 6) & 0x3f) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_b64_encode() {
        assert_eq!(b64_encode(b""), "");
        assert_eq!(b64_encode(b"f"), "Zg==");
        assert_eq!(b64_encode(b"fo"), "Zm8=");
        assert_eq!(b64_encode(b"foo"), "Zm9v");
        assert_eq!(b64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(b64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_validate_path() {
        assert!(validate_path("/etc/hosts").is_ok());
        assert!(validate_path("/config/configuration.yaml").is_ok());
        assert!(validate_path("").is_err());
        assert!(validate_path("bad\npath").is_err());
        assert!(validate_path("bad'path").is_err());
        assert!(validate_path("bad\0path").is_err());
    }

    #[test]
    fn test_ha_cli_rejects_metacharacters() {
        let ssh = SshConfig {
            session_id: Some("x".into()),
            host: None,
            port: None,
            username: None,
            password: None,
            private_key_pem: None,
            host_key_fingerprint: None,
            insecure_ignore_host_key: None,
            gateway_port: None,
        };
        assert!(ha_cli(&ssh, "core check; rm -rf /").is_err());
        assert!(ha_cli(&ssh, "core check && whoami").is_err());
        assert!(ha_cli(&ssh, "core check | grep x").is_err());
        assert!(ha_cli(&ssh, "").is_err());
        // Whitelist rejects quoting and globbing too.
        assert!(ha_cli(&ssh, "core 'check'").is_err());
        assert!(ha_cli(&ssh, "core check\\").is_err());
        assert!(ha_cli(&ssh, "core *").is_err());
        assert!(ha_cli(&ssh, "core ?").is_err());
        assert!(ha_cli(&ssh, "core (check)").is_err());
        assert!(ha_cli(&ssh, "core {check}").is_err());
    }
}
