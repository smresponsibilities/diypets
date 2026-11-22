//! Local HTTP + MCP (JSON-RPC 2.0) server on `127.0.0.1:3721` so AI coding agents
//! (Claude Code, Cursor, Codex, Gemini CLI) and CI scripts can react on your desktop.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 3721;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCommand {
    pub mood: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub pack_id: Option<String>,
}

fn http_json(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

pub fn handle_http_request(raw: &str, tx: &Sender<AgentCommand>) -> String {
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
            http_json("200 OK", r#"{"ok":true,"app":"diypets","port":3721}"#)
        }
        ("POST", "/react") => {
            let cmd = serde_json::from_str::<AgentCommand>(body).unwrap_or(AgentCommand {
                mood: "happy".to_string(),
                text: None,
                pack_id: None,
            });
            let _ = tx.send(cmd);
            http_json("200 OK", r#"{"ok":true}"#)
        }
        ("POST", "/mcp") => handle_mcp_jsonrpc(body, tx),
        _ => http_json("404 Not Found", r#"{"ok":false,"error":"not found"}"#),
    }
}

fn handle_mcp_jsonrpc(body: &str, tx: &Sender<AgentCommand>) -> String {
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
                    "description": "Make your DIYPets desktop pet react (happy/hearts, hop, sleep, walk)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "mood": { "type": "string", "enum": ["happy", "hop", "sleep", "walk"] },
                            "text": { "type": "string" }
                        },
                        "required": ["mood"]
                    }
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
                    let _ = tx.send(AgentCommand {
                        mood: mood.clone(),
                        text,
                        pack_id: None,
                    });
                    serde_json::json!({
                        "content": [{ "type": "text", "text": format!("DIYPets reacted: {mood}") }]
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
                    });
                    serde_json::json!({
                        "content": [{ "type": "text", "text": format!("Spawned pet: {:?}", pack_id) }]
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

pub fn start_local_server(tx: Sender<AgentCommand>) {
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
                let resp = handle_http_request(&req, &tx);
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        }
    });
}

pub fn send_cli_react(mood: &str, text: Option<&str>) -> Result<String, String> {
    let addr: SocketAddr = format!("127.0.0.1:{DEFAULT_PORT}").parse().unwrap();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(1))
        .map_err(|_| "DIYPets is not running (could not connect to 127.0.0.1:3721).".to_string())?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));

    let payload = serde_json::to_string(&AgentCommand {
        mood: mood.to_string(),
        text: text.map(String::from),
        pack_id: None,
    })
    .map_err(|e| e.to_string())?;

    let req = format!(
        "POST /react HTTP/1.1\r\nHost: 127.0.0.1:{DEFAULT_PORT}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp);
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn http_react_and_mcp_tool_call_dispatch_commands() {
        let (tx, rx) = mpsc::channel();
        let req = "POST /react HTTP/1.1\r\nHost: localhost\r\n\r\n{\"mood\":\"happy\",\"text\":\"Build green!\"}";
        let resp = handle_http_request(req, &tx);
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        let cmd = rx.try_recv().unwrap();
        assert_eq!(cmd.mood, "happy");
        assert_eq!(cmd.text.as_deref(), Some("Build green!"));

        let mcp = "POST /mcp HTTP/1.1\r\nHost: localhost\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"react_pet\",\"arguments\":{\"mood\":\"sleep\"}}}";
        let mcp_resp = handle_http_request(mcp, &tx);
        assert!(mcp_resp.contains("DIYPets reacted: sleep"));
        assert_eq!(rx.try_recv().unwrap().mood, "sleep");
    }
}