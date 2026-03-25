// ─── pretool_redirect — PreToolUse handler for tool denial/redirection ────────
//
// Denies or redirects tool calls that should use alternatives:
//   - Grep → rg (via Bash)
//   - Glob → fd (via Bash)
//   - aidex_signature for unsupported file extensions → outline/Read
//
// Matched by "Grep|Glob|mcp__aidex__aidex_signature" matcher in settings.json.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::config;

/// PreToolUse handler for Grep|Glob|aidex_signature — denies with redirect
pub fn run(raw: &str) {
    let input = common::parse_input_or_return!(raw);

    let tool = match input.tool_name.as_deref() {
        Some(t) => t,
        None => return,
    };

    match tool {
        "Grep" => {
            let pattern = input
                .tool_input
                .as_ref()
                .and_then(|v| v.get("pattern"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            record_redirect_denial();
            common::log(
                "pretool-redirect",
                &format!("DENY Grep: pattern={:?}", common::truncate(pattern, 60)),
            );
            common::deny("PreToolUse", "Grep\u{2192}rg");
        }
        "Glob" => {
            let pattern = input
                .tool_input
                .as_ref()
                .and_then(|v| v.get("pattern"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            record_redirect_denial();
            common::log(
                "pretool-redirect",
                &format!("DENY Glob: pattern={:?}", common::truncate(pattern, 60)),
            );
            common::deny("PreToolUse", "Glob\u{2192}fd");
        }
        "mcp__aidex__aidex_signature" => {
            check_aidex_extension(&input);
        }
        _ => {
            // Defensive — should never reach due to matcher
        }
    }
}

/// Record a redirect denial for drift detection
fn record_redirect_denial() {
    let mut state = common::read_session_state();
    state.record_denial();
    common::write_session_state(&state);
}

/// Deny aidex_signature calls for file extensions aidex can't parse.
/// Silently passes through (no output) for supported extensions.
fn check_aidex_extension(input: &common::HookInput) {
    let file = input
        .tool_input
        .as_ref()
        .and_then(|v| v.get("file"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if file.is_empty() {
        return; // No file param — let aidex handle it
    }

    let ext = file.rsplit('.').next().unwrap_or("").to_lowercase();
    if ext.is_empty() {
        return;
    }

    if !config::AIDEX_EXTS.contains(&ext.as_str()) {
        record_redirect_denial();
        common::log(
            "pretool-redirect",
            &format!(
                "DENY aidex_signature: .{} not supported (file={:?})",
                ext,
                common::truncate(file, 60)
            ),
        );
        common::deny(
            "PreToolUse",
            &format!(
                "aidex doesn't support .{} files (supported: {}). Use `outline` or `Read` directly.",
                ext,
                config::AIDEX_EXTS.join(", ")
            ),
        );
    }
    // else: silently pass through — permission-approve will auto-allow
}
