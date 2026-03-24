// ─── posttool_mcp — MCP output trimming ─────────────────────────────────────
//
// PostToolUse handler for MCP tools (mcp__.*). Trims oversized JSON outputs
// to reduce token waste in context. MCP tools (aidex, docker, obsidian,
// umbraco) often return massive JSON blobs where only a fraction is useful.
//
// Logic:
//   1. Skip non-MCP tools (exit silently)
//   2. Measure serialized tool_output size
//   3. If ≤ MAX_MCP_OUTPUT: passthrough (no output)
//   4. If > MAX_MCP_OUTPUT: recursively trim JSON tree
//   5. Output {"updatedMCPToolOutput": <trimmed>}
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::config;
use crate::rules;
use serde_json::Value;

/// MCP tools whose output naturally contains instruction-like text — skip injection scan
const SKIP_INJECTION_SCAN: &[&str] = &[
    "mcp__playwright__",
    "mcp__context7__",
];

pub fn run(raw: &str) {
    let input = common::parse_input_or_return!(raw);

    // Only process MCP tools
    let tool_name = match &input.tool_name {
        Some(name) if name.starts_with("mcp__") => name.clone(),
        _ => return,
    };

    // Skip MCP tools that need full output (browser snapshots, screenshots, etc.)
    if is_full_output_tool(&tool_name) {
        return;
    }

    // Get tool_output
    let output = match &input.tool_output {
        Some(v) => v.clone(),
        None => return,
    };

    // Measure serialized size
    let serialized = match serde_json::to_string(&output) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Progressive compression: tighter limits as session progresses
    let max_output = get_max_mcp_output();

    // Injection scan on MCP output (skip web-content tools)
    if !SKIP_INJECTION_SCAN.iter().any(|prefix| tool_name.starts_with(prefix)) {
        let matches = common::sanitize::scan_for_injection(&serialized);
        if !matches.is_empty() {
            let warning = common::sanitize::build_warning(&matches);
            common::log("injection-detect", &format!("mcp-{}: {}", tool_name, warning));
            common::additional_context(&warning);
        }
    }

    let original_size = serialized.len();
    if original_size <= max_output {
        return; // Small enough — passthrough
    }

    // Trim the JSON tree
    let trimmed = trim_value(output);

    let trimmed_size = serde_json::to_string(&trimmed)
        .map(|s| s.len())
        .unwrap_or(0);

    // Record MCP trim savings
    let bytes_saved = original_size.saturating_sub(trimmed_size);
    let tokens_saved = (bytes_saved as u64) / 4; // ~1 token per 4 chars
    {
        let mut state = common::read_session_state();
        state.estimated_tokens_saved += tokens_saved;
        state.savings_truncation += 1;
        common::write_session_state(&state);
    }

    common::log(
        "posttool-mcp",
        &format!(
            "Trimmed {} output: {}B → {}B (~{} tokens saved)",
            tool_name, original_size, trimmed_size, tokens_saved
        ),
    );

    common::updated_mcp_output(&trimmed);
}

/// Progressive MCP output threshold based on session turn
fn get_max_mcp_output() -> usize {
    let state = common::read_session_state();

    // TODO: re-enable when adaptation ported
    // let adapted = state.adaptive.params.mcp_output_limit;
    let adapted: usize = 0; // TODO: use adaptive limit when available

    // Use adapted limit if available, else fall back to turn-based defaults
    if adapted > 0 {
        adapted
    } else {
        match state.turn {
            0..=15 => rules::RULES.max_mcp_output,
            16..=30 => 10_000,
            _ => 7_000,
        }
    }
}

/// MCP tools that need full unmodified output — never trim these
const FULL_OUTPUT_TOOLS: &[&str] = &[
    "mcp__playwright__",     // All playwright tools (snapshots, screenshots, DOM)
    "mcp__context7__",       // Library docs — trimming defeats the purpose
    "mcp__sequential-thinking__", // Reasoning chains must stay intact
];

fn is_full_output_tool(tool_name: &str) -> bool {
    FULL_OUTPUT_TOOLS.iter().any(|prefix| tool_name.starts_with(prefix))
}

/// Recursively trim a JSON value tree
fn trim_value(value: Value) -> Value {
    match value {
        Value::String(s) => trim_string(s),
        Value::Array(arr) => trim_array(arr),
        Value::Object(map) => {
            let trimmed: serde_json::Map<String, Value> = map
                .into_iter()
                .map(|(k, v)| (k, trim_value(v)))
                .collect();
            Value::Object(trimmed)
        }
        other => other, // Numbers, bools, nulls pass through
    }
}

/// Trim a string if it exceeds MAX_STRING_LEN
fn trim_string(s: String) -> Value {
    if s.len() <= rules::RULES.max_string_len {
        return Value::String(s);
    }

    let total = s.len();
    let head = &s[..config::STRING_KEEP_HEAD];
    let tail = &s[total - config::STRING_KEEP_TAIL..];
    let trimmed_count = total - config::STRING_KEEP_HEAD - config::STRING_KEEP_TAIL;

    Value::String(format!(
        "{}...({} chars trimmed)...{}",
        head, trimmed_count, tail
    ))
}

/// Trim an array if it exceeds MAX_ARRAY_LEN
fn trim_array(arr: Vec<Value>) -> Value {
    if arr.len() <= rules::RULES.max_array_len {
        // Still recurse into elements
        return Value::Array(arr.into_iter().map(trim_value).collect());
    }

    let total = arr.len();
    let mut result: Vec<Value> = arr[..config::ARRAY_KEEP_FIRST]
        .iter()
        .cloned()
        .map(trim_value)
        .collect();

    let trimmed_count = total - config::ARRAY_KEEP_FIRST - config::ARRAY_KEEP_LAST;
    result.push(Value::String(format!(
        "...({} items trimmed)...",
        trimmed_count
    )));

    let tail: Vec<Value> = arr[total - config::ARRAY_KEEP_LAST..]
        .iter()
        .cloned()
        .map(trim_value)
        .collect();
    result.extend(tail);

    Value::Array(result)
}
