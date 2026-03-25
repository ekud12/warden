// ─── common::output — hook output writers ────────────────────────────────────
//
// All output goes through write_json(). In CLI mode it writes to stdout.
// In daemon mode, start_capture() activates a thread-local buffer that
// collects output; take_capture() retrieves it after the handler returns.

use super::types::*;
use serde_json::Value;
use std::cell::RefCell;
use std::io;

// ─── Capture buffer for daemon in-process dispatch ───────────────────────────

thread_local! {
    static CAPTURE_BUF: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

/// Activate the capture buffer (daemon calls this before dispatching a handler)
pub fn start_capture() {
    CAPTURE_BUF.with(|buf| *buf.borrow_mut() = Some(Vec::with_capacity(512)));
}

/// Take the captured output and reset (daemon calls this after handler returns)
pub fn take_capture() -> String {
    CAPTURE_BUF.with(|buf| {
        buf.borrow_mut()
            .take()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    })
}

/// Write serialized JSON — to capture buffer if active, otherwise to stdout
fn write_json<T: serde::Serialize>(value: &T) {
    CAPTURE_BUF.with(|buf| {
        let mut borrow = buf.borrow_mut();
        if let Some(ref mut vec) = *borrow {
            let _ = serde_json::to_writer(vec, value);
        } else {
            let _ = serde_json::to_writer(io::stdout(), value);
        }
    });
}

// ─── Hook output helpers ─────────────────────────────────────────────────────

/// Write PreToolUse deny
pub fn deny(event: &str, message: &str) {
    let out = PreToolDeny {
        hook_specific_output: PreToolDenyInner {
            hook_event_name: event.to_string(),
            permission_decision: "deny".to_string(),
        },
        system_message: message.to_string(),
    };
    write_json(&out);
}

/// Write PreToolUse deny with inline rationale (rule ID + opt-out).
/// The agent sees the rule ID in every denial — no need to call explain.
pub fn deny_with_id(event: &str, message: &str, restriction_id: &str) {
    let full_message = format!(
        "{} [{}]\nTo disable: `{} restrictions disable {}`",
        message, restriction_id, crate::constants::NAME, restriction_id
    );
    let out = PreToolDeny {
        hook_specific_output: PreToolDenyInner {
            hook_event_name: event.to_string(),
            permission_decision: "deny".to_string(),
        },
        system_message: full_message,
    };
    write_json(&out);
}

/// Write PreToolUse allow-with-updatedInput
pub fn allow_with_update(event: &str, input: Value) {
    let out = PreToolAllow {
        hook_specific_output: PreToolAllowInner {
            hook_event_name: event.to_string(),
            permission_decision: "allow".to_string(),
            updated_input: input,
        },
    };
    write_json(&out);
}

/// Write PreToolUse allow (bypasses permission prompt, no payload)
pub fn allow(event: &str) {
    let out = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": event,
            "permissionDecision": "allow"
        }
    });
    write_json(&out);
}

/// Write PreToolUse allow-with-advisory (command runs, advisory injected)
/// Uses additionalContext (invisible to user) instead of systemMessage (visible)
pub fn allow_with_advisory(event: &str, message: &str) {
    let out = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": event,
            "permissionDecision": "allow"
        },
        "additionalContext": message
    });
    write_json(&out);
}

/// Write additionalContext (for SessionStart, PreCompact)
pub fn additional_context(ctx: &str) {
    let out = SystemMsg {
        system_message: None,
        additional_context: Some(ctx.to_string()),
    };
    write_json(&out);
}

/// Write Stop hook block decision
pub fn stop_block(reason: &str) {
    let out = StopBlock {
        decision: "block".to_string(),
        reason: reason.to_string(),
    };
    write_json(&out);
}

/// Write PermissionRequest approve decision
pub fn permission_approve() {
    let out = serde_json::json!({
        "hookSpecificOutput": {
            "decision": {
                "behavior": "approve"
            }
        }
    });
    write_json(&out);
}

/// Write PostToolUse with replaced MCP tool output
pub fn updated_mcp_output(output: &serde_json::Value) {
    let out = serde_json::json!({"updatedMCPToolOutput": output});
    write_json(&out);
}
