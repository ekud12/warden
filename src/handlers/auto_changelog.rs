// ─── auto_changelog — session-end narrative generation ───────────────────────
//
// Generates a human-readable summary of what happened during the session.
// Called at session-end, writes to .warden/projects/{hash}/last-session.md.
// Feeds into PR descriptions, standups, and team updates.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;

/// Generate and save session changelog
pub fn generate(state: &common::SessionState) -> Option<String> {
    if state.turn < 3 {
        return None; // Too short for a meaningful summary
    }

    let mut sections = Vec::new();

    // Header
    sections.push(format!("# Session Summary ({})", common::now_iso()));
    sections.push(format!(
        "**Duration:** {} turns | **Phase:** {} | **Quality:** estimated from {} snapshots",
        state.turn,
        state.adaptive.phase,
        state.turn_snapshots.len()
    ));

    // Files edited
    if !state.files_edited.is_empty() {
        let mut edit_section = String::from("\n## Files Modified\n");
        for file in &state.files_edited {
            let short = file
                .rsplit('/')
                .next()
                .or_else(|| file.rsplit('\\').next())
                .unwrap_or(file);
            edit_section.push_str(&format!("- `{}`\n", short));
        }
        sections.push(edit_section);
    }

    // Files read (top 10 by frequency)
    if !state.files_read.is_empty() {
        let mut read_section = String::from("\n## Files Examined\n");
        let mut reads: Vec<(&String, &common::FileReadEntry)> = state.files_read.iter().collect();
        reads.sort_by(|a, b| b.1.turn.cmp(&a.1.turn));
        for (path, entry) in reads.iter().take(10) {
            let short = path
                .rsplit('/')
                .next()
                .or_else(|| path.rsplit('\\').next())
                .unwrap_or(path);
            read_section.push_str(&format!("- `{}` (turn {})\n", short, entry.turn));
        }
        sections.push(read_section);
    }

    // Session notes summary (errors, milestones) — redb primary, JSONL fallback
    {
        let event_entries: Vec<serde_json::Value> = if common::storage::is_available() {
            common::storage::read_last_events(500)
                .iter()
                .filter_map(|e| serde_json::from_slice(e).ok())
                .collect()
        } else {
            let project_dir = common::project_dir();
            let session_path = project_dir.join("session-notes.jsonl");
            std::fs::read_to_string(&session_path)
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        };
        let mut errors = Vec::new();
        let mut milestones = Vec::new();
        let mut edits_count = 0u32;

        for entry in &event_entries {
            let note_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let detail = entry.get("detail").and_then(|v| v.as_str()).unwrap_or("");
            match note_type {
                "error" if errors.len() < 10 => errors.push(detail.to_string()),
                "milestone" if milestones.len() < 10 => milestones.push(detail.to_string()),
                "edit" => edits_count += 1,
                _ => {}
            }
        }

        if !milestones.is_empty() {
            let mut ms = String::from("\n## Milestones\n");
            for m in &milestones {
                ms.push_str(&format!("- {}\n", m));
            }
            sections.push(ms);
        }

        if !errors.is_empty() {
            let mut es = String::from("\n## Errors Encountered\n");
            for e in &errors {
                es.push_str(&format!("- {}\n", e));
            }
            sections.push(es);
        }

        sections.push(format!(
            "\n## Stats\n- Total edits: {}\n- Errors: {}\n- Milestones: {}\n- Tokens saved: ~{}K",
            edits_count,
            errors.len(),
            milestones.len(),
            state.estimated_tokens_saved / 1000
        ));
    }

    let changelog = sections.join("\n");

    // Write to project dir
    let output_path = common::project_dir().join("last-session.md");
    let _ = std::fs::write(&output_path, &changelog);
    common::log(
        "auto-changelog",
        &format!("Wrote session summary to {}", output_path.display()),
    );

    Some(changelog)
}
