// ─── assistant::gemini_cli — Gemini CLI adapter ─────────────────────────────

use super::{Assistant, HookInput};
use std::path::PathBuf;

#[derive(Default)]
pub struct GeminiCli;

impl Assistant for GeminiCli {
    fn name(&self) -> &str {
        "gemini-cli"
    }

    fn parse_input(&self, raw: &str) -> Option<HookInput> {
        // Gemini CLI uses similar JSON but with different field names
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        Some(HookInput {
            tool_name: v
                .get("tool")
                .and_then(|t| t.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string()),
            tool_input: v.get("tool").and_then(|t| t.get("arguments")).cloned(),
            tool_output: v.get("tool").and_then(|t| t.get("result")).cloned(),
            command: v
                .get("tool")
                .and_then(|t| t.get("arguments"))
                .and_then(|a| a.get("command"))
                .and_then(|c| c.as_str())
                .map(|s| s.to_string()),
            exit_code: v
                .get("tool")
                .and_then(|t| t.get("result"))
                .and_then(|r| r.get("exitCode"))
                .and_then(|e| e.as_i64()),
            session_id: v
                .get("session_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            reason: v
                .get("reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            hook_event: v
                .get("hook_event")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            agent_type: v
                .get("agent_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
    }

    fn format_deny(&self, _event: &str, message: &str) -> String {
        serde_json::json!({
            "decision": "deny",
            "reason": message
        })
        .to_string()
    }

    fn format_allow(&self, advisory: Option<&str>) -> String {
        let mut out = serde_json::json!({ "decision": "allow" });
        if let Some(msg) = advisory {
            out["systemMessage"] = serde_json::json!(msg);
        }
        out.to_string()
    }

    fn format_auto_allow(&self) -> String {
        serde_json::json!({ "decision": "allow" }).to_string()
    }

    fn format_context(&self, text: &str) -> String {
        serde_json::json!({
            "systemMessage": text
        })
        .to_string()
    }

    fn format_updated_output(&self, output: &serde_json::Value) -> String {
        serde_json::json!({
            "updatedOutput": output
        })
        .to_string()
    }

    fn settings_path(&self) -> PathBuf {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".gemini").join("settings.json")
    }

    fn generate_hooks_config(&self, binary_path: &std::path::Path) -> String {
        let bin = binary_path.to_string_lossy().replace('\\', "/");
        // Gemini CLI uses BeforeTool/AfterTool event names
        format!(
            r#"{{
  "hooks": {{
    "BeforeTool": [
      {{ "matcher": "run_shell_command", "hooks": [{{ "type": "command", "command": "{bin} pretool-bash" }}] }},
      {{ "matcher": "read_file", "hooks": [{{ "type": "command", "command": "{bin} pretool-read" }}] }},
      {{ "matcher": "write_file|replace", "hooks": [{{ "type": "command", "command": "{bin} pretool-write" }}] }}
    ],
    "AfterTool": [
      {{ "matcher": "run_shell_command|write_file|replace", "hooks": [{{ "type": "command", "command": "{bin} posttool-session" }}] }}
    ],
    "SessionStart": [
      {{ "matcher": "", "hooks": [{{ "type": "command", "command": "{bin} session-start" }}] }}
    ],
    "SessionEnd": [
      {{ "matcher": "", "hooks": [{{ "type": "command", "command": "{bin} session-end" }}] }}
    ]
  }}
}}"#
        )
    }
}
