// ─── stop_check — anti-drift stop gate ──────────────────────────────────────
//
// Stop hook that blocks Claude from finishing if work is incomplete.
// Only blocks once per stop cycle to prevent infinite loops.
//
// Logic:
//   1. If stop_hook_active == true: exit immediately (loop prevention)
//   2. Read last 2KB of session-notes.jsonl
//   3. Check for edits to aidex-supported files without subsequent aidex_update
//   4. Count unresolved errors (errors after last milestone)
//   5. Errors -> block. Missing aidex_update -> advisory only (not blocking).
//   6. If clean: exit silently (allow stop)
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::config;
use std::path::Path;

pub fn run(raw: &str) {
    let input = common::parse_input_or_return!(raw);

    // SAFETY: If stop_hook_active is true, always allow to prevent infinite loops.
    // This is the FIRST check — must come before any other logic.
    if input.stop_hook_active.unwrap_or(false) {
        return;
    }

    let project_dir = common::project_dir();
    let session_path = project_dir.join(crate::constants::SESSION_NOTES_FILE);

    if !session_path.exists() {
        common::log("stop-check", "No session file — allow");
        return;
    }

    // Check if current project has .aidex/ (only enforce aidex_update if it does)
    let has_aidex = aidex_exists();

    // Read last 2KB of session file
    let tail = common::read_tail(&session_path, 2048);
    let lines: Vec<&str> = tail.lines().collect();

    let mut has_recent_edit = false;
    let mut has_aidex_update = false;
    let mut unresolved_errors = 0u32;

    // Scan backwards through entries
    for line in lines.iter().rev() {
        let entry = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let note_type = entry
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match note_type {
            "edit" => {
                // Only count edits to aidex-supported file extensions
                let detail = entry
                    .get("detail")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if is_aidex_supported_file(detail) {
                    has_recent_edit = true;
                }
            }
            "error" => {
                unresolved_errors += 1;
            }
            "milestone" => {
                let detail = entry
                    .get("detail")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if detail.contains("aidex_update") {
                    has_aidex_update = true;
                }
                // Milestone resets error count (errors before milestone are resolved)
                break;
            }
            _ => {}
        }
    }

    // Errors still block — this is the valuable check
    if unresolved_errors > 0 {
        let reason = format!("Incomplete: {} unresolved error(s)", unresolved_errors);
        common::log("stop-check", &format!("BLOCK — {}", reason));
        common::stop_block(&reason);
        return;
    }

    // Missing aidex_update is advisory-only (not blocking)
    // Only fire if .aidex/ exists in the project AND edits happened to supported files
    // AND enough edits to warrant the advisory (>= 5)
    let aidex_dir_exists = std::env::current_dir()
        .map(|d| d.join(".aidex").is_dir())
        .unwrap_or(false);
    if has_aidex && aidex_dir_exists && has_recent_edit && !has_aidex_update {
        let state = common::read_session_state();
        if state.files_edited.len() >= 5 {
            common::log("stop-check", "ADVISORY — edits without aidex_update");
            common::additional_context(
                "Consider running aidex_update for indexed files before ending the session.",
            );
            return;
        }
    }

    common::log("stop-check", "PASS — clean stop");
}

/// Check if a file path has an aidex-supported extension
fn is_aidex_supported_file(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    config::AIDEX_EXTS.contains(&ext.as_str())
}

/// Check if .aidex/ exists in cwd or common project roots
fn aidex_exists() -> bool {
    // Check cwd first
    if Path::new(".aidex").exists() {
        return true;
    }
    // Check if PWD env points somewhere with .aidex
    if let Ok(pwd) = std::env::var("PWD")
        && Path::new(&pwd).join(".aidex").exists() {
            return true;
        }
    false
}
