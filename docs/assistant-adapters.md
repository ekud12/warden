# Assistant Adapters

Warden supports multiple AI coding assistants through an adapter pattern. The same binary, rules, and analytics work across all supported assistants.

## How Adapters Work

The `Assistant` trait (`src/assistant/mod.rs`) normalizes each assistant's JSON protocol into a common `HookInput` struct. All pipeline stages operate on `HookInput` and produce responses through the adapter's format methods.

```rust
pub trait Assistant: Send + Sync {
    fn name(&self) -> &str;
    fn parse_input(&self, raw: &str) -> Option<HookInput>;
    fn format_deny(&self, event: &str, message: &str) -> String;
    fn format_allow(&self, advisory: Option<&str>) -> String;
    fn format_auto_allow(&self) -> String;
    fn format_context(&self, text: &str) -> String;
    fn format_updated_output(&self, output: &Value) -> String;
    fn settings_path(&self) -> PathBuf;
    fn generate_hooks_config(&self, binary_path: &Path) -> String;
}
```

**Auto-detection** checks environment variables at runtime:

| Signal | Assistant |
|--------|-----------|
| `CLAUDE_SESSION_ID` or `CLAUDE_CODE_ENTRYPOINT` | Claude Code |
| `GEMINI_SESSION_ID` or `GEMINI_PROJECT_DIR` | Gemini CLI |
| Neither set | Claude Code (default) |

Override via `config.toml`:

```toml
[assistant]
type = "gemini-cli"  # "claude-code" | "gemini-cli" | "auto"
```

## Claude Code

**Module:** `src/assistant/claude_code.rs`

**Settings file:** `~/.claude/settings.json`

**Install:** `warden install claude-code`

### Hook Event Mapping

| Warden Subcommand | Claude Code Event | Matcher |
|-------------------|-------------------|---------|
| `pretool-bash` | PreToolUse | `Bash` |
| `pretool-read` | PreToolUse | `Read` |
| `pretool-write` | PreToolUse | `Write\|Edit\|MultiEdit` |
| `pretool-redirect` | PreToolUse | `Grep\|Glob\|aidex_signature` |
| `posttool-session` | PostToolUse | `Bash\|Write\|Edit\|MultiEdit` |
| `posttool-mcp` | PostToolUse | `mcp__` |
| `permission-approve` | PermissionRequest | (all) |
| `session-start` | SessionStart | (all) |
| `session-end` | SessionEnd | (all) |
| `userprompt-context` | UserPromptSubmit | (all) |
| `precompact-memory` | PreCompact | (all) |
| `stop-check` | Stop | (all) |
| `postfailure-guide` | PostToolUseFailure | (all) |

### JSON Input

```json
{
  "tool_name": "Bash",
  "tool_input": { "command": "grep -r foo ." },
  "hook_event_name": "PreToolUse",
  "session_id": "abc123"
}
```

### Response Formats

**Deny:**
```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny"
  },
  "systemMessage": "BLOCKED: Use rg instead of grep"
}
```

**Allow with advisory:**
```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow"
  },
  "additionalContext": "Advisory: consider using fd instead"
}
```

**Auto-allow (bypass permission prompt):**
```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "autoApprove": true,
    "permissionDecision": "allow"
  }
}
```

**Context injection (UserPromptSubmit, SessionStart):**
```json
{
  "additionalContext": "Turn 15. Phase: Productive. Quality: 72/100."
}
```

**Updated MCP output (PostToolUse for MCP tools):**
```json
{
  "updatedMCPToolOutput": { "truncated": "output" }
}
```

## Gemini CLI

**Module:** `src/assistant/gemini_cli.rs`

**Settings file:** `~/.gemini/settings.json`

**Install:** `warden install gemini-cli`

### Hook Event Mapping

| Warden Subcommand | Gemini CLI Event | Matcher |
|-------------------|------------------|---------|
| `pretool-bash` | BeforeTool | `run_shell_command` |
| `pretool-read` | BeforeTool | `read_file` |
| `pretool-write` | BeforeTool | `write_file\|replace` |
| `posttool-session` | AfterTool | `run_shell_command\|write_file\|replace` |
| `session-start` | SessionStart | (all) |
| `session-end` | SessionEnd | (all) |

### JSON Input

```json
{
  "tool": {
    "name": "run_shell_command",
    "arguments": { "command": "grep -r foo ." },
    "result": { "exitCode": 0, "output": "..." }
  },
  "hook_event": "BeforeTool",
  "session_id": "xyz789"
}
```

### Response Formats

**Deny:**
```json
{
  "decision": "deny",
  "reason": "BLOCKED: Use rg instead of grep"
}
```

**Allow with advisory:**
```json
{
  "decision": "allow",
  "systemMessage": "Advisory: consider using fd instead"
}
```

**Context injection:**
```json
{
  "systemMessage": "Turn 15. Phase: Productive."
}
```

**Updated output:**
```json
{
  "updatedOutput": { "truncated": "output" }
}
```

## Key Differences

| Aspect | Claude Code | Gemini CLI |
|--------|-------------|------------|
| Tool name location | `tool_name` (top-level) | `tool.name` (nested) |
| Tool input location | `tool_input` (top-level) | `tool.arguments` (nested) |
| Tool output location | `tool_response` / `tool_result` | `tool.result` |
| Deny mechanism | `permissionDecision: "deny"` | `decision: "deny"` |
| Context injection | `additionalContext` | `systemMessage` |
| Auto-approve | `autoApprove: true` | Not supported |
| Updated output | `updatedMCPToolOutput` | `updatedOutput` |
| Event names | PreToolUse, PostToolUse | BeforeTool, AfterTool |
| Tool names | Bash, Read, Write, Edit | run_shell_command, read_file, write_file |

## Adding a New Assistant

### 1. Create adapter module

Add `src/assistant/your_assistant.rs`:

```rust
use super::{Assistant, HookInput};
use std::path::PathBuf;

#[derive(Default)]
pub struct YourAssistant;

impl Assistant for YourAssistant {
    fn name(&self) -> &str { "your-assistant" }

    fn parse_input(&self, raw: &str) -> Option<HookInput> {
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        Some(HookInput {
            tool_name: /* extract from your format */,
            tool_input: /* extract from your format */,
            command: /* extract command string if bash-like */,
            // ... normalize all fields into HookInput
            ..Default::default()
        })
    }

    fn format_deny(&self, _event: &str, message: &str) -> String {
        // Return JSON your assistant expects for denial
        serde_json::json!({ "deny": true, "reason": message }).to_string()
    }

    fn format_allow(&self, advisory: Option<&str>) -> String {
        // Return JSON your assistant expects for allow
        let mut out = serde_json::json!({ "allow": true });
        if let Some(msg) = advisory {
            out["message"] = serde_json::json!(msg);
        }
        out.to_string()
    }

    fn format_auto_allow(&self) -> String {
        serde_json::json!({ "allow": true }).to_string()
    }

    fn format_context(&self, text: &str) -> String {
        serde_json::json!({ "context": text }).to_string()
    }

    fn format_updated_output(&self, output: &serde_json::Value) -> String {
        serde_json::json!({ "updatedOutput": output }).to_string()
    }

    fn settings_path(&self) -> PathBuf {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".your-assistant").join("settings.json")
    }

    fn generate_hooks_config(&self, binary_path: &std::path::Path) -> String {
        // Generate the hooks JSON block for your assistant's settings file
        let bin = binary_path.to_string_lossy().replace('\\', "/");
        format!(r#"{{ "hooks": {{ ... }} }}"#)
    }
}
```

### 2. Register the adapter

In `src/assistant/mod.rs`:

- Add `pub mod your_assistant;`
- Add environment variable detection to `detect_assistant()`:

```rust
if std::env::var("YOUR_ASSISTANT_SESSION").is_ok() {
    return Box::new(your_assistant::YourAssistant);
}
```

### 3. Add install target

In `src/main.rs`, add a match arm under the `install` command:

```rust
"your-assistant" => {
    let _ = install::ensure_dirs();
    let _ = install::install_binary();
    install_assistant::<assistant::your_assistant::YourAssistant>();
}
```

### 4. Map tool names

Your adapter's `parse_input` must normalize tool names to Warden's internal convention so pipeline matchers work correctly. The pipeline expects names like `Bash`, `Read`, `Write`, `Edit`, `Grep`, `Glob`.
