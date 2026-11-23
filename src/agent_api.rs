//! Local HTTP + MCP (JSON-RPC 2.0) server on `127.0.0.1:3721` so AI coding agents
//! (Claude Code, Cursor, Codex, Gemini CLI) and CI scripts can control your desktop pets.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 3721;

pub type SharedHerd = Arc<Mutex<Vec<String>>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCommand {
    pub mood: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub pack_id: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

fn http_json(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

pub fn handle_http_request(raw: &str, tx: &Sender<AgentCommand>) -> String {
    handle_http_request_with_herd(raw, tx, &[])
}

pub fn handle_http_request_with_herd(
    raw: &str,
    tx: &Sender<AgentCommand>,
    active_herd: &[String],
) -> String {
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
            let cmd = serde_json::from_str::<AgentCommand>(body).unwrap_or(AgentCommand {
                mood: "happy".to_string(),
                text: None,
                pack_id: None,
                duration_ms: None,
                enabled: None,
            });
            let _ = tx.send(cmd);
            http_json("200 OK", r#"{"ok":true}"#)
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
        "tools/list" => serde_json::json!({
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
        }),
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
                    let _ = tx.send(AgentCommand {
                        mood: mood.clone(),
                        text,
                        pack_id: None,
                        duration_ms,
                        enabled: None,
                    });
                    serde_json::json!({
                        "content": [{ "type": "text", "text": format!("DIYPets reacted: {mood}") }]
                    })
                }
                "say" => {
                    let text = args
                        .get("text")
                        .and_then(|s| s.as_str())
                        .unwrap_or("Hello!")
                        .to_string();
                    let duration_ms = args.get("duration_ms").and_then(|d| d.as_u64());
                    let _ = tx.send(AgentCommand {
                        mood: "say".to_string(),
                        text: Some(text.clone()),
                        pack_id: None,
                        duration_ms,
                        enabled: None,
                    });
                    serde_json::json!({
                        "content": [{ "type": "text", "text": format!("DIYPets said: {text}") }]
                    })
                }
                "spawn_pet" => {
                    let pack_id = args
                        .get("pack_id")
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    let _ = tx.send(AgentCommand {
                        mood: "spawn".to_string(),
                        text: None,
                        pack_id: pack_id.clone(),
                        duration_ms: None,
                        enabled: None,
                    });
                    serde_json::json!({
                        "content": [{ "type": "text", "text": format!("Spawned pet: {:?}", pack_id) }]
                    })
                }
                "drop_ball" => {
                    let _ = tx.send(AgentCommand {
                        mood: "ball".to_string(),
                        text: Some("Play time!".to_string()),
                        pack_id: None,
                        duration_ms: Some(3000),
                        enabled: None,
                    });
                    serde_json::json!({
                        "content": [{ "type": "text", "text": "Dropped bouncy ball for DIYPets herd!" }]
                    })
                }
                "set_follow_cursor" => {
                    let enabled = args.get("enabled").and_then(|b| b.as_bool()).unwrap_or(true);
                    let _ = tx.send(AgentCommand {
                        mood: "follow_cursor".to_string(),
                        text: None,
                        pack_id: None,
                        duration_ms: None,
                        enabled: Some(enabled),
                    });
                    serde_json::json!({
                        "content": [{ "type": "text", "text": format!("Follow Cursor set to {enabled}") }]
                    })
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

pub fn start_local_server(tx: Sender<AgentCommand>, shared_herd: SharedHerd) {
    std::thread::spawn(move || {
        let Ok(listener) = TcpListener::bind(("127.0.0.1", DEFAULT_PORT)) else {
            return;
        };
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let mut buf = [0u8; 8192];
            if let Ok(n) = stream.read(&mut buf)
                && n > 0
            {
                let req = String::from_utf8_lossy(&buf[..n]);
                let snapshot = shared_herd
                    .lock()
                    .map(|g| g.clone())
                    .unwrap_or_default();
                let resp = handle_http_request_with_herd(&req, &tx, &snapshot);
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        }
    });
}

pub fn send_cli_command(cmd: &AgentCommand) -> Result<String, String> {
    let addr: SocketAddr = format!("127.0.0.1:{DEFAULT_PORT}").parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(1))
        .map_err(|_| "DIYPets is not running (could not connect to 127.0.0.1:3721).".to_string())?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));

    let payload = serde_json::to_string(cmd).map_err(|e| e.to_string())?;
    let req = format!(
        "POST /react HTTP/1.1\r\nHost: 127.0.0.1:{DEFAULT_PORT}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp);
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
    let addr: SocketAddr = format!("127.0.0.1:{DEFAULT_PORT}").parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(1))
        .map_err(|_| "DIYPets desktop app is not running (could not connect to 127.0.0.1:3721). Launch `diypets` first.".to_string())?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));

    let req = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{DEFAULT_PORT}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json_line}",
        json_line.len()
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp);
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

    // Strict JSON-RPC 2.0: notifications have no `id` and expect no reply
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
        if let Some(reply) = process_stdio_mcp_line(&line) {
            if writeln!(stdout, "{reply}").is_err() {
                break;
            }
            let _ = stdout.flush();
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
}