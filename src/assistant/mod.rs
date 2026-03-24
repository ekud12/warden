// ─── assistant — multi-assistant adapter layer ───────────────────────────────
//
// Abstraction over AI coding assistants (Claude Code, Gemini CLI, etc.).
// Each assistant has different JSON formats for hook I/O, different event names,
// and different settings paths. The adapter trait normalizes these differences
// so all pipeline stages work identically regardless of assistant.
// ──────────────────────────────────────────────────────────────────────────────

pub mod claude_code;
pub mod gemini_cli;

use std::path::PathBuf;

/// Normalized hook input — assistant-independent
#[derive(Debug, Clone, Default)]
pub struct HookInput {
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_output: Option<serde_json::Value>,
    pub command: Option<String>,
    pub exit_code: Option<i64>,
    pub session_id: Option<String>,
    pub reason: Option<String>,
    pub hook_event: Option<String>,
    pub agent_type: Option<String>,
}

/// Trait implemented by each AI assistant adapter
pub trait Assistant: Send + Sync {
    /// Assistant name for logging
    fn name(&self) -> &str;

    /// Parse raw JSON stdin into normalized HookInput
    fn parse_input(&self, raw: &str) -> Option<HookInput>;

    /// Format a deny response for this assistant's protocol
    fn format_deny(&self, event: &str, message: &str) -> String;

    /// Format an allow response (optionally with advisory)
    fn format_allow(&self, advisory: Option<&str>) -> String;

    /// Format an allow with auto-approve (bypass permission prompt)
    fn format_auto_allow(&self) -> String;

    /// Format context injection (additionalContext or equivalent)
    fn format_context(&self, text: &str) -> String;

    /// Format updated tool output (for MCP trimming, etc.)
    fn format_updated_output(&self, output: &serde_json::Value) -> String;

    /// Path to this assistant's settings file
    fn settings_path(&self) -> PathBuf;

    /// Generate the hooks configuration JSON for this assistant
    fn generate_hooks_config(&self, binary_path: &std::path::Path) -> String;

    /// Directory where this assistant stores user rules (e.g. tool-enforcement.md)
    fn rules_dir(&self) -> PathBuf {
        self.settings_path().parent().map(|p| p.join("rules")).unwrap_or_default()
    }
}

/// Detect which assistant is running based on environment variables
pub fn detect_assistant() -> Box<dyn Assistant> {
    // Check for Claude Code env vars
    if std::env::var("CLAUDE_SESSION_ID").is_ok()
        || std::env::var("CLAUDE_CODE_ENTRYPOINT").is_ok()
    {
        return Box::new(claude_code::ClaudeCode);
    }

    // Check for Gemini CLI env vars
    if std::env::var("GEMINI_SESSION_ID").is_ok()
        || std::env::var("GEMINI_PROJECT_DIR").is_ok()
    {
        return Box::new(gemini_cli::GeminiCli);
    }

    // Default to Claude Code (most common)
    Box::new(claude_code::ClaudeCode)
}
