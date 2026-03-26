// ─── Engine: Anchor — Session Start ─────────────────────────────────────────
//
// Runs once at session start. Injects ONLY tool enforcement rules.
// All other context (aidex note, session activity, cross-project insights,
// git warnings, provider output) is stored silently in redb — available
// via MCP session_status on demand, never injected.
//
// Re-init guard: if turn > 0 (mid-session re-fire), skips state reset,
// only re-injects rules.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::constants;
use crate::engines::dream::lore;
use std::fs;

/// SessionStart hook — rules-only injection
/// All non-rules context stored silently in redb
pub fn run(raw: &str) {
    let _input = common::parse_input(raw);

    // Open redb storage for this project (creates DB + tables on first run)
    let proj_dir = common::project_dir();
    common::storage::open_db(&proj_dir);

    // Migrate legacy JSON files to redb on first run
    let legacy_state = proj_dir.join("session-state.json");
    if legacy_state.exists() {
        common::storage::migrate_from_json(&proj_dir);
        // Rename legacy files to .bak
        for name in [
            "session-state.json",
            "stats.json",
            "rule-effectiveness.json",
            "session-notes.jsonl",
        ] {
            let src = proj_dir.join(name);
            if src.exists() {
                let _ = fs::rename(
                    &src,
                    src.with_extension(format!(
                        "{}.bak",
                        src.extension().unwrap_or_default().to_str().unwrap_or("")
                    )),
                );
            }
        }
        common::log("session-start", "Migrated legacy JSON files to redb");
    }

    let existing = common::read_session_state();
    let is_reinit = existing.turn > 0;

    if is_reinit {
        // Mid-session re-fire (e.g. after deploy/daemon restart) — preserve state,
        // just re-inject rules and restart daemon if needed.
        common::log(
            "session-start",
            &format!("Re-init at turn {} (skipping state reset)", existing.turn),
        );
    } else {
        // True fresh session — reset state
        common::write_session_state(&common::SessionState::default());
        cleanup_stale_tmp();
        // Prune dream artifacts to caps on fresh session start
        crate::engines::dream::pruner::prune_on_session_start();
    }

    // A.9: Auto-detect project type + store in session state
    if !is_reinit {
        let cwd = std::env::current_dir().unwrap_or_default();
        let project_type = detect_project_type(&cwd);
        let mut state = common::read_session_state();
        state.project_type = project_type.to_string();
        common::write_session_state(&state);
        common::log("session-start", &format!("Project type: {}", project_type));
    }

    // A.6: Self-healing daemon — auto-start if not running
    if std::env::var("WARDEN_NO_DAEMON").is_err() && !crate::runtime::ipc::daemon_is_running() {
        // Clean stale PID if process is dead
        if let Some(pid) = crate::runtime::ipc::read_pid()
            && (!crate::runtime::ipc::pid_is_alive(pid) || !crate::runtime::ipc::pid_is_warden(pid))
        {
            crate::runtime::ipc::remove_pid_file();
        }
        crate::runtime::ipc::spawn_daemon();
        common::log("session-start", "Auto-started daemon");
    }

    // Persist WARDEN_HOME to CLAUDE_ENV_FILE if available (makes path available to Bash calls)
    persist_warden_home();

    let mut context_parts: Vec<String> = Vec::new();

    // Load tool enforcement rules from the active assistant's rules directory
    let rules_path = common::assistant_rules_dir().join("tool-enforcement.md");
    if let Ok(rules) = fs::read_to_string(&rules_path)
        && !rules.trim().is_empty()
    {
        context_parts.push(rules.trim().to_string());
    }

    // On re-init, skip heavy context — rules re-injection is enough
    if is_reinit {
        context_parts.push(format!(
            "{} re-initialized (daemon restart at turn {}). Session state preserved.",
            constants::NAME,
            existing.turn
        ));
        if !context_parts.is_empty() {
            common::additional_context(&context_parts.join("\n\n"));
        }
        common::log("session-start", "Re-init context loaded (lightweight)");
        return;
    }

    // ── Full init below (fresh session only) ──
    // Store context silently in redb (available via MCP on demand).
    // ONLY tool enforcement rules get injected into agent context.

    let cwd = std::env::current_dir().unwrap_or_default();

    // Aidex note → redb (silent)
    let aidex_note = cwd.join(".aidex").join("note.md");
    if let Ok(content) = fs::read_to_string(&aidex_note)
        && !content.trim().is_empty()
    {
        let _ = common::storage::write_json("dream", "aidex_note", &content.trim().to_string());
    }

    // Cross-session recurring errors → redb (silent)
    let session_path = common::project_dir().join("session-notes.jsonl");
    if let Some(recurring) = lore::detect_recurring(&session_path) {
        let _ = common::storage::write_json("dream", "cross_session_recurring", &recurring);
    }

    // Cross-project learning insights → redb (silent)
    if let Some(insights) = lore::get_insights() {
        let _ = common::storage::write_json("dream", "cross_project_insights", &insights);
    }

    // Git branch safety → redb (silent)
    let git_warnings = crate::handlers::git_guardian::check_branch_state();
    if !git_warnings.is_empty() {
        let _ = common::storage::write_json("dream", "git_warnings", &git_warnings);
    }

    // Session counter (tracked, no onboarding injection)
    let _session_count = increment_session_count();

    // Custom providers → redb (silent)
    let providers_dir = common::hooks_dir().join("providers");
    if providers_dir.is_dir()
        && let Ok(entries) = fs::read_dir(&providers_dir)
    {
        let mut provider_outputs: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(output) = common::subprocess::run_with_timeout(
                    path.to_str().unwrap_or(""),
                    &[],
                    std::time::Duration::from_secs(2),
                )
                && !output.stdout.trim().is_empty()
            {
                provider_outputs.push(output.stdout.trim().to_string());
            }
        }
        if !provider_outputs.is_empty() {
            let _ = common::storage::write_json("dream", "provider_outputs", &provider_outputs);
        }
    }

    // Inject ONLY tool enforcement rules
    if !context_parts.is_empty() {
        common::additional_context(&context_parts.join("\n\n"));
    }

    common::log("session-start", "Context loaded (rules-only)");
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
    let _ = std::fs::write(
        &stats_path,
        serde_json::to_string_pretty(&data).unwrap_or_default(),
    );
    count
}

/// Clean up stale .tmp files from previous crashes
fn cleanup_stale_tmp() {
    let hooks_dir = common::hooks_dir();
    if let Ok(entries) = fs::read_dir(hooks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "tmp").unwrap_or(false) {
                common::log(
                    "session-start",
                    &format!("Cleaning stale tmp: {:?}", path.file_name()),
                );
                let _ = fs::remove_file(&path);
            }
        }
    }
}

/// Auto-detect project type from workspace files
fn detect_project_type(cwd: &std::path::Path) -> &'static str {
    if cwd.join("Cargo.toml").exists() {
        "rust"
    } else if cwd.join("package.json").exists() {
        "node"
    } else if cwd.join("pyproject.toml").exists() || cwd.join("setup.py").exists() {
        "python"
    } else if cwd.join("go.mod").exists() {
        "go"
    } else if cwd.join("pom.xml").exists() || cwd.join("build.gradle").exists() {
        "java"
    } else if has_extension(cwd, "sln") || has_extension(cwd, "csproj") {
        "dotnet"
    } else if cwd.join("composer.json").exists() {
        "php"
    } else if cwd.join("Gemfile").exists() {
        "ruby"
    } else if cwd.join("Package.swift").exists() {
        "swift"
    } else {
        "unknown"
    }
}

/// Check if any file with the given extension exists in the directory (non-recursive)
fn has_extension(dir: &std::path::Path, ext: &str) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.path().extension().map(|x| x == ext).unwrap_or(false))
        })
        .unwrap_or(false)
}
