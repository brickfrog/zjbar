use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

pub const STATUS_BAR_SCHEMA_VERSION: u64 = 1;
pub const CHOIR_POLL_INTERVAL_MS: u64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoirRole {
    Tl,
    Dev,
    Worker,
    Root,
}

impl ChoirRole {
    pub fn is_top_level(self) -> bool {
        matches!(self, Self::Tl | Self::Root)
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Root => 0,
            Self::Tl => 1,
            Self::Dev => 2,
            Self::Worker => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Codex,
    Claude,
    MoonPilot,
    Gemini,
    CursorAgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Working,
    ReviewOwned,
    ChangesRequested,
    Done,
    Failed,
    Exitable,
    WaitingForRedGate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiRollup {
    Success,
    InProgress,
    Failure,
    Unknown,
    NoChecks,
}

impl CiRollup {
    pub fn symbol(self) -> Option<&'static str> {
        match self {
            Self::Success => Some("ci✓"),
            Self::InProgress => Some("ci…"),
            Self::Failure => Some("ci✗"),
            Self::NoChecks => Some("ci∅"),
            Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusBarPane {
    pub zellij_pane_id: u32,
    pub agent_id: String,
    pub role: ChoirRole,
    pub agent_type: AgentType,
    pub lifecycle: Lifecycle,
    pub pr_number: Option<u32>,
    pub unresolved_threads: u32,
    pub ci_rollup: CiRollup,
    pub attention_needed: bool,
    pub parent_agent_id: Option<String>,
    pub last_activity_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusBarState {
    pub schema_version: u64,
    pub taken_at_ms: u64,
    pub panes: Vec<StatusBarPane>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoirStatus {
    Ready(StatusBarState),
    NoChoir,
    SchemaAhead(u64),
    Invalid(String),
}

impl Default for ChoirStatus {
    fn default() -> Self {
        Self::NoChoir
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusBarError {
    SchemaAhead(u64),
    Server(String),
    Invalid(String),
}

impl fmt::Display for StatusBarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaAhead(version) => write!(f, "schema ahead: {version}"),
            Self::Server(message) => write!(f, "server error: {message}"),
            Self::Invalid(message) => write!(f, "invalid response: {message}"),
        }
    }
}

pub fn status_bar_state_jsonrpc_request() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "zjbar-status-bar-state",
        "method": "tools/call",
        "params": {
            "name": "status_bar_state",
            "arguments": {}
        }
    })
    .to_string()
        + "\n"
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn poll_command(socket_path: &str, initial_cwd: Option<&str>) -> String {
    let request = status_bar_state_jsonrpc_request();
    let request = request.trim_end();
    let initial_cwd = initial_cwd
        .map(|cwd| format!(" ZJBAR_PLUGIN_INITIAL_CWD={}", shell_quote(cwd)))
        .unwrap_or_default();
    let script = r#"import os
import socket
import sys

sock_path = os.environ.get('ZJBAR_CHOIR_SOCKET', '.choir/server.sock')
request = os.environ['ZJBAR_CHOIR_REQUEST'] + '\n'
def candidate_socket_paths(sock_path):
    if os.path.isabs(sock_path):
        return [sock_path]
    candidates = []
    def add(candidate):
        candidates.append(candidate)
    def add_parent_candidates(base):
        if not base:
            return
        current = os.path.abspath(base)
        while True:
            add(os.path.join(current, sock_path))
            parent = os.path.dirname(current)
            if parent == current:
                break
            current = parent
    add(sock_path)
    workspace = os.environ.get('CHOIR_WORKSPACE')
    if workspace:
        add(os.path.join(workspace, sock_path))
    add_parent_candidates(os.environ.get('ZJBAR_PLUGIN_INITIAL_CWD'))
    add_parent_candidates(os.environ.get('PWD'))
    add_parent_candidates(os.getcwd())
    seen = set()
    unique = []
    for candidate in candidates:
        normalized = os.path.abspath(candidate)
        if normalized not in seen:
            seen.add(normalized)
            unique.append(candidate)
    return unique

try:
    sock = None
    last_exc = None
    for candidate in candidate_socket_paths(sock_path):
        try:
            attempt = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            attempt.settimeout(0.35)
            attempt.connect(candidate)
            sock = attempt
            break
        except Exception as exc:
            last_exc = exc
            try:
                attempt.close()
            except Exception:
                pass
    if sock is None:
        raise last_exc or FileNotFoundError(sock_path)
    sock.sendall(request.encode('utf-8'))
    data = b''
    while not data.endswith(b'\n') and len(data) < 1048576:
        chunk = sock.recv(4096)
        if not chunk:
            break
        data += chunk
    sock.close()
    if not data:
        sys.exit(2)
    sys.stdout.buffer.write(data.splitlines()[0])
except Exception as exc:
    sys.stderr.write(str(exc) + '\n')
    sys.exit(1)
"#;
    format!(
        "ZJBAR_CHOIR_SOCKET={} ZJBAR_CHOIR_REQUEST={}{} python3 - <<'PY'\n{}PY",
        shell_quote(socket_path),
        shell_quote(request),
        initial_cwd,
        script,
    )
}

pub fn parse_status_bar_state_response(raw: &str) -> Result<StatusBarState, StatusBarError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(StatusBarError::Invalid("empty response".into()));
    }
    let root: Value = serde_json::from_str(trimmed)
        .map_err(|e| StatusBarError::Invalid(format!("invalid JSON: {e}")))?;

    if let Some(message) = response_error_message(&root) {
        return Err(StatusBarError::Server(message));
    }

    if let Some(snapshot) = snapshot_value(&root).cloned() {
        return decode_snapshot(snapshot);
    }

    if let Some(text) = root
        .pointer("/result/content/0/text")
        .and_then(|value| value.as_str())
    {
        let text_value: Value = serde_json::from_str(text)
            .map_err(|e| StatusBarError::Invalid(format!("invalid content text JSON: {e}")))?;
        if let Some(snapshot) = snapshot_value(&text_value).cloned() {
            return decode_snapshot(snapshot);
        }
    }

    Err(StatusBarError::Invalid(
        "response did not contain status_bar_state snapshot".into(),
    ))
}

fn decode_snapshot(value: Value) -> Result<StatusBarState, StatusBarError> {
    let schema_version = value
        .get("schema_version")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| StatusBarError::Invalid("missing schema_version".into()))?;
    if schema_version > STATUS_BAR_SCHEMA_VERSION {
        return Err(StatusBarError::SchemaAhead(schema_version));
    }
    if schema_version != STATUS_BAR_SCHEMA_VERSION {
        return Err(StatusBarError::Invalid(format!(
            "unsupported schema_version {schema_version}"
        )));
    }
    serde_json::from_value(value)
        .map_err(|e| StatusBarError::Invalid(format!("schema mismatch: {e}")))
}

fn response_error_message(value: &Value) -> Option<String> {
    if value.get("ok").and_then(|v| v.as_bool()) == Some(false) {
        return Some(
            value
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string(),
        );
    }
    value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(|message| message.as_str())
        .map(|message| message.to_string())
}

fn snapshot_value(value: &Value) -> Option<&Value> {
    if value.get("schema_version").is_some() {
        return Some(value);
    }

    for path in [
        "/result/structuredContent/status_bar_state",
        "/result/structuredContent/snapshot",
        "/result/structuredContent",
        "/result/status_bar_state",
        "/result/snapshot",
        "/result",
        "/structuredContent/status_bar_state",
        "/structuredContent/snapshot",
        "/structuredContent",
        "/status_bar_state",
        "/snapshot",
    ] {
        if let Some(candidate) = value.pointer(path) {
            if candidate.get("schema_version").is_some() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_json() -> String {
        serde_json::json!({
            "schema_version": 1,
            "taken_at_ms": 1760000000123_u64,
            "panes": [{
                "zellij_pane_id": 7,
                "agent_id": "main.leaf-a",
                "role": "dev",
                "agent_type": "codex",
                "lifecycle": "working",
                "pr_number": 42,
                "unresolved_threads": 2,
                "ci_rollup": "in_progress",
                "attention_needed": true,
                "parent_agent_id": "root",
                "last_activity_unix": 1760000000_u64
            }]
        })
        .to_string()
    }

    #[test]
    fn request_is_newline_framed_jsonrpc_tools_call() {
        let request = status_bar_state_jsonrpc_request();
        assert!(request.ends_with('\n'));
        let value: Value = serde_json::from_str(request.trim_end()).unwrap();
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["method"], "tools/call");
        assert_eq!(value["params"]["name"], "status_bar_state");
    }

    #[test]
    fn parse_direct_snapshot() {
        let snapshot = parse_status_bar_state_response(&snapshot_json()).unwrap();
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.panes[0].agent_id, "main.leaf-a");
        assert_eq!(snapshot.panes[0].lifecycle, Lifecycle::Working);
    }

    #[test]
    fn parse_internal_uds_response_snapshot() {
        let response = serde_json::json!({
            "ok": true,
            "result": serde_json::from_str::<Value>(&snapshot_json()).unwrap()
        })
        .to_string();
        let snapshot = parse_status_bar_state_response(&response).unwrap();
        assert_eq!(snapshot.panes[0].zellij_pane_id, 7);
    }

    #[test]
    fn parse_mcp_structured_content_snapshot() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "zjbar-status-bar-state",
            "result": {
                "structuredContent": serde_json::from_str::<Value>(&snapshot_json()).unwrap(),
                "isError": false
            }
        })
        .to_string();
        let snapshot = parse_status_bar_state_response(&response).unwrap();
        assert_eq!(snapshot.panes[0].ci_rollup, CiRollup::InProgress);
    }

    #[test]
    fn parse_mcp_text_content_snapshot() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "zjbar-status-bar-state",
            "result": {
                "content": [{ "type": "text", "text": snapshot_json() }],
                "isError": false
            }
        })
        .to_string();
        let snapshot = parse_status_bar_state_response(&response).unwrap();
        assert_eq!(snapshot.panes[0].attention_needed, true);
    }

    #[test]
    fn schema_ahead_is_explicit() {
        let response = serde_json::json!({
            "schema_version": 2,
            "taken_at_ms": 1,
            "panes": []
        })
        .to_string();
        assert_eq!(
            parse_status_bar_state_response(&response),
            Err(StatusBarError::SchemaAhead(2))
        );
    }

    #[test]
    fn unknown_field_fails_closed() {
        let response = serde_json::json!({
            "schema_version": 1,
            "taken_at_ms": 1,
            "panes": [],
            "extra": true
        })
        .to_string();
        assert!(matches!(
            parse_status_bar_state_response(&response),
            Err(StatusBarError::Invalid(_))
        ));
    }

    #[test]
    fn poll_command_contains_socket_and_request() {
        let command = poll_command(
            ".choir/server.sock",
            Some("/workspace/.choir/worktrees/leaf"),
        );
        assert!(command.contains("ZJBAR_CHOIR_SOCKET='.choir/server.sock'"));
        assert!(command.contains("ZJBAR_CHOIR_REQUEST="));
        assert!(command.contains("AF_UNIX"));
        assert!(command.contains("CHOIR_WORKSPACE"));
        assert!(command.contains("ZJBAR_PLUGIN_INITIAL_CWD"));
        assert!(command.contains("PWD"));
        assert!(command.contains("os.getcwd()"));
        assert!(command.contains("candidate_socket_paths"));
    }
}
