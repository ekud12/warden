// ─── subagent_stop — SubagentStop output validator + truncation ──────────────
//
// Validates subagent output quality and truncates verbose responses.
//
//   1. Blocks empty/trivially short results
//   2. Truncates outputs exceeding MAX_OUTPUT_CHARS to reduce orchestrator
//      context consumption — keeps first + last sections with a summary marker
//
// Fails open (exits 0, no output) on any error.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;

/// Minimum useful output length (chars)
const MIN_OUTPUT_LEN: usize = 50;

/// Maximum output before truncation kicks in (chars)
/// ~8K chars ≈ ~2K tokens — enough for any useful result
const MAX_OUTPUT_CHARS: usize = 8000;

/// How much to keep from the start and end when truncating
const KEEP_HEAD: usize = 3000;
const KEEP_TAIL: usize = 2000;

pub fn run(raw: &str) {
    let input = common::parse_input_or_return!(raw);

    let agent_type = input.agent_type.as_deref().unwrap_or("unknown");

    // Try to find output content in multiple possible fields
    let output_text = extract_output(raw);

    match output_text {
        Some(text) if text.trim().is_empty() => {
            common::log(
                "subagent-stop",
                &format!("BLOCK empty output from {} agent", agent_type),
            );
            common::stop_block(&format!(
                "Subagent ({}) returned empty output. Re-try with a more specific prompt or different agent type.",
                agent_type
            ));
        }
        Some(text) if text.trim().len() < MIN_OUTPUT_LEN => {
            common::log(
                "subagent-stop",
                &format!(
                    "BLOCK short output ({} chars) from {} agent",
                    text.trim().len(),
                    agent_type
                ),
            );
            common::stop_block(&format!(
                "Subagent ({}) returned insufficient output ({} chars). The result may be incomplete — consider re-trying with clearer instructions.",
                agent_type,
                text.trim().len()
            ));
        }
        Some(text) if text.len() > MAX_OUTPUT_CHARS => {
            let original_len = text.len();
            let truncated = truncate_output(&text);
            let saved = original_len - truncated.len();
            common::log(
                "subagent-stop",
                &format!(
                    "TRUNCATE {} agent: {} -> {} chars (saved {})",
                    agent_type,
                    original_len,
                    truncated.len(),
                    saved
                ),
            );

            // Track token savings
            let mut state = common::read_session_state();
            state.estimated_tokens_saved += (saved / 4) as u64; // rough chars-to-tokens
            state.savings_truncation += 1;
            common::write_session_state(&state);

            // Inject truncated version as additional context for the orchestrator
            common::additional_context(&format!(
                "[Subagent output truncated: {} -> {} chars]\n{}",
                original_len,
                truncated.len(),
                truncated
            ));
        }
        Some(text) => {
            common::log(
                "subagent-stop",
                &format!("PASS {} agent ({} chars)", agent_type, text.len()),
            );
        }
        None => {
            common::log(
                "subagent-stop",
                &format!("PASS {} agent (no output field to validate)", agent_type),
            );
        }
    }
}

/// Truncate verbose output keeping head + tail with a marker
fn truncate_output(text: &str) -> String {
    // Try to cut at line boundaries for cleaner output
    let head_end = text[..KEEP_HEAD].rfind('\n').unwrap_or(KEEP_HEAD);
    let tail_start_offset = text.len() - KEEP_TAIL;
    let tail_start = text[tail_start_offset..]
        .find('\n')
        .map(|i| tail_start_offset + i + 1)
        .unwrap_or(tail_start_offset);

    let omitted = tail_start - head_end;
    format!(
        "{}\n\n[... {} chars omitted ...]\n\n{}",
        &text[..head_end],
        omitted,
        &text[tail_start..]
    )
}

/// Extract subagent output text from the raw JSON, checking multiple possible field names
fn extract_output(raw: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;

    // Check "result" field (string)
    if let Some(s) = v.get("result").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }

    // Check "output" field (string)
    if let Some(s) = v.get("output").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }

    // Check "tool_output" field (object with possible content)
    if let Some(obj) = v.get("tool_output") {
        if let Some(s) = obj.as_str() {
            return Some(s.to_string());
        }
        if let Some(s) = obj.get("content").and_then(|v| v.as_str()) {
            return Some(s.to_string());
        }
        if let Some(s) = obj.get("result").and_then(|v| v.as_str()) {
            return Some(s.to_string());
        }
    }

    // Check "response" field (string)
    if let Some(s) = v.get("response").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }

    None
}
