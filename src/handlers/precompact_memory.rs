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

    let _project_dir = common::project_dir();
    let mut context_parts: Vec<String> = Vec::new();

    // Mark compaction in session state + clear files_read (Claude loses file content memory)
    let mut state = common::read_session_state();
    state.last_compaction_turn = state.turn;
    state.files_read.clear(); // Post-compaction, Claude won't remember file contents
    common::write_session_state(&state);

    // A.10: Re-inject tool enforcement rules (must survive compaction)
    let rules_path = common::assistant_rules_dir().join("tool-enforcement.md");
    if let Ok(rules) = fs::read_to_string(&rules_path)
        && !rules.trim().is_empty()
    {
        context_parts.push(rules.trim().to_string());
    }

    // A.10: Compact resume packet from dream state (replaces verbose summary)
    if let Some(packet) = crate::dream::get_resume_packet() {
        let mut resume = format!("Session: {} turns.", state.turn);
        if !packet.high_salience_files.is_empty() {
            resume.push_str(&format!(
                " Files: {}.",
                packet.high_salience_files.join(", ")
            ));
        }
        if !packet.last_verified_state.is_empty() {
            resume.push_str(&format!(" Verified: {}.", packet.last_verified_state));
        }
        if !packet.current_issue.is_empty() {
            resume.push_str(&format!(" Blocked: {}.", packet.current_issue));
        }
        if !packet.dead_ends.is_empty() {
            resume.push_str(&format!(" Avoid: {}.", packet.dead_ends.join(", ")));
        }
        context_parts.push(resume);
    } else {
        // Fallback: minimal summary when dream state hasn't run
        let mut summary = format!("Session: {} turns", state.turn);
        if !state.files_edited.is_empty() {
            let recent: Vec<&str> = state
                .files_edited
                .iter()
                .rev()
                .take(5)
                .map(|s| s.as_str())
                .collect();
            summary.push_str(&format!(", edited: {}", recent.join(", ")));
        }
        if state.errors_unresolved > 0 {
            summary.push_str(&format!(", {} unresolved errors", state.errors_unresolved));
        }
        if !state.last_milestone.is_empty() {
            summary.push_str(&format!(", last milestone: {}", state.last_milestone));
        }
        context_parts.push(summary);
    }

    if !context_parts.is_empty() {
        let combined = context_parts.join("\n\n");
        // Phase 7.2: Track resume packet size for compaction validation
        common::log_structured(
            "precompact",
            common::LogLevel::Info,
            "resume-size",
            &format!("{}chars {}parts", combined.len(), context_parts.len()),
        );
        common::additional_context(&combined);
    } else {
        common::log("precompact-memory", "No session data to summarize");
    }
}
