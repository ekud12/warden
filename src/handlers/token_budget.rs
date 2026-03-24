// ─── token_budget — token estimation, budget logging, threshold ──────────────

use crate::common;
use serde_json::Value;
use std::fs;
use std::io::Write;

/// Track token usage from tool input/output. Called after posttool dispatch.
pub fn track(tool_input: Option<&Value>, tool_output: Option<&Value>) {
    let tokens_in = tool_input.map(estimate_tokens).unwrap_or(0);
    let tokens_out = tool_output.map(estimate_tokens).unwrap_or(0);

    if tokens_in == 0 && tokens_out == 0 {
        return;
    }

    let mut state = common::read_session_state();
    state.estimated_tokens_in += tokens_in;
    state.estimated_tokens_out += tokens_out;
    let turn = state.turn;
    let total = state.estimated_tokens_in + state.estimated_tokens_out;
    common::write_session_state(&state);

    // Append to token-budget.log
    let log_dir = common::project_dir().join("logs");
    let log_path = log_dir.join("token-budget.log");
    let _ = fs::create_dir_all(&log_dir);
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = writeln!(
            f,
            "{} turn={} in={} out={} total={}",
            common::now_iso(),
            turn,
            tokens_in,
            tokens_out,
            total
        );
    }
}

/// Check if token budget threshold exceeded. Returns advisory text if over threshold.
pub fn check_threshold(state: &common::SessionState) -> Option<String> {
    let total = state.estimated_tokens_in + state.estimated_tokens_out;
    let threshold = crate::rules::RULES.token_budget_advisory;
    if total > threshold {
        Some(format!(
            "Session token estimate: ~{}K. Context pressure is high — consider wrapping up current work.",
            total / 1000
        ))
    } else {
        None
    }
}

/// Estimate tokens from JSON value size (roughly 1 token per 4 chars)
fn estimate_tokens(value: &Value) -> u64 {
    match value {
        Value::String(s) => (s.len() as u64) / 4,
        Value::Object(map) => {
            let mut total = 0u64;
            for (k, v) in map {
                total += (k.len() as u64) / 4;
                total += estimate_tokens(v);
            }
            total
        }
        Value::Array(arr) => arr.iter().map(estimate_tokens).sum(),
        Value::Number(_) => 1,
        Value::Bool(_) => 1,
        Value::Null => 0,
    }
}
