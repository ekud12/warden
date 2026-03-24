// ─── assistant::claude_code — Claude Code adapter ────────────────────────────

use super::{Assistant, HookInput};
use std::path::PathBuf;

#[derive(Default)]
pub struct ClaudeCode;

impl Assistant for ClaudeCode {
    fn name(&self) -> &str { "claude-code" }

    fn parse_input(&self, raw: &str) -> Option<HookInput> {
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        Some(HookInput {
            tool_name: v.get("tool_name").and_then(|v| v.as_str()).map(|s| s.to_string()),
            tool_input: v.get("tool_input").cloned(),
            tool_output: v.get("tool_response")
                .or_else(|| v.get("tool_result"))
                .or_else(|| v.get("tool_output"))
                .cloned(),
            command: v.get("tool_input")
                .and_then(|ti| ti.get("command"))
                .and_then(|c| c.as_str())
                .map(|s| s.to_string()),
            exit_code: v.get("tool_response")
                .and_then(|tr| tr.get("exitCode"))
                .and_then(|e| e.as_i64()),
            session_id: v.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            reason: v.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
            hook_event: v.get("hook_event_name").and_then(|v| v.as_str()).map(|s| s.to_string()),
            agent_type: v.get("agent_type").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    fn format_deny(&self, event: &str, message: &str) -> String {
        serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": event,
                "permissionDecision": "deny"
            },
            "systemMessage": message
        }).to_string()
    }

    fn format_allow(&self, advisory: Option<&str>) -> String {
        let mut out = serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow"
            }
        });
        if let Some(msg) = advisory {
            out["additionalContext"] = serde_json::json!(msg);
        }
        out.to_string()
    }

    fn format_auto_allow(&self) -> String {
        serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "autoApprove": true,
                "permissionDecision": "allow"
            }
        }).to_string()
    }

    fn format_context(&self, text: &str) -> String {
        serde_json::json!({
            "additionalContext": text
        }).to_string()
    }

    fn format_updated_output(&self, output: &serde_json::Value) -> String {
        serde_json::json!({
            "updatedMCPToolOutput": output
        }).to_string()
    }

    fn settings_path(&self) -> PathBuf {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".claude").join("settings.json")
    }

    fn generate_hooks_config(&self, binary_path: &std::path::Path) -> String {
        let bin = binary_path.to_string_lossy().replace('\\', "/");
        format!(r#"{{
  "hooks": {{
    "PreToolUse": [
      {{ "matcher": "Bash", "hooks": [{{ "type": "command", "command": "{bin} pretool-bash" }}] }},
      {{ "matcher": "Read", "hooks": [{{ "type": "command", "command": "{bin} pretool-read" }}] }},
      {{ "matcher": "Write|Edit|MultiEdit", "hooks": [{{ "type": "command", "command": "{bin} pretool-write" }}] }},
      {{ "matcher": "Grep|Glob|aidex_signature", "hooks": [{{ "type": "command", "command": "{bin} pretool-redirect" }}] }}
    ],
    "PostToolUse": [
      {{ "matcher": "Bash|Write|Edit|MultiEdit", "hooks": [{{ "type": "command", "command": "{bin} posttool-session" }}] }},
      {{ "matcher": "mcp__", "hooks": [{{ "type": "command", "command": "{bin} posttool-mcp" }}] }}
    ],
    "PermissionRequest": [
      {{ "matcher": "", "hooks": [{{ "type": "command", "command": "{bin} permission-approve" }}] }}
    ],
    "SessionStart": [
      {{ "matcher": "", "hooks": [{{ "type": "command", "command": "{bin} session-start" }}] }}
    ],
    "SessionEnd": [
      {{ "matcher": "", "hooks": [{{ "type": "command", "command": "{bin} session-end" }}] }}
    ],
    "UserPromptSubmit": [
      {{ "matcher": "", "hooks": [{{ "type": "command", "command": "{bin} userprompt-context" }}] }}
    ],
    "PreCompact": [
      {{ "matcher": "", "hooks": [{{ "type": "command", "command": "{bin} precompact-memory" }}] }}
    ],
    "Stop": [
      {{ "matcher": "", "hooks": [{{ "type": "command", "command": "{bin} stop-check" }}] }}
    ],
    "PostToolUseFailure": [
      {{ "matcher": "", "hooks": [{{ "type": "command", "command": "{bin} postfailure-guide" }}] }}
    ]
  }}
}}"#)
    }
}
