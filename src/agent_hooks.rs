//! 1-Click Coding-Agent Lifecycle Hooks for Claude Code & Cursor (T-009).
//!
//! Hardened against common hook pitfalls:
//! - Matches official 2026 JSON schemas for Claude Code (`settings.json`) and Cursor (`hooks.json`).
//! - Idempotent non-destructive merge: preserves all existing user settings and third-party hooks.
//! - Atomic write (stage sibling temp file -> swap) so agent config files are never corrupted.
//! - Secret-safe event mapping: strips control characters and never leaks stdin secrets.
//! - Short 350ms HTTP timeout and always exits 0 so hooks never block or slow down coding agents.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Map, Value, json};

pub const HOOK_CMD_PREFIX: &str = "diypets hook-event";
pub const DEFAULT_AGENT_PORT: u16 = 3721;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTarget {
    ClaudeCode,
    Cursor,
}

impl AgentTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Cursor => "cursor",
        }
    }

    pub fn parse_targets(arg: &str) -> Result<Vec<Self>, String> {
        match arg.trim().to_ascii_lowercase().as_str() {
            "claude" | "claude-code" | "claudecode" => Ok(vec![Self::ClaudeCode]),
            "cursor" => Ok(vec![Self::Cursor]),
            "all" => Ok(vec![Self::ClaudeCode, Self::Cursor]),
            other => Err(format!(
                "Unknown agent '{other}'. Use: claude-code, cursor, or all"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookScope {
    User,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookReaction {
    pub text: String,
    pub emotion: &'static str,
}

fn user_home_dir() -> Result<PathBuf, String> {
    if let Ok(home) = std::env::var("USERPROFILE")
        && !home.trim().is_empty()
    {
        return Ok(PathBuf::from(home));
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return Ok(PathBuf::from(home));
    }
    Err("Could not determine home directory (USERPROFILE / HOME unset)".to_string())
}

pub fn config_path(target: AgentTarget, scope: HookScope) -> Result<PathBuf, String> {
    let base = match scope {
        HookScope::User => user_home_dir()?,
        HookScope::Project => std::env::current_dir()
            .map_err(|e| format!("Could not resolve current directory: {e}"))?,
    };
    let rel = match target {
        AgentTarget::ClaudeCode => Path::new(".claude").join("settings.json"),
        AgentTarget::Cursor => Path::new(".cursor").join("hooks.json"),
    };
    Ok(base.join(rel))
}

fn read_json_or_empty(path: &Path) -> Result<Value, String> {
    if !path.is_file() {
        return Ok(Value::Object(Map::new()));
    }
    let raw =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let parsed: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Invalid JSON in {}: {e}", path.display()))?;
    if !parsed.is_object() {
        return Err(format!(
            "Expected top-level JSON object in {}",
            path.display()
        ));
    }
    Ok(parsed)
}

fn atomic_write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }
    let mut rendered = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize JSON: {e}"))?;
    rendered.push('\n');

    let tmp_name = format!(
        ".diypets-tmp-{}-{}",
        std::process::id(),
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("hooks.json")
    );
    let tmp_path = path.with_file_name(tmp_name);
    fs::write(&tmp_path, rendered.as_bytes())
        .map_err(|e| format!("Failed to stage {}: {e}", tmp_path.display()))?;

    if path.exists() {
        if let Err(rename_err) = fs::rename(&tmp_path, path) {
            // Windows rejects rename onto an existing file; replace safely after staging.
            fs::copy(&tmp_path, path).map_err(|copy_err| {
                if let Err(rm_err) = fs::remove_file(&tmp_path) {
                    crate::logging::warn(&format!(
                        "agent_hooks: temp cleanup failed for {}: {rm_err}",
                        tmp_path.display()
                    ));
                }
                format!(
                    "Failed to update {} (rename: {rename_err}; copy: {copy_err})",
                    path.display()
                )
            })?;
            if let Err(rm_err) = fs::remove_file(&tmp_path) {
                crate::logging::warn(&format!(
                    "agent_hooks: temp cleanup failed for {}: {rm_err}",
                    tmp_path.display()
                ));
            }
        }
    } else {
        fs::rename(&tmp_path, path).map_err(|e| {
            if let Err(rm_err) = fs::remove_file(&tmp_path) {
                crate::logging::warn(&format!(
                    "agent_hooks: temp cleanup failed for {}: {rm_err}",
                    tmp_path.display()
                ));
            }
            format!("Failed to write {}: {e}", path.display())
        })?;
    }
    Ok(())
}

fn is_diypets_command(cmd: &str) -> bool {
    cmd.contains(HOOK_CMD_PREFIX)
}

fn claude_entry_is_diypets(group: &Value) -> bool {
    if let Some(hooks_arr) = group.get("hooks").and_then(Value::as_array) {
        return hooks_arr.iter().any(|h| {
            h.get("command")
                .and_then(Value::as_str)
                .is_some_and(is_diypets_command)
        });
    }
    false
}

fn cursor_entry_is_diypets(entry: &Value) -> bool {
    entry
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(is_diypets_command)
}

const CLAUDE_EVENTS: &[(&str, bool)] = &[
    ("UserPromptSubmit", false),
    ("PostToolUseFailure", true),
    ("Stop", false),
    ("StopFailure", false),
    ("Notification", true),
    ("TaskCompleted", false),
];

const CURSOR_EVENTS: &[&str] = &["beforeSubmitPrompt", "postToolUseFailure", "stop"];

/// Idempotently install DIYPets hooks into a Claude Code `settings.json` object.
pub fn install_claude_code_hooks_json(root: &mut Value) -> usize {
    let Some(root_obj) = root.as_object_mut() else {
        return 0;
    };
    let hooks_val = root_obj
        .entry("hooks".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !hooks_val.is_object() {
        *hooks_val = Value::Object(Map::new());
    }
    let Some(hooks_map) = hooks_val.as_object_mut() else {
        return 0;
    };

    let mut installed = 0usize;
    for &(event_name, include_matcher) in CLAUDE_EVENTS {
        let arr_val = hooks_map
            .entry(event_name.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if !arr_val.is_array() {
            *arr_val = Value::Array(Vec::new());
        }
        if let Some(arr) = arr_val.as_array_mut() {
            arr.retain(|g| !claude_entry_is_diypets(g));
            let cmd = format!("{HOOK_CMD_PREFIX} claude-code {event_name}");
            let entry = if include_matcher {
                json!({
                    "matcher": "",
                    "hooks": [
                        {
                            "type": "command",
                            "command": cmd
                        }
                    ]
                })
            } else {
                json!({
                    "hooks": [
                        {
                            "type": "command",
                            "command": cmd
                        }
                    ]
                })
            };
            arr.push(entry);
            installed += 1;
        }
    }
    installed
}

/// Remove only DIYPets hook entries from a Claude Code `settings.json` object.
pub fn uninstall_claude_code_hooks_json(root: &mut Value) -> usize {
    let Some(hooks_map) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return 0;
    };
    let mut removed = 0usize;
    let mut empty_keys = Vec::new();
    for (event_name, val) in hooks_map.iter_mut() {
        if let Some(arr) = val.as_array_mut() {
            let before = arr.len();
            arr.retain(|g| !claude_entry_is_diypets(g));
            removed += before.saturating_sub(arr.len());
            if arr.is_empty() {
                empty_keys.push(event_name.clone());
            }
        }
    }
    for key in empty_keys {
        hooks_map.remove(&key);
    }
    removed
}

/// Idempotently install DIYPets hooks into a Cursor `hooks.json` object (`version: 1`).
pub fn install_cursor_hooks_json(root: &mut Value) -> usize {
    let Some(root_obj) = root.as_object_mut() else {
        return 0;
    };
    root_obj
        .entry("version".to_string())
        .or_insert_with(|| json!(1));

    let hooks_val = root_obj
        .entry("hooks".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !hooks_val.is_object() {
        *hooks_val = Value::Object(Map::new());
    }
    let Some(hooks_map) = hooks_val.as_object_mut() else {
        return 0;
    };

    let mut installed = 0usize;
    for &event_name in CURSOR_EVENTS {
        let arr_val = hooks_map
            .entry(event_name.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if !arr_val.is_array() {
            *arr_val = Value::Array(Vec::new());
        }
        if let Some(arr) = arr_val.as_array_mut() {
            arr.retain(|e| !cursor_entry_is_diypets(e));
            let cmd = format!("{HOOK_CMD_PREFIX} cursor {event_name}");
            arr.push(json!({ "command": cmd }));
            installed += 1;
        }
    }
    installed
}

/// Remove only DIYPets hook entries from a Cursor `hooks.json` object.
pub fn uninstall_cursor_hooks_json(root: &mut Value) -> usize {
    let Some(hooks_map) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return 0;
    };
    let mut removed = 0usize;
    let mut empty_keys = Vec::new();
    for (event_name, val) in hooks_map.iter_mut() {
        if let Some(arr) = val.as_array_mut() {
            let before = arr.len();
            arr.retain(|e| !cursor_entry_is_diypets(e));
            removed += before.saturating_sub(arr.len());
            if arr.is_empty() {
                empty_keys.push(event_name.clone());
            }
        }
    }
    for key in empty_keys {
        hooks_map.remove(&key);
    }
    removed
}

pub fn count_installed_hooks(target: AgentTarget, root: &Value) -> usize {
    let Some(hooks_map) = root.get("hooks").and_then(Value::as_object) else {
        return 0;
    };
    let mut count = 0usize;
    for val in hooks_map.values() {
        if let Some(arr) = val.as_array() {
            for item in arr {
                let matched = match target {
                    AgentTarget::ClaudeCode => claude_entry_is_diypets(item),
                    AgentTarget::Cursor => cursor_entry_is_diypets(item),
                };
                if matched {
                    count += 1;
                }
            }
        }
    }
    count
}

pub fn install_hooks(target: AgentTarget, scope: HookScope) -> Result<(PathBuf, usize), String> {
    let path = config_path(target, scope)?;
    let mut json_val = read_json_or_empty(&path)?;
    let count = match target {
        AgentTarget::ClaudeCode => install_claude_code_hooks_json(&mut json_val),
        AgentTarget::Cursor => install_cursor_hooks_json(&mut json_val),
    };
    atomic_write_json(&path, &json_val)?;
    Ok((path, count))
}

pub fn uninstall_hooks(target: AgentTarget, scope: HookScope) -> Result<(PathBuf, usize), String> {
    let path = config_path(target, scope)?;
    if !path.is_file() {
        return Ok((path, 0));
    }
    let mut json_val = read_json_or_empty(&path)?;
    let removed = match target {
        AgentTarget::ClaudeCode => uninstall_claude_code_hooks_json(&mut json_val),
        AgentTarget::Cursor => uninstall_cursor_hooks_json(&mut json_val),
    };
    if removed > 0 {
        atomic_write_json(&path, &json_val)?;
    }
    Ok((path, removed))
}

pub fn status_report(scope: HookScope) -> Vec<(AgentTarget, PathBuf, usize)> {
    let mut out = Vec::new();
    for target in [AgentTarget::ClaudeCode, AgentTarget::Cursor] {
        if let Ok(path) = config_path(target, scope) {
            let count = read_json_or_empty(&path)
                .map(|v| count_installed_hooks(target, &v))
                .unwrap_or(0);
            out.push((target, path, count));
        }
    }
    out
}

fn sanitize_short_label(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_control() {
            continue;
        }
        out.push(ch);
        if out.chars().count() >= max_chars {
            break;
        }
    }
    out.trim().to_string()
}

/// Map a coding-agent hook event (+ optional stdin JSON) into a safe desktop pet reaction.
pub fn map_hook_event(agent: &str, event: &str, stdin_payload: &str) -> Option<HookReaction> {
    let tag = match agent.trim().to_ascii_lowercase().as_str() {
        "cursor" => "Cursor",
        _ => "Claude",
    };

    let tool_hint = serde_json::from_str::<Value>(stdin_payload)
        .ok()
        .and_then(|v| {
            v.get("tool_name")
                .and_then(Value::as_str)
                .map(|s| sanitize_short_label(s, 18))
        })
        .filter(|s| !s.is_empty());

    match event.trim() {
        "UserPromptSubmit" | "beforeSubmitPrompt" => Some(HookReaction {
            text: format!("{tag} thinking..."),
            emotion: "hop",
        }),
        "Stop" | "stop" => Some(HookReaction {
            text: format!("{tag} finished!"),
            emotion: "happy",
        }),
        "TaskCompleted" => Some(HookReaction {
            text: format!("{tag} task done!"),
            emotion: "happy",
        }),
        "PostToolUseFailure" | "postToolUseFailure" => {
            let text = match tool_hint {
                Some(t) => format!("{t} failed!"),
                None => format!("{tag} tool failed!"),
            };
            Some(HookReaction {
                text,
                emotion: "alert",
            })
        }
        "StopFailure" => Some(HookReaction {
            text: format!("{tag} hit an error!"),
            emotion: "alert",
        }),
        "Notification" => Some(HookReaction {
            text: format!("{tag} needs input!"),
            emotion: "hop",
        }),
        _ => None,
    }
}

/// Fire-and-forget local HTTP delivery to a running `diypets` instance.
/// Uses a strict 350ms timeout and ignores errors so hooks never block or fail a coding agent.
pub fn send_hook_reaction_nonblocking(reaction: &HookReaction) {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(350))
        .build();
    let base = format!("http://127.0.0.1:{DEFAULT_AGENT_PORT}");
    let payload = serde_json::to_string(&json!({
        "mood": reaction.emotion,
        "text": reaction.text,
    }));
    if let Ok(body) = payload {
        // Intentionally ignore offline-daemon errors so hooks never block Claude/Cursor.
        drop(
            agent
                .post(&format!("{base}/react"))
                .set("Content-Type", "application/json")
                .send_string(&body),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_install_preserves_existing_hooks_and_is_idempotent() {
        let mut cfg = json!({
            "theme": "dark",
            "hooks": {
                "PostToolUse": [
                    {
                        "matcher": "Edit|Write",
                        "hooks": [{ "type": "command", "command": "npx prettier --write" }]
                    }
                ]
            }
        });

        let n1 = install_claude_code_hooks_json(&mut cfg);
        assert_eq!(n1, CLAUDE_EVENTS.len());
        let n2 = install_claude_code_hooks_json(&mut cfg);
        assert_eq!(n2, CLAUDE_EVENTS.len());

        // Existing settings and third-party hooks stay intact, and DIYPets entries never duplicate.
        assert_eq!(cfg["theme"], "dark");
        assert_eq!(cfg["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(cfg["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(
            count_installed_hooks(AgentTarget::ClaudeCode, &cfg),
            CLAUDE_EVENTS.len()
        );

        let removed = uninstall_claude_code_hooks_json(&mut cfg);
        assert_eq!(removed, CLAUDE_EVENTS.len());
        assert_eq!(count_installed_hooks(AgentTarget::ClaudeCode, &cfg), 0);
        assert_eq!(cfg["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn cursor_install_and_uninstall_roundtrip_preserves_user_hooks() {
        let mut cfg = json!({
            "version": 1,
            "hooks": {
                "afterFileEdit": [{ "command": "./hooks/format.sh" }]
            }
        });

        let installed = install_cursor_hooks_json(&mut cfg);
        assert_eq!(installed, CURSOR_EVENTS.len());
        assert_eq!(cfg["version"], 1);
        assert_eq!(cfg["hooks"]["afterFileEdit"].as_array().unwrap().len(), 1);
        assert_eq!(
            count_installed_hooks(AgentTarget::Cursor, &cfg),
            CURSOR_EVENTS.len()
        );

        let removed = uninstall_cursor_hooks_json(&mut cfg);
        assert_eq!(removed, CURSOR_EVENTS.len());
        assert_eq!(cfg["hooks"]["afterFileEdit"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn hook_event_mapping_sanitizes_tool_name_and_maps_events() {
        let ok = map_hook_event("claude-code", "Stop", "{}").expect("Stop maps");
        assert_eq!(ok.text, "Claude finished!");

        let payload = "{\"tool_name\":\"Bash\n\r\tExtraLongToolNameShouldTruncate\"}";
        let fail =
            map_hook_event("claude-code", "PostToolUseFailure", payload).expect("failure maps");
        assert!(!fail.text.contains('\n'));
        assert!(fail.text.ends_with("failed!"));

        let cur = map_hook_event("cursor", "beforeSubmitPrompt", "").expect("cursor prompt maps");
        assert_eq!(cur.text, "Cursor thinking...");
    }
}
