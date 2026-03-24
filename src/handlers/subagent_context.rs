// ─── subagent_context — subagent rules + token reduction ─────────────────────
//
// SubagentStart handler. Injects:
//   1. Condensed tool rules (substitutions, priorities)
//   2. Output format directive (terse responses, no narration)
//   3. Files-in-context hint (avoid redundant re-reads)
//   4. Parallelism scout hint
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;

/// Condensed rules for subagents (assistant-agnostic)
const SUBAGENT_RULES: &str = "\
Tool substitutions: grep->rg, find->fd, cat->bat, curl->xh. Use just <recipe> when Justfile exists.\n\
Prefer structured tools for symbol lookups and library docs over text search.\n\
Be terse. No AI/Claude/Copilot attribution in code or comments.";

/// Output format directive — reduces narration tokens
const OUTPUT_DIRECTIVE: &str = "\
Be terse. Return only the result: code, diff, answer, or file path. \
No preamble, no summaries, no explaining what you did. \
If you identify sub-tasks that could run in parallel, list them at the end.";

pub fn run(raw: &str) {
    let input = common::parse_input(raw);
    let agent_type = input.as_ref()
        .and_then(|i| i.agent_type.as_deref())
        .unwrap_or("unknown");

    let mut parts = vec![SUBAGENT_RULES.to_string(), OUTPUT_DIRECTIVE.to_string()];

    // Inject files-in-context hint so subagents skip redundant reads
    let state = common::read_session_state();
    if !state.files_read.is_empty() {
        let mut entries: Vec<(&String, &common::FileReadEntry)> = state.files_read.iter().collect();
        entries.sort_by(|a, b| b.1.turn.cmp(&a.1.turn));
        let files: Vec<&str> = entries.iter().take(8).map(|(p, _)| shorten(p)).collect();
        parts.push(format!("Orchestrator already read: {}. Skip re-reading these unless you need to edit them.", files.join(", ")));
    }

    common::additional_context(&parts.join("\n"));
    common::log("subagent-context", &format!("Injected rules for {} agent", agent_type));
}

/// Shorten path to last 2 components
fn shorten(path: &str) -> &str {
    let normalized = path.replace('\\', "/");
    // Find second-to-last slash
    let bytes = normalized.as_bytes();
    let mut slash_count = 0;
    for (i, &b) in bytes.iter().enumerate().rev() {
        if b == b'/' {
            slash_count += 1;
            if slash_count == 2 {
                return &path[i + 1..];
            }
        }
    }
    path
}
