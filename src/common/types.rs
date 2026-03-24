// ─── common::types — hook input/output type definitions ──────────────────────

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Raw hook input from stdin (all hook events)
#[derive(Deserialize, Debug, Default)]
#[allow(dead_code)]
pub struct HookInput {
    pub tool_name: Option<String>,
    pub tool_input: Option<Value>,
    #[serde(alias = "tool_result", alias = "tool_response")]
    pub tool_output: Option<Value>,

    // Common fields sent by Claude Code
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub transcript_path: Option<String>,

    // SessionStart fields
    pub session_type: Option<String>,

    // PreCompact fields
    pub compact_type: Option<String>,

    // Stop fields
    pub stop_reason: Option<String>,
    pub stop_hook_active: Option<bool>,

    // UserPromptSubmit fields
    pub prompt: Option<String>,

    // PostToolUseFailure fields
    pub error: Option<String>,

    // SessionEnd fields
    pub reason: Option<String>,

    // SubagentStart fields
    pub agent_type: Option<String>,
}

/// PreToolUse deny output
#[derive(Serialize)]
pub struct PreToolDeny {
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: PreToolDenyInner,
    #[serde(rename = "systemMessage")]
    pub system_message: String,
}

#[derive(Serialize)]
pub struct PreToolDenyInner {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    #[serde(rename = "permissionDecision")]
    pub permission_decision: String,
}

/// PreToolUse allow-with-update output
#[derive(Serialize)]
pub struct PreToolAllow {
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: PreToolAllowInner,
}

#[derive(Serialize)]
pub struct PreToolAllowInner {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    #[serde(rename = "permissionDecision")]
    pub permission_decision: String,
    #[serde(rename = "updatedInput")]
    pub updated_input: Value,
}

/// SystemMessage-only output (for PostToolUse, SessionStart, PreCompact)
#[derive(Serialize)]
pub struct SystemMsg {
    #[serde(rename = "systemMessage", skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
    #[serde(rename = "additionalContext", skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

/// Stop hook block output
#[derive(Serialize)]
pub struct StopBlock {
    pub decision: String,
    pub reason: String,
}
