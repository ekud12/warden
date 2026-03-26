// ─── Engine: Harbor — CLI: explain ────────────────────────────────────────────
//
// `warden explain <rule-id>` shows what a rule does, why it exists, and how
// to disable it. Transparency builds trust.
//
// `warden explain-session` shows every Warden intervention this session.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::config::restrictions::RESTRICTIONS;
use crate::constants;

/// Explain a specific rule by ID
pub fn explain_rule(rule_id: &str) {
    let restriction = RESTRICTIONS.iter().find(|r| r.id == rule_id);

    match restriction {
        Some(r) => {
            eprintln!("Rule: {}", r.id);
            eprintln!("Category: {}", r.category);
            eprintln!("Severity: {}", r.severity);
            eprintln!("Handler: {}", r.handler);
            eprintln!("Description: {}", r.description);
            eprintln!("Disableable: {}", if r.can_disable { "yes" } else { "no" });
            if r.can_disable {
                eprintln!(
                    "\nDisable: {} restrictions disable {}",
                    constants::NAME,
                    r.id
                );
                eprintln!(
                    "Re-enable: {} restrictions enable {}",
                    constants::NAME,
                    r.id
                );
            }
        }
        None => {
            // Try fuzzy match
            let matches: Vec<&crate::config::restrictions::Restriction> = RESTRICTIONS
                .iter()
                .filter(|r| {
                    r.id.contains(rule_id)
                        || r.description
                            .to_lowercase()
                            .contains(&rule_id.to_lowercase())
                })
                .collect();

            if matches.is_empty() {
                eprintln!(
                    "Rule '{}' not found. Run `{} restrictions list` to see all rules.",
                    rule_id,
                    constants::NAME
                );
            } else {
                eprintln!("No exact match for '{}'. Did you mean:\n", rule_id);
                for r in &matches {
                    eprintln!("  {} — {}", r.id, r.description);
                }
            }
        }
    }
}

/// Explain all Warden interventions this session
pub fn explain_session() {
    let project_dir = common::project_dir();

    // Read session notes
    let session_path = project_dir.join("session-notes.jsonl");
    let notes = std::fs::read_to_string(&session_path).unwrap_or_default();

    // Read logs
    let log_dir = project_dir.join("logs");
    let pretool_log = std::fs::read_to_string(log_dir.join("pretool-bash.log")).unwrap_or_default();
    let session_log =
        std::fs::read_to_string(log_dir.join("userprompt-context.log")).unwrap_or_default();
    let posttool_log =
        std::fs::read_to_string(log_dir.join("posttool-session.log")).unwrap_or_default();

    println!("# Warden Session Interventions\n");

    // Denials from pretool-bash log
    let denials: Vec<&str> = pretool_log
        .lines()
        .filter(|l| l.contains("[DENY]"))
        .collect();
    if !denials.is_empty() {
        println!("## Denials ({} total)\n", denials.len());
        for line in &denials {
            println!("  {}", line.trim());
        }
        println!();
    }

    // Advisories from logs
    let advisories: Vec<&str> = pretool_log
        .lines()
        .chain(session_log.lines())
        .chain(posttool_log.lines())
        .filter(|l| l.contains("[ADVISORY]") || l.contains("advisory"))
        .collect();
    if !advisories.is_empty() {
        println!("## Advisories ({} total)\n", advisories.len());
        for line in advisories.iter().take(20) {
            println!("  {}", line.trim());
        }
        if advisories.len() > 20 {
            println!("  ... and {} more", advisories.len() - 20);
        }
        println!();
    }

    // Session events from JSONL
    let mut events = Vec::new();
    for line in notes.lines() {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            let note_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let detail = entry.get("detail").and_then(|v| v.as_str()).unwrap_or("");
            match note_type {
                "error" | "milestone" | "session-end" | "session-summary" => {
                    events.push(format!("[{}] {}", note_type, detail));
                }
                _ => {}
            }
        }
    }
    if !events.is_empty() {
        println!("## Session Events ({} total)\n", events.len());
        for event in &events {
            println!("  {}", event);
        }
        println!();
    }

    // Session state summary
    let state = common::read_session_state();
    println!("## Session State\n");
    println!("  Turn: {}", state.turn);
    println!("  Phase: {}", state.adaptive.phase);
    println!("  Files edited: {}", state.files_edited.len());
    println!("  Files read: {}", state.files_read.len());
    println!("  Errors unresolved: {}", state.errors_unresolved);
    println!("  Tokens saved: ~{}K", state.estimated_tokens_saved / 1000);
    if state.context_switch_detected {
        println!("  Context switch: detected");
    }
    if !state.session_goal.is_empty() {
        println!("  Session goal: {}", state.session_goal);
    }
}
