// ─── session_start — SessionStart hook for context injection ──────────────────
//
// Runs once at session start. Loads and injects additionalContext:
//
//   1. Tool enforcement rules from ~/.claude/rules/tool-enforcement.md
//   2. Aidex session memory from .aidex/note.md (if project has .aidex/)
//   3. Recent session activity (last 10 entries from session-notes.jsonl)
//   4. Justfile reminder (if Justfile exists in cwd)
//
// The rules injection ensures NEVER/ALWAYS tool policies are present in
// context from the very start. PreCompact re-injects them for survival.
//
// Re-init guard: if turn > 0 (mid-session re-fire after deploy/daemon restart),
// skips state reset and heavy context, only re-injects rules.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::constants;
use crate::handlers::cross_session;
use crate::handlers::learning;
use std::fs;
use std::path::Path;

/// SessionStart hook — context loader
/// Reads aidex_note + last session entries, returns as additionalContext
pub fn run(raw: &str) {
    let _input = common::parse_input(raw);

    let existing = common::read_session_state();
    let is_reinit = existing.turn > 0;

    if is_reinit {
        // Mid-session re-fire (e.g. after deploy/daemon restart) — preserve state,
        // just re-inject rules and restart daemon if needed.
        common::log("session-start", &format!("Re-init at turn {} (skipping state reset)", existing.turn));
    } else {
        // True fresh session — reset state
        common::write_session_state(&common::SessionState::default());
        cleanup_stale_tmp();
    }

    // TODO: Auto-start daemon if not running (ipc module not yet ported)
    // auto_start_daemon();

    // Persist WARDEN_HOME to CLAUDE_ENV_FILE if available (makes path available to Bash calls)
    persist_warden_home();

    let mut context_parts: Vec<String> = Vec::new();

    // Load tool enforcement rules from the active assistant's rules directory
    let rules_path = common::assistant_rules_dir().join("tool-enforcement.md");
    if let Ok(rules) = fs::read_to_string(&rules_path)
        && !rules.trim().is_empty() {
            context_parts.push(rules.trim().to_string());
        }

    // On re-init, skip heavy context — rules re-injection is enough
    if is_reinit {
        context_parts.push(format!(
            "{} re-initialized (daemon restart at turn {}). Session state preserved.",
            constants::NAME, existing.turn
        ));
        if !context_parts.is_empty() {
            common::additional_context(&context_parts.join("\n\n"));
        }
        common::log("session-start", "Re-init context loaded (lightweight)");
        return;
    }

    // ── Full init below (fresh session only) ──

    // Check for .aidex directory and note
    let cwd = std::env::current_dir().unwrap_or_default();
    let aidex_dir = cwd.join(".aidex");

    if aidex_dir.is_dir() {
        // Read aidex note if available
        let note_path = aidex_dir.join("note.md");
        if let Ok(content) = fs::read_to_string(&note_path)
            && !content.trim().is_empty() {
                context_parts.push(format!(
                    "## Session Memory (from aidex_note)\n{}",
                    content.trim()
                ));
            }
    }

    // Read last 10 entries from session-notes.jsonl (per-project)
    let project_dir = common::project_dir();
    let session_path = project_dir.join("session-notes.jsonl");
    if session_path.exists() {
        let tail = common::read_tail(&session_path, 4096);
        let lines: Vec<&str> = tail.lines().collect();
        let recent: Vec<&str> = if lines.len() > 10 {
            lines[lines.len() - 10..].to_vec()
        } else {
            lines
        };

        if !recent.is_empty() {
            let mut summary = String::from("## Recent Session Activity\n");
            for line in &recent {
                if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                    let note_type = entry
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let detail = entry
                        .get("detail")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    summary.push_str(&format!("- [{}] {}\n", note_type, detail));
                }
            }
            context_parts.push(summary);
        }
    }

    // Cross-session recurring error detection
    if let Some(recurring) = cross_session::detect_recurring(&session_path) {
        context_parts.push(recurring);
    }

    // Cross-project learning insights
    if let Some(insights) = learning::get_insights() {
        context_parts.push(insights);
    }

    // Git branch safety check
    let git_warnings = crate::handlers::git_guardian::check_branch_state();
    for warning in &git_warnings {
        context_parts.push(warning.clone());
    }

    // Progressive onboarding: track session count, limit features for new users
    let session_count = increment_session_count();
    if session_count <= 3 {
        context_parts.push(format!(
            "{} session #{} — safety rules active. Substitutions + analytics unlock after 3 sessions. \
             Skip: `{} config set onboarding.level full`",
            constants::NAME, session_count, constants::NAME
        ));
    }

    // Custom context providers: run scripts in ~/.warden/providers/
    let providers_dir = common::hooks_dir().join("providers");
    if providers_dir.is_dir()
        && let Ok(entries) = fs::read_dir(&providers_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(output) = common::subprocess::run_with_timeout(
                        path.to_str().unwrap_or(""), &[], std::time::Duration::from_secs(2),
                    )
                    && !output.stdout.trim().is_empty() {
                        context_parts.push(output.stdout.trim().to_string());
                    }
            }
        }

    // Check for Justfile
    if Path::new("Justfile").exists() || Path::new("justfile").exists() {
        context_parts
            .push("Justfile detected: use `just <recipe>` for all build/test/lint commands.".to_string());
    }

    if !context_parts.is_empty() {
        common::additional_context(&context_parts.join("\n\n"));
    }

    common::log("session-start", "Context loaded");
}

/// Write WARDEN_HOME to CLAUDE_ENV_FILE so it's available to all Bash calls in the session.
/// CLAUDE_ENV_FILE is only set by Claude Code during SessionStart hooks.
fn persist_warden_home() {
    let env_file = match std::env::var("CLAUDE_ENV_FILE") {
        Ok(f) if !f.is_empty() => f,
        _ => return,
    };
    let warden_home = common::hooks_dir();
    let line = format!("export WARDEN_HOME=\"{}\"\n", warden_home.display());
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&env_file)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(line.as_bytes())
        });
}

/// Increment and return the global session counter (stored in ~/.warden/stats.json)
fn increment_session_count() -> u32 {
    let stats_path = common::hooks_dir().join("stats.json");
    let mut count: u32 = std::fs::read_to_string(&stats_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("sessions_completed")?.as_u64())
        .unwrap_or(0) as u32;
    count += 1;
    let data = serde_json::json!({ "sessions_completed": count });
    let _ = std::fs::write(&stats_path, serde_json::to_string_pretty(&data).unwrap_or_default());
    count
}

/// Clean up stale .tmp files from previous crashes
fn cleanup_stale_tmp() {
    let hooks_dir = common::hooks_dir();
    if let Ok(entries) = fs::read_dir(hooks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "tmp").unwrap_or(false) {
                common::log("session-start", &format!("Cleaning stale tmp: {:?}", path.file_name()));
                let _ = fs::remove_file(&path);
            }
        }
    }
}
