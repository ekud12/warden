// ─── precompact_memory — PreCompact hook for compaction survival ──────────────
//
// Runs before context compaction. Generates a compact summary of session
// activity and re-injects critical context that must survive compaction:
//
//   1. Tool enforcement rules from ~/.claude/rules/tool-enforcement.md
//   2. Categorized session summary: errors, milestones, edited files
//   3. Diagnostic info (log file sizes)
//
// The rules re-injection is the key mechanism ensuring tool policies persist
// across context compaction boundaries.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use std::fs;

/// PreCompact hook — session memory saver
/// Reads recent session activity and returns summary as additionalContext
pub fn run(raw: &str) {
    let _input = common::parse_input(raw);

    let project_dir = common::project_dir();
    let mut context_parts: Vec<String> = Vec::new();

    // Mark compaction in session state + clear files_read (Claude loses file content memory)
    let mut state = common::read_session_state();
    state.last_compaction_turn = state.turn;
    state.files_read.clear(); // Post-compaction, Claude won't remember file contents
    common::write_session_state(&state);

    if state.turn > 0 {
        let mut state_lines = vec![format!("## Session State (turn {})", state.turn)];
        if !state.current_task.is_empty() {
            state_lines.push(format!("- Task: {}", state.current_task));
        }
        if !state.last_milestone.is_empty() {
            state_lines.push(format!("- Last milestone: {}", state.last_milestone));
        }
        state_lines.push(format!(
            "- Exploration: {} ops since last edit",
            state.explore_count
        ));
        state_lines.push(format!(
            "- Files examined: {} unique files",
            state.files_read.len()
        ));
        if !state.files_edited.is_empty() {
            state_lines.push(format!(
                "- Files edited: {}",
                state.files_edited.join(", ")
            ));
        }
        state_lines.push(format!(
            "- Unresolved errors: {}",
            state.errors_unresolved
        ));
        if !state.decisions.is_empty() {
            let decisions: Vec<String> = state
                .decisions
                .iter()
                .map(|d| format!("[{}]", d))
                .collect();
            state_lines.push(format!("- Decisions: {}", decisions.join(" ")));
        }
        context_parts.push(state_lines.join("\n"));
    }

    // Re-inject tool enforcement rules so they survive compaction
    let rules_path = common::assistant_rules_dir().join("tool-enforcement.md");
    if let Ok(rules) = fs::read_to_string(&rules_path)
        && !rules.trim().is_empty() {
            context_parts.push(rules.trim().to_string());
        }

    // Read session-notes.jsonl for recent activity (per-project)
    let session_path = project_dir.join(crate::constants::SESSION_NOTES_FILE);
    if session_path.exists() {
        // Read last 8KB for a good summary
        let tail = common::read_tail(&session_path, 8192);
        let lines: Vec<&str> = tail.lines().collect();

        let mut errors: Vec<String> = Vec::new();
        let mut milestones: Vec<String> = Vec::new();
        let mut edits: Vec<String> = Vec::new();

        for line in &lines {
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                let note_type = entry
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let detail = entry
                    .get("detail")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                match note_type {
                    "ts-error" | "lint-error" | "dep-error" | "test-fail" | "build-fail"
                    | "permission" | "missing-tool" | "knip-finding" => {
                        if errors.len() < 20 {
                            errors.push(format!("[{}] {}", note_type, detail));
                        }
                    }
                    "milestone" | "commit" => {
                        if milestones.len() < 15 {
                            milestones.push(detail);
                        }
                    }
                    "edit" => {
                        if edits.len() < 20 && !edits.contains(&detail) {
                            edits.push(detail);
                        }
                    }
                    _ => {}
                }
            }
        }

        if !errors.is_empty() {
            context_parts.push(format!(
                "## Errors This Session\n{}",
                errors.join("\n")
            ));
        }
        if !milestones.is_empty() {
            context_parts.push(format!(
                "## Milestones This Session\n- {}",
                milestones.join("\n- ")
            ));
        }
        if !edits.is_empty() {
            context_parts.push(format!(
                "## Files Edited This Session\n- {}",
                edits.join("\n- ")
            ));
        }
    }

    // Read posttool-session.log for diagnostic info (per-project)
    let log_path = project_dir.join("logs").join("posttool-session.log");
    if let Ok(meta) = fs::metadata(&log_path) {
        context_parts.push(format!(
            "Session log size: {}KB (logs/posttool-session.log)",
            meta.len() / 1024
        ));
    }

    if !context_parts.is_empty() {
        common::additional_context(&context_parts.join("\n\n"));
        common::log("precompact-memory", "Compaction summary generated");
    } else {
        common::log("precompact-memory", "No session data to summarize");
    }
}
