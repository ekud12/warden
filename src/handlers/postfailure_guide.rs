// ─── postfailure_guide — error recovery hints ──────────────────────────────
//
// PostToolUseFailure handler. When a tool fails, pattern-matches common
// errors and injects targeted recovery hints as additionalContext.
// ──────────────────────────────────────────────────────────────────────────────

use crate::analytics;
use crate::common;
use crate::config;
use regex::Regex;
use std::sync::LazyLock;

struct CompiledHint {
    pattern: Regex,
    hint: &'static str,
}

static HINTS: LazyLock<Vec<CompiledHint>> = LazyLock::new(|| {
    config::ERROR_HINTS
        .iter()
        .filter_map(|(pattern, hint)| {
            Regex::new(pattern).ok().map(|re| CompiledHint {
                pattern: re,
                hint,
            })
        })
        .collect()
});

pub fn run(raw: &str) {
    let input = common::parse_input_or_return!(raw);

    // Extract error text from multiple possible locations
    let error_text = extract_error_text(&input, raw);
    if error_text.is_empty() {
        return;
    }

    // CLI command recovery: check for "command not found" or bad flags
    let cmd = input.tool_input.as_ref()
        .and_then(|ti| ti.get("command"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    if let Some(recovery) = analytics::recovery::check_not_found(&error_text) {
        common::additional_context(&recovery);
        common::log("postfailure-guide", &format!("Recovery: {}", recovery));
        let mut state = common::read_session_state();
        state.errors_unresolved = state.errors_unresolved.saturating_add(1);
        common::write_session_state(&state);
        return;
    }
    if let Some(fix) = analytics::recovery::check_bad_flag(cmd, &error_text) {
        common::additional_context(&fix);
        common::log("postfailure-guide", &format!("Flag fix: {}", fix));
        let mut state = common::read_session_state();
        state.errors_unresolved = state.errors_unresolved.saturating_add(1);
        common::write_session_state(&state);
        return;
    }

    // Match against error hint patterns
    for hint in HINTS.iter() {
        if hint.pattern.is_match(&error_text) {
            common::additional_context(hint.hint);
            common::log("postfailure-guide", &format!("Hint: {}", hint.hint));

            // Track unresolved error count for session awareness
            let mut state = common::read_session_state();
            state.errors_unresolved = state.errors_unresolved.saturating_add(1);
            common::write_session_state(&state);
            return;
        }
    }
}

/// Extract error text from the hook input, checking multiple fields
fn extract_error_text(input: &common::HookInput, raw: &str) -> String {
    // Check explicit error field
    if let Some(ref err) = input.error
        && !err.is_empty() {
            return err.clone();
        }

    // Check tool_output.stderr
    if let Some(ref output) = input.tool_output {
        if let Some(stderr) = output.get("stderr").and_then(|v| v.as_str())
            && !stderr.is_empty() {
                return stderr.to_string();
            }
        // Check tool_output.error
        if let Some(err) = output.get("error").and_then(|v| v.as_str())
            && !err.is_empty() {
                return err.to_string();
            }
    }

    // Fallback: search the raw JSON for error-like content (limited to 2KB)
    let search_range = &raw[..raw.len().min(2048)];
    if search_range.contains("error") || search_range.contains("Error") {
        return search_range.to_string();
    }

    String::new()
}
