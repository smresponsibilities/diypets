//! Local HTTP + MCP (JSON-RPC 2.0) server on `127.0.0.1:3721` so AI coding agents
//! (Claude Code, Cursor, Codex, Gemini CLI) and CI scripts can control your desktop pets.
//!
//! Hardened (T-022):
//! - Strict `127.0.0.1` loopback bind & peer check.
//! - 4 KB request cap (`413 Payload Too Large`) on both raw bytes and `Content-Length`.
//! - Sliding-window burst rate limiter (`429 Too Many Requests`).
//! - Input sanitization for speech text, mood, `pack_id`, and `duration_ms`.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::time::{Duration, Instant};

pub const DEFAULT_PORT: u16 = 3721;
pub const MAX_HTTP_REQUEST_BYTES: usize = 4096;
pub const MAX_TEXT_CHARS: usize = 160;
pub const MAX_MOOD_CHARS: usize = 32;
pub const MAX_PACK_ID_CHARS: usize = 64;
pub const RATE_LIMIT_MAX_REQUESTS: usize = 20;
pub const RATE_LIMIT_WINDOW_MS: u64 = 1000;

pub type SharedHerd = Arc<Mutex<Vec<String>>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCommand {
    #[serde(alias = "action")]
    pub mood: String,
    #[serde(default, alias = "message")]
    pub text: Option<String>,
    #[serde(default)]
    pub pack_id: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

impl AgentCommand {
    /// Sanitize untrusted agent fields so malformed or hostile payloads cannot break UI or paths.
    pub fn sanitized(self) -> Self {
        let mood_clean: String = self
            .mood
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .take(MAX_MOOD_CHARS)
            .collect();
        let mood = if mood_clean.is_empty() {
            "happy".to_string()
        } else {
            mood_clean
        };

        let text = self.text.and_then(|raw| {
            let clean: String = raw
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(MAX_TEXT_CHARS)
                .collect();
            if clean.is_empty() { None } else { Some(clean) }
        });

        let pack_id = self.pack_id.and_then(|raw| {
            let clean: String = raw
                .trim()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .take(MAX_PACK_ID_CHARS)
                .collect();
            if clean.is_empty() { None } else { Some(clean) }
        });

        let duration_ms = self.duration_ms.map(|ms| ms.clamp(1000, 15_000));

        Self {
            mood,
            text,
            pack_id,
            duration_ms,
            enabled: self.enabled,
        }
    }
}

/// Sliding-window burst limiter preventing runaway agent loops from flooding the desktop runner.
#[derive(Debug)]
pub struct RateLimiter {
    window: Duration,
    max_requests: usize,
    hits: VecDeque<Instant>,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            window,
            max_requests: max_requests.max(1),
            hits: VecDeque::with_capacity(max_requests.max(1)),
        }
    }

    pub fn allow(&mut self, now: Instant) -> bool {
        while let Some(&front) = self.hits.front() {
            if now.saturating_duration_since(front) >= self.window {
                self.hits.pop_front();
            } else {
                break;
            }
        }
        if self.hits.len() >= self.max_requests {
            return false;
        }
        self.hits.push_back(now);
        true
    }
}

fn http_json(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn http_rate_limited() -> String {
    let body = r#"{"ok":false,"error":"rate limit exceeded"}"#;
    format!(
        "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nRetry-After: 1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn declared_content_length(raw: &str) -> Option<usize> {
    let headers = raw
        .split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .map(|(h, _)| h)
        .unwrap_or(raw);
    for line in headers.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':')
            && k.trim().eq_ignore_ascii_case("content-length")
            && let Ok(len) = v.trim().parse::<usize>()
        {
            return Some(len);
        }
    }
    None
}

fn dispatch_command(tx: &Sender<AgentCommand>, cmd: AgentCommand) -> Result<(), String> {
    tx.send(cmd.sanitized()).map_err(|e| {
        crate::logging::warn(&format!("agent_api: desktop runner channel closed: {e}"));
        "DIYPets desktop runner is shutting down".to_string()
    })
}

pub fn handle_http_request(raw: &str, tx: &Sender<AgentCommand>) -> String {
    handle_http_request_with_herd(raw, tx, &[])
}

pub fn handle_http_request_with_herd(
    raw: &str,
    tx: &Sender<AgentCommand>,
    active_herd: &[String],
) -> String {
    if raw.len() > MAX_HTTP_REQUEST_BYTES
        || declared_content_length(raw).is_some_and(|n| n > MAX_HTTP_REQUEST_BYTES)
    {
        return http_json(
            "413 Payload Too Large",
            r#"{"ok":false,"error":"request exceeds 4096 byte limit"}"#,
        );
    }

    let mut lines = raw.lines();
    let req_line = lines.next().unwrap_or("");
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    let body = raw
        .split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .map(|(_, b)| b.trim())
        .unwrap_or("");

    match (method, path) {
        ("GET", "/status") | ("GET", "/") => {
            let payload = serde_json::json!({
                "ok": true,
                "app": "diypets",
                "version": env!("CARGO_PKG_VERSION"),
                "port": DEFAULT_PORT,
                "herd_count": active_herd.len(),
            });
            http_json("200 OK", &payload.to_string())
        }
        ("GET", "/herd") => {
            let payload = serde_json::json!({
                "ok": true,
                "herd": active_herd,
                "count": active_herd.len(),
            });
            http_json("200 OK", &payload.to_string())
        }
        ("POST", "/react") => {
            let cmd = if body.is_empty() {
                AgentCommand {
                    mood: "happy".to_string(),
                    text: None,
                    pack_id: None,
                    duration_ms: None,
                    enabled: None,
                }
            } else {
                match serde_json::from_str::<AgentCommand>(body) {
                    Ok(c) => c,
                    Err(_) => {
                        return http_json(
                            "400 Bad Request",
                            r#"{"ok":false,"error":"invalid JSON payload"}"#,
                        );
                    }
                }
            };
            match dispatch_command(tx, cmd) {
                Ok(()) => http_json("200 OK", r#"{"ok":true}"#),
                Err(_) => http_json(
                    "503 Service Unavailable",
                    r#"{"ok":false,"error":"runner channel unavailable"}"#,
                ),
            }
        }
        ("POST", "/mcp") => handle_mcp_jsonrpc(body, tx, active_herd),
        _ => http_json("404 Not Found", r#"{"ok":false,"error":"not found"}"#),
    }
}

fn handle_mcp_jsonrpc(
    body: &str,
    tx: &Sender<AgentCommand>,
    active_herd: &[String],
) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        return http_json(
            "400 Bad Request",
            r#"{"jsonrpc":"2.0","error":{"code":-32700,"message":"parse error"}}"#,
        );
    };
    let id = v.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");

    let result = match method {
        "initialize" => serde_json::json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": { "name": "diypets", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "tools": {} }
        }),
        "tools/list" => mcp_tools_manifest(),
        "tools/call" => {
            let params = v.get("params").cloned().unwrap_or_default();
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_default();
            match name {
                "react_pet" => {
                    let mood = args
                        .get("mood")
                        .and_then(|s| s.as_str())
                        .unwrap_or("happy")
                        .to_string();
                    let text = args.get("text").and_then(|s| s.as_str()).map(String::from);
                    let duration_ms = args.get("duration_ms").and_then(|d| d.as_u64());
                    let cmd = AgentCommand {
                        mood,
                        text,
                        pack_id: None,
                        duration_ms,
                        enabled: None,
                    }
                    .sanitized();
                    let mood_out = cmd.mood.clone();
                    match dispatch_command(tx, cmd) {
                        Ok(()) => serde_json::json!({
                            "content": [{ "type": "text", "text": format!("DIYPets reacted: {mood_out}") }]
                        }),
                        Err(err) => serde_json::json!({
                            "isError": true,
                            "content": [{ "type": "text", "text": err }]
                        }),
                    }
                }
                "say" => {
                    let text = args
                        .get("text")
                        .and_then(|s| s.as_str())
                        .unwrap_or("Hello!")
                        .to_string();
                    let duration_ms = args.get("duration_ms").and_then(|d| d.as_u64());
                    let cmd = AgentCommand {
                        mood: "say".to_string(),
                        text: Some(text),
                        pack_id: None,
                        duration_ms,
                        enabled: None,
                    }
                    .sanitized();
                    let shown = cmd.text.clone().unwrap_or_else(|| "Hello!".to_string());
                    match dispatch_command(tx, cmd) {
                        Ok(()) => serde_json::json!({
                            "content": [{ "type": "text", "text": format!("DIYPets said: {shown}") }]
                        }),
                        Err(err) => serde_json::json!({
                            "isError": true,
                            "content": [{ "type": "text", "text": err }]
                        }),
                    }
                }
                "spawn_pet" => {
                    let pack_id = args
                        .get("pack_id")
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    let cmd = AgentCommand {
                        mood: "spawn".to_string(),
                        text: None,
                        pack_id,
                        duration_ms: None,
                        enabled: None,
                    }
                    .sanitized();
                    let clean_id = cmd.pack_id.clone();
                    match dispatch_command(tx, cmd) {
                        Ok(()) => serde_json::json!({
                            "content": [{ "type": "text", "text": format!("Spawned pet: {clean_id:?}") }]
                        }),
                        Err(err) => serde_json::json!({
                            "isError": true,
                            "content": [{ "type": "text", "text": err }]
                        }),
                    }
                }
                "drop_ball" => {
                    let cmd = AgentCommand {
                        mood: "ball".to_string(),
                        text: Some("Play time!".to_string()),
                        pack_id: None,
                        duration_ms: Some(3000),
                        enabled: None,
                    };
                    match dispatch_command(tx, cmd) {
                        Ok(()) => serde_json::json!({
                            "content": [{ "type": "text", "text": "Dropped bouncy ball for DIYPets herd!" }]
                        }),
                        Err(err) => serde_json::json!({
                            "isError": true,
                            "content": [{ "type": "text", "text": err }]
                        }),
                    }
                }
                "set_follow_cursor" => {
                    let enabled = args.get("enabled").and_then(|b| b.as_bool()).unwrap_or(true);
                    let cmd = AgentCommand {
                        mood: "follow_cursor".to_string(),
                        text: None,
                        pack_id: None,
                        duration_ms: None,
                        enabled: Some(enabled),
                    };
                    match dispatch_command(tx, cmd) {
                        Ok(()) => serde_json::json!({
                            "content": [{ "type": "text", "text": format!("Follow Cursor set to {enabled}") }]
                        }),
                        Err(err) => serde_json::json!({
                            "isError": true,
                            "content": [{ "type": "text", "text": err }]
                        }),
                    }
                }
                "list_herd" => {
                    let summary = serde_json::json!({
                        "count": active_herd.len(),
                        "herd": active_herd,
                    });
                    serde_json::json!({
                        "content": [{ "type": "text", "text": summary.to_string() }]
                    })
                }
                _ => serde_json::json!({ "isError": true, "content": [{ "type": "text", "text": "unknown tool" }] }),
            }
        }
        _ => serde_json::json!({}),
    };

    let resp = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    http_json("200 OK", &resp.to_string())
}

enum ReadRequestOutcome {
    Ok(String),
    TooLarge,
    Empty,
}

fn read_bounded_http_request(stream: &mut TcpStream) -> ReadRequestOutcome {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() > MAX_HTTP_REQUEST_BYTES {
                    return ReadRequestOutcome::TooLarge;
                }
                let as_str = String::from_utf8_lossy(&buf);
                if let Some((headers, body)) = as_str
                    .split_once("\r\n\r\n")
                    .or_else(|| as_str.split_once("\n\n"))
                {
                    if let Some(cl) = declared_content_length(headers) {
                        if cl > MAX_HTTP_REQUEST_BYTES {
                            return ReadRequestOutcome::TooLarge;
                        }
                        if body.len() >= cl {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(e) => {
                crate::logging::warn(&format!("agent_api: socket read error: {e}"));
                break;
            }
        }
    }
    if buf.is_empty() {
        ReadRequestOutcome::Empty
    } else {
        ReadRequestOutcome::Ok(String::from_utf8_lossy(&buf).into_owned())
    }
}

pub fn start_local_server(tx: Sender<AgentCommand>, shared_herd: SharedHerd) {
    std::thread::spawn(move || {
        let addr = SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT));
        let listener = match TcpListener::bind(addr) {
            Ok(l) => l,
            Err(e) => {
                crate::logging::warn(&format!("agent_api: failed to bind {addr}: {e}"));
                return;
            }
        };
        let mut limiter = RateLimiter::new(
            RATE_LIMIT_MAX_REQUESTS,
            Duration::from_millis(RATE_LIMIT_WINDOW_MS),
        );

        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            if let Ok(peer) = stream.peer_addr()
                && !peer.ip().is_loopback()
            {
                crate::logging::warn(&format!("agent_api: rejected non-loopback peer {peer}"));
                continue;
            }
            if let Err(e) = stream.set_read_timeout(Some(Duration::from_millis(500))) {
                crate::logging::warn(&format!("agent_api: failed setting read timeout: {e}"));
            }
            if let Err(e) = stream.set_write_timeout(Some(Duration::from_millis(500))) {
                crate::logging::warn(&format!("agent_api: failed setting write timeout: {e}"));
            }

            let resp = if !limiter.allow(Instant::now()) {
                http_rate_limited()
            } else {
                match read_bounded_http_request(&mut stream) {
                    ReadRequestOutcome::Empty => continue,
                    ReadRequestOutcome::TooLarge => http_json(
                        "413 Payload Too Large",
                        r#"{"ok":false,"error":"request exceeds 4096 byte limit"}"#,
                    ),
                    ReadRequestOutcome::Ok(req) => {
                        let snapshot = shared_herd
                            .lock()
                            .map(|g| g.clone())
                            .unwrap_or_default();
                        handle_http_request_with_herd(&req, &tx, &snapshot)
                    }
                }
            };

            if let Err(e) = stream.write_all(resp.as_bytes()) {
                crate::logging::warn(&format!("agent_api: response write error: {e}"));
            } else if let Err(e) = stream.flush() {
                crate::logging::warn(&format!("agent_api: response flush error: {e}"));
            }
        }
    });
}

pub fn send_cli_command(cmd: &AgentCommand) -> Result<String, String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(1))
        .map_err(|_| "DIYPets is not running (could not connect to 127.0.0.1:3721).".to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .map_err(|e| format!("Failed to set socket read timeout: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .map_err(|e| format!("Failed to set socket write timeout: {e}"))?;

    let clean = cmd.clone().sanitized();
    let payload = serde_json::to_string(&clean).map_err(|e| e.to_string())?;
    let req = format!(
        "POST /react HTTP/1.1\r\nHost: 127.0.0.1:{DEFAULT_PORT}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut resp = String::new();
    stream
        .read_to_string(&mut resp)
        .map_err(|e| format!("Failed reading response from DIYPets daemon: {e}"))?;
    Ok(resp)
}

pub fn send_cli_react(mood: &str, text: Option<&str>) -> Result<String, String> {
    send_cli_command(&AgentCommand {
        mood: mood.to_string(),
        text: text.map(String::from),
        pack_id: None,
        duration_ms: None,
        enabled: None,
    })
}

fn mcp_tools_manifest() -> serde_json::Value {
    serde_json::json!({
        "tools": [
            {
                "name": "react_pet",
                "description": "Make your DIYPets desktop pets react (happy/hearts, hop, sleep, walk, ball) with an optional speech bubble",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "mood": { "type": "string", "enum": ["happy", "hop", "sleep", "walk", "ball"] },
                        "text": { "type": "string" },
                        "duration_ms": { "type": "integer", "minimum": 1000, "maximum": 15000 }
                    },
                    "required": ["mood"]
                }
            },
            {
                "name": "say",
                "description": "Show a floating speech bubble above the primary desktop pet without forcing a hop",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        "duration_ms": { "type": "integer", "minimum": 1000, "maximum": 15000 }
                    },
                    "required": ["text"]
                }
            },
            {
                "name": "drop_ball",
                "description": "Drop a bouncy pixel toy ball onto the desktop for all pets to chase and kick",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "spawn_pet",
                "description": "Add another pet to the active desktop herd (up to 8)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pack_id": { "type": "string" }
                    }
                }
            },
            {
                "name": "set_follow_cursor",
                "description": "Enable or disable Play Mode cursor chasing across the active herd",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "enabled": { "type": "boolean" }
                    },
                    "required": ["enabled"]
                }
            },
            {
                "name": "list_herd",
                "description": "List all currently active desktop pets in the herd",
                "inputSchema": { "type": "object", "properties": {} }
            }
        ]
    })
}

fn forward_mcp_to_daemon(json_line: &str) -> Result<String, String> {
    if json_line.len() > MAX_HTTP_REQUEST_BYTES {
        return Err("MCP request exceeds 4096 byte limit".to_string());
    }
    let addr = SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(1))
        .map_err(|_| "DIYPets desktop app is not running (could not connect to 127.0.0.1:3721). Launch `diypets` first.".to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| format!("Failed to set MCP socket read timeout: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| format!("Failed to set MCP socket write timeout: {e}"))?;

    let req = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{DEFAULT_PORT}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json_line}",
        json_line.len()
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut resp = String::new();
    stream
        .read_to_string(&mut resp)
        .map_err(|e| format!("Failed reading MCP response from DIYPets daemon: {e}"))?;
    resp.split_once("\r\n\r\n")
        .or_else(|| resp.split_once("\n\n"))
        .map(|(_, b)| b.trim().to_string())
        .filter(|b| !b.is_empty())
        .ok_or_else(|| "Empty HTTP response from DIYPets daemon".to_string())
}

/// Process a single newline-delimited JSON-RPC 2.0 line for `diypets mcp` stdio mode.
/// Returns `None` for JSON-RPC notifications (which must not emit a response).
pub fn process_stdio_mcp_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return Some(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": serde_json::Value::Null,
                "error": { "code": -32700, "message": "Parse error" }
            })
            .to_string(),
        );
    };
    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = v.get("id").cloned();

    if method.starts_with("notifications/") || id.is_none() {
        return None;
    }
    let id = id.unwrap();

    match method {
        "initialize" => Some(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "serverInfo": { "name": "diypets", "version": env!("CARGO_PKG_VERSION") },
                    "capabilities": { "tools": {} }
                }
            })
            .to_string(),
        ),
        "ping" => Some(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            })
            .to_string(),
        ),
        "tools/list" => Some(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": mcp_tools_manifest()
            })
            .to_string(),
        ),
        "tools/call" => match forward_mcp_to_daemon(trimmed) {
            Ok(body) => Some(body),
            Err(msg) => Some(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "isError": true,
                        "content": [{ "type": "text", "text": msg }]
                    }
                })
                .to_string(),
            ),
        },
        _ => Some(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") }
            })
            .to_string(),
        ),
    }
}

/// Run the blocking `stdio` MCP JSON-RPC 2.0 server (`diypets mcp`) for Claude Desktop, Cursor, and Claude Code.
pub fn run_stdio_mcp_bridge() {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines().map_while(Result::ok) {
        if let Some(reply) = process_stdio_mcp_line(&line)
            && (writeln!(stdout, "{reply}").is_err() || stdout.flush().is_err())
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn http_react_and_mcp_tool_call_dispatch_commands() {
        let (tx, rx) = mpsc::channel();
        let herd = vec!["highland_cow".to_string(), "dog".to_string()];

        let req = "POST /react HTTP/1.1\r\nHost: localhost\r\n\r\n{\"mood\":\"happy\",\"text\":\"Build green!\"}";
        let resp = handle_http_request_with_herd(req, &tx, &herd);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        let cmd = rx.try_recv().unwrap();
        assert_eq!(cmd.mood, "happy");
        assert_eq!(cmd.text.as_deref(), Some("Build green!"));

        let herd_resp = handle_http_request_with_herd("GET /herd HTTP/1.1\r\n\r\n", &tx, &herd);
        assert!(herd_resp.contains("highland_cow"));
        assert!(herd_resp.contains("\"count\":2"));

        let mcp_say = "POST /mcp HTTP/1.1\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"say\",\"arguments\":{\"text\":\"PR merged!\",\"duration_ms\":6000}}}";
        let say_resp = handle_http_request_with_herd(mcp_say, &tx, &herd);
        assert!(say_resp.contains("DIYPets said: PR merged!"));
        let say_cmd = rx.try_recv().unwrap();
        assert_eq!(say_cmd.mood, "say");
        assert_eq!(say_cmd.duration_ms, Some(6000));

        let mcp_follow = "POST /mcp HTTP/1.1\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"set_follow_cursor\",\"arguments\":{\"enabled\":true}}}";
        let follow_resp = handle_http_request_with_herd(mcp_follow, &tx, &herd);
        assert!(follow_resp.contains("Follow Cursor set to true"));
        assert_eq!(rx.try_recv().unwrap().enabled, Some(true));

        let mcp_list = "POST /mcp HTTP/1.1\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"list_herd\",\"arguments\":{}}}";
        let list_resp = handle_http_request_with_herd(mcp_list, &tx, &herd);
        assert!(list_resp.contains("highland_cow"));
    }

    #[test]
    fn stdio_mcp_bridge_handles_handshake_notifications_and_tools_list() {
        let init = process_stdio_mcp_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).unwrap();
        assert!(init.contains("\"protocolVersion\":\"2024-11-05\""));
        assert!(init.contains("\"name\":\"diypets\""));

        assert_eq!(
            process_stdio_mcp_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            None,
            "notifications must not emit a stdout response"
        );

        let list = process_stdio_mcp_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        assert!(list.contains("react_pet"));
        assert!(list.contains("drop_ball"));
        assert!(list.contains("list_herd"));

        let ping = process_stdio_mcp_line(r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#).unwrap();
        assert!(ping.contains("\"result\":{}"));
    }

    #[test]
    fn rejects_oversized_requests_and_bad_json() {
        let (tx, _rx) = mpsc::channel();
        let big_body = "a".repeat(MAX_HTTP_REQUEST_BYTES + 128);
        let big_req = format!("POST /react HTTP/1.1\r\n\r\n{big_body}");
        assert!(handle_http_request(&big_req, &tx).starts_with("HTTP/1.1 413"));

        let huge_cl = "POST /react HTTP/1.1\r\nContent-Length: 999999\r\n\r\n{}";
        assert!(handle_http_request(huge_cl, &tx).starts_with("HTTP/1.1 413"));

        let bad_json = "POST /react HTTP/1.1\r\n\r\n{not-json";
        assert!(handle_http_request(bad_json, &tx).starts_with("HTTP/1.1 400"));
    }

    #[test]
    fn rate_limiter_caps_burst_and_recovers_after_window() {
        let mut lim = RateLimiter::new(3, Duration::from_secs(1));
        let t0 = Instant::now();
        assert!(lim.allow(t0));
        assert!(lim.allow(t0 + Duration::from_millis(10)));
        assert!(lim.allow(t0 + Duration::from_millis(20)));
        assert!(!lim.allow(t0 + Duration::from_millis(30)));
        assert!(lim.allow(t0 + Duration::from_millis(1050)));
    }

    #[test]
    fn sanitizes_agent_command_fields_and_supports_hook_aliases() {
        let (tx, rx) = mpsc::channel();
        let long_msg = "x".repeat(300);
        let req = format!(
            "POST /react HTTP/1.1\r\n\r\n{{\"action\":\"HOP!\",\"message\":\"hi\\n\\t{long_msg}\",\"pack_id\":\"../goat\",\"duration_ms\":999999}}"
        );
        let resp = handle_http_request(&req, &tx);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        let cmd = rx.try_recv().unwrap();
        assert_eq!(cmd.mood, "hop");
        assert_eq!(cmd.pack_id.as_deref(), Some("goat"));
        assert_eq!(cmd.duration_ms, Some(15_000));
        let txt = cmd.text.unwrap();
        assert!(!txt.contains('\n'));
        assert!(txt.chars().count() <= MAX_TEXT_CHARS);
    }

    #[test]
    fn returns_503_when_runner_channel_is_closed() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let req = "POST /react HTTP/1.1\r\n\r\n{\"mood\":\"happy\"}";
        assert!(handle_http_request(req, &tx).starts_with("HTTP/1.1 503"));
    }
}
