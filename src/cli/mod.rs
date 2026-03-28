// ─── cli — user-facing command dispatch ──────────────────────────────────────
//
// Handles all CLI commands that users invoke directly (init, install, etc.)
// as well as help text, unknown command suggestions, and debug aliases.
// ──────────────────────────────────────────────────────────────────────────────

use crate::{
    assistant, common, config, constants, engines, handlers, install, rules, runtime, scorecard,
};
use std::process;

pub const USER_COMMANDS: &[&str] = &[
    "init",
    "install",
    "uninstall",
    "update",
    "config",
    "describe",
    "version",
    "mcp",
    "doctor",
    "explain",
    "stats",
    "scorecard",
    "replay",
    "tui",
    "export",
    "restrictions",
    "server-status",
    "server-stop",
    "allow",
    "status",
    "server-start",
    "server-restart",
    "rules",
    "session",
    "redb",
    "cleanup",
];

pub fn run(subcmd: &str, args: &[String]) {
    match subcmd {
        // ── Management commands (no stdin) ──
        "version" | "--version" | "-v" => {
            println!("{} {}", constants::NAME, env!("CARGO_PKG_VERSION"));
        }
        "init" => install::wizard::run(),
        "uninstall" => install::uninstall::run(),
        "install" => {
            let target = args.get(2).map(|s| s.as_str()).unwrap_or("");
            match target {
                "claude-code" => {
                    use install::term;
                    eprintln!();
                    term::print_bold(
                        term::BRAND,
                        &format!("  {} install claude-code\n", constants::NAME),
                    );
                    eprintln!();

                    // Check if already installed
                    let adapter = assistant::claude_code::ClaudeCode;
                    if is_already_installed(&adapter) {
                        term::status_warn("Warden is already installed for Claude Code.");
                        term::hint("Run `warden update` to check for updates.");
                        eprintln!();
                    }

                    let sp = term::Spinner::start("Setting up directories...");
                    let _ = install::ensure_dirs();
                    let _ = install::install_binary();
                    sp.finish_ok("Directories ready");

                    let sp = term::Spinner::start("Configuring hooks...");
                    install_assistant::<assistant::claude_code::ClaudeCode>();
                    sp.finish_ok("Claude Code hooks installed");

                    if !install::path::is_on_path() {
                        let sp = term::Spinner::start("Registering PATH...");
                        match install::path::add_to_path() {
                            Ok(msg) => sp.finish_ok(&msg),
                            Err(e) => sp.finish_warn(&format!("PATH: {}", e)),
                        }
                    } else {
                        term::status_ok("Already on PATH");
                    }

                    if !runtime::ipc::daemon_is_running() {
                        let sp = term::Spinner::start("Starting server...");
                        runtime::ipc::spawn_daemon();
                        sp.finish_ok("Server started");
                    } else {
                        term::status_ok("Server already running");
                    }

                    eprintln!();
                    term::print_bold(term::SUCCESS, "  Ready! ");
                    term::print_colored(
                        term::DIM,
                        "Start a Claude Code session and Warden will guard it.\n",
                    );
                    eprintln!();
                }
                "gemini-cli" => {
                    use install::term;
                    eprintln!();
                    term::print_bold(
                        term::BRAND,
                        &format!("  {} install gemini-cli\n", constants::NAME),
                    );
                    eprintln!();

                    // Check if already installed
                    let adapter = assistant::gemini_cli::GeminiCli;
                    if is_already_installed(&adapter) {
                        term::status_warn("Warden is already installed for Gemini CLI.");
                        term::hint("Run `warden update` to check for updates.");
                        eprintln!();
                    }

                    let sp = term::Spinner::start("Setting up directories...");
                    let _ = install::ensure_dirs();
                    let _ = install::install_binary();
                    sp.finish_ok("Directories ready");

                    let sp = term::Spinner::start("Configuring hooks...");
                    install_assistant::<assistant::gemini_cli::GeminiCli>();
                    sp.finish_ok("Gemini CLI hooks installed");

                    if !install::path::is_on_path() {
                        let sp = term::Spinner::start("Registering PATH...");
                        match install::path::add_to_path() {
                            Ok(msg) => sp.finish_ok(&msg),
                            Err(e) => sp.finish_warn(&format!("PATH: {}", e)),
                        }
                    } else {
                        term::status_ok("Already on PATH");
                    }

                    if !runtime::ipc::daemon_is_running() {
                        let sp = term::Spinner::start("Starting server...");
                        runtime::ipc::spawn_daemon();
                        sp.finish_ok("Server started");
                    } else {
                        term::status_ok("Server already running");
                    }

                    eprintln!();
                    term::print_bold(term::SUCCESS, "  Ready! ");
                    term::print_colored(
                        term::DIM,
                        "Start a Gemini CLI session and Warden will guard it.\n",
                    );
                    eprintln!();
                }
                _ => eprintln!(
                    "Usage: {} install <claude-code|gemini-cli>",
                    constants::NAME
                ),
            }
        }

        "update" => {
            install::update::run(&args[1..]);
        }

        "doctor" => {
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if sub == "intelligence" {
                run_doctor_intelligence();
            } else {
                install::update::run_doctor();
            }
        }

        // ── Config commands ──
        "config" => {
            let action = args.get(2).map(|s| s.as_str()).unwrap_or("list");
            let config_path = install::home_dir().join(constants::CONFIG_FILE);
            match action {
                "path" => println!("{}", config_path.display()),
                "list" => {
                    if let Ok(content) = std::fs::read_to_string(&config_path) {
                        print!("{}", content);
                    } else {
                        eprintln!("No config found. Run `{} init` first.", constants::NAME);
                    }
                }
                "set" => {
                    let key = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    let val = args.get(4).map(|s| s.as_str()).unwrap_or("");
                    if key.is_empty() || val.is_empty() {
                        eprintln!("Usage: {} config set <key> <value>", constants::NAME);
                    } else {
                        set_config_value(&config_path, key, val);
                    }
                }
                "get" => {
                    let key = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    if key.is_empty() {
                        eprintln!("Usage: {} config get <key>", constants::NAME);
                    } else {
                        get_config_value(&config_path, key);
                    }
                }
                "schema" => {
                    print!("{}", include_str!("../../schemas/config.schema.json"));
                }
                _ => eprintln!(
                    "Usage: {} config <list|get|set|path|schema>",
                    constants::NAME
                ),
            }
        }

        // ── No-stdin subcommands ──
        "describe" => engines::harbor::describe::run(&args[1..]),
        "debug-explain" => {
            let target = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if target.is_empty() {
                eprintln!("Usage: {} debug-explain <rule-id>", constants::NAME);
            } else {
                engines::harbor::explain::explain_rule(target);
            }
        }
        "debug-explain-session" => engines::harbor::explain::explain_session(),
        "project-dir" => println!("{}", crate::common::project_dir().display()),
        "rules" => {
            let action = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if action == "schema" {
                print!("{}", include_str!("../../schemas/rules.schema.json"));
            } else {
                let r = &*rules::RULES;
                let out = serde_json::json!({
                    "safety": r.safety_pairs.len(),
                    "substitutions": r.substitutions_pairs.len(),
                    "advisories": r.advisories_pairs.len(),
                    "auto_allow": r.auto_allow_patterns.len(),
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
            }
        }
        "debug-restrictions" => {
            let action = args.get(2).map(|s| s.as_str()).unwrap_or("");
            match action {
                "enable" => {
                    let id = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    if id.is_empty() {
                        eprintln!(
                            "Usage: {} restrictions enable <restriction-id>",
                            constants::NAME
                        );
                    } else {
                        toggle_restriction(id, false);
                    }
                }
                "disable" => {
                    let id = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    if id.is_empty() {
                        eprintln!(
                            "Usage: {} restrictions disable <restriction-id>",
                            constants::NAME
                        );
                    } else {
                        toggle_restriction(id, true);
                    }
                }
                "list" => {
                    let filter = if args.get(3).map(|s| s.as_str()) == Some("--disabled") {
                        Some("disabled")
                    } else {
                        None
                    };
                    if filter == Some("disabled") {
                        let config_path = install::home_dir().join(constants::CONFIG_FILE);
                        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
                        eprintln!("Disabled restrictions (from config.toml):");
                        for line in content.lines() {
                            if line.trim().starts_with("disabled") {
                                eprintln!("  {}", line.trim());
                            }
                        }
                    } else {
                        config::restrictions::run(&args[2..]);
                    }
                }
                _ => config::restrictions::run(&args[2..]),
            }
        }
        "debug-export" => engines::harbor::export_sessions::run(&args[2..]),
        "debug-stats" => print!("{}", engines::dream::lore::format_stats()),
        "debug-scorecard" => scorecard::run(),
        "debug-replay" => engines::harbor::replay::run(&args[2..]),
        "debug-diff" if args.len() >= 4 => {
            engines::harbor::replay::run(&["diff".to_string(), args[2].clone(), args[3].clone()]);
        }
        "debug-tui" => {
            if let Err(e) = engines::harbor::tui::run() {
                eprintln!("TUI error: {}", e);
                process::exit(1);
            }
        }
        "mcp" => engines::harbor::mcp::run(),
        "truncate-filter" => handlers::truncate_filter::run(),

        // ── Daemon subcommands ──
        "daemon" => {
            // This shouldn't be reached via cli::run, but handle gracefully
            let mtime: u64 = args
                .get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(runtime::ipc::get_binary_mtime);
            crate::runtime::daemon::run_server(mtime);
        }
        "debug-server-stop" => {
            if let Some(resp) = runtime::ipc::try_daemon("shutdown", "") {
                if resp.exit_code == 0 {
                    eprintln!("Server stopped");
                }
            } else {
                eprintln!("Server not running");
            }
        }
        "debug-server-status" => {
            if let Some(resp) = runtime::ipc::try_daemon("server-status", "") {
                println!("{}", resp.stdout);
            } else {
                eprintln!("Server not running");
                process::exit(1);
            }
        }

        // ── Clean command aliases (map to debug-* handlers) ──
        "explain" => {
            let target = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if target.is_empty() {
                eprintln!("Usage: {} explain <rule-id>", constants::NAME);
            } else {
                engines::harbor::explain::explain_rule(target);
            }
        }
        "explain-session" => engines::harbor::explain::explain_session(),
        "stats" => print!("{}", engines::dream::lore::format_stats()),
        "scorecard" => scorecard::run(),
        "benchmark" => crate::benchmark::run(&args[2..]),
        "replay" => engines::harbor::replay::run(&args[2..]),
        "tui" => {
            if let Err(e) = engines::harbor::tui::run() {
                eprintln!("TUI error: {}", e);
                process::exit(1);
            }
        }
        "export" | "export-sessions" => engines::harbor::export_sessions::run(&args[2..]),
        "restrictions" => {
            // Forward to debug-restrictions handler
            let action = args.get(2).map(|s| s.as_str()).unwrap_or("");
            match action {
                "enable" => {
                    let id = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    if !id.is_empty() {
                        toggle_restriction(id, false);
                    }
                }
                "disable" => {
                    let id = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    if !id.is_empty() {
                        toggle_restriction(id, true);
                    }
                }
                _ => config::restrictions::run(&args[2..]),
            }
        }
        "server-status" => {
            if let Some(resp) = runtime::ipc::try_daemon("server-status", "") {
                println!("{}", resp.stdout);
            } else {
                eprintln!("Server not running");
            }
        }
        "server-stop" => {
            if let Some(resp) = runtime::ipc::try_daemon("shutdown", "") {
                if resp.exit_code == 0 {
                    eprintln!("Server stopped");
                }
            } else {
                eprintln!("Server not running");
            }
        }

        // ── Phase 4: Appeal — one-time rule override ──
        "allow" => {
            let rule_id = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if rule_id.is_empty() {
                eprintln!("Usage: {} allow <rule_id>", constants::NAME);
                eprintln!(
                    "Adds a one-time override for the specified rule (current session only)."
                );
                process::exit(1);
            }
            let mut state = common::read_session_state();
            if !state.allowed_overrides.contains(&rule_id.to_string()) {
                state.allowed_overrides.push(rule_id.to_string());
                common::write_session_state(&state);
                eprintln!("Override added for '{}' (this session only).", rule_id);
            } else {
                eprintln!("'{}' is already overridden.", rule_id);
            }
        }

        // ── Phase 5: Missing CLI commands ──
        "state" => {
            // Dump raw session state as JSON to stdout (for tests + debugging)
            if let Ok(cwd) = std::env::current_dir() {
                common::set_project_cwd(&cwd.to_string_lossy());
            }
            let state = common::read_session_state();
            if let Ok(json) = serde_json::to_string_pretty(&state) {
                println!("{}", json);
            }
        }

        "status" => {
            let state = common::read_session_state();
            let phase = &state.adaptive.phase;
            let trust = crate::engines::anchor::trust::compute_trust(&state);
            let focus = crate::engines::anchor::focus::compute_focus(&state);
            eprintln!(
                "Turn: {}  Phase: {:?}  Trust: {}  Focus: {}  Errors: {}  Milestone: {}",
                state.turn,
                phase,
                trust,
                focus.score,
                state.errors_unresolved,
                if state.last_milestone.is_empty() {
                    "none"
                } else {
                    &state.last_milestone
                }
            );
            if !state.session_goal.is_empty() {
                eprintln!("Goal: {}", state.session_goal);
            }
        }

        "server-start" => {
            if runtime::ipc::daemon_is_running() {
                eprintln!("Server already running.");
            } else {
                // v2.4: spawn warden.exe __server directly (no binary copy needed)
                runtime::server::spawn();
                std::thread::sleep(std::time::Duration::from_millis(300));
                if runtime::ipc::daemon_is_running() {
                    eprintln!("Server started.");
                } else {
                    eprintln!("Failed to start server.");
                }
            }
        }

        "server-restart" => {
            eprintln!("Stopping server...");
            runtime::ipc::stop_daemon_graceful(2000);
            std::thread::sleep(std::time::Duration::from_millis(200));
            runtime::ipc::spawn_daemon();
            std::thread::sleep(std::time::Duration::from_millis(300));
            if runtime::ipc::daemon_is_running() {
                eprintln!("Server restarted.");
            } else {
                eprintln!("Server stopped but failed to restart.");
            }
        }

        "redb" => {
            let subcmd2 = args.get(2).map(|s| s.as_str()).unwrap_or("stats");
            // Open redb for current project
            let project_dir = common::project_dir();
            common::storage::open_db(&project_dir);
            match subcmd2 {
                "stats" => {
                    let events = common::storage::read_last_events(10000);
                    let diags = common::storage::read_last_diagnostics(10000);
                    eprintln!("  Database: {}/warden.redb", project_dir.display());
                    eprintln!("  Events:      {}", events.len());
                    eprintln!("  Diagnostics: {}", diags.len());
                    // Show table keys for dream/resume/stats
                    if let Some(state) =
                        common::storage::read_json::<serde_json::Value>("session_state", "current")
                    {
                        let turn = state.get("turn").and_then(|v| v.as_u64()).unwrap_or(0);
                        let phase = state
                            .get("adaptive")
                            .and_then(|a| a.get("phase"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        eprintln!("  Session:     turn {}, phase {}", turn, phase);
                    }
                }
                "diagnostics" | "diag" => {
                    let limit: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20);
                    let diags = common::storage::read_last_diagnostics(limit);
                    if diags.is_empty() {
                        eprintln!("  No diagnostic entries.");
                    } else {
                        for d in &diags {
                            let cat = d.get("cat").and_then(|v| v.as_str()).unwrap_or("?");
                            let detail = d.get("detail").and_then(|v| v.as_str()).unwrap_or("?");
                            let ts = d.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
                            let secs = ts / 1_000_000_000;
                            eprintln!("  [{}] {}: {}", secs, cat, common::truncate(detail, 100));
                        }
                    }
                }
                "events" => {
                    let limit: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20);
                    let events = common::storage::read_last_events(limit);
                    for raw in &events {
                        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(raw) {
                            let t = v.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                            let detail = v.get("detail").and_then(|v| v.as_str()).unwrap_or("");
                            eprintln!("  [{}] {}", t, common::truncate(detail, 100));
                        }
                    }
                    if events.is_empty() {
                        eprintln!("  No events.");
                    }
                }
                "dump" => {
                    let table = args.get(3).map(|s| s.as_str()).unwrap_or("session_state");
                    let key = args.get(4).map(|s| s.as_str()).unwrap_or("current");
                    if let Some(val) = common::storage::read_json::<serde_json::Value>(table, key) {
                        if let Ok(pretty) = serde_json::to_string_pretty(&val) {
                            println!("{}", pretty);
                        }
                    } else {
                        eprintln!("  No data for table='{}' key='{}'", table, key);
                    }
                }
                _ => eprintln!(
                    "Usage: {} redb [stats|diagnostics|events|dump <table> <key>]",
                    constants::NAME
                ),
            }
        }

        "session" => {
            let subcmd2 = args.get(2).map(|s| s.as_str()).unwrap_or("list");
            match subcmd2 {
                "list" => {
                    let projects_dir = common::hooks_dir().join("projects");
                    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
                        eprintln!(
                            "{:<10} {:<40} {:<6} {:<8}",
                            "Hash", "Project", "Turns", "Phase"
                        );
                        eprintln!("{}", "-".repeat(70));
                        for entry in entries.flatten() {
                            let dir = entry.path();
                            if !dir.is_dir() {
                                continue;
                            }
                            let hash = entry.file_name().to_string_lossy().to_string();
                            let project = std::fs::read_to_string(dir.join("project.txt"))
                                .unwrap_or_else(|_| "unknown".into())
                                .trim()
                                .to_string();
                            // Try redb first, fall back to JSON
                            let state_opt: Option<serde_json::Value> = {
                                common::storage::close();
                                common::storage::open_db(&dir);
                                common::storage::read_json::<serde_json::Value>("session_state", "current")
                            }.or_else(|| {
                                let state_path = dir.join("session-state.json");
                                std::fs::read_to_string(&state_path).ok()
                                    .and_then(|c| serde_json::from_str(&c).ok())
                            });
                            common::storage::close();
                            if let Some(state) = state_opt {
                                let turns = state["turn"].as_u64().unwrap_or(0);
                                let phase = state["adaptive"]["phase"].as_str().unwrap_or("?");
                                eprintln!(
                                    "{:<10} {:<40} {:<6} {:<8}",
                                    hash,
                                    common::truncate(&project, 38),
                                    turns,
                                    phase
                                );
                            }
                        }
                    }
                }
                "end" => {
                    let mut state = common::read_session_state();
                    state.turn = 0;
                    common::write_session_state(&state);
                    eprintln!("Session ended (state reset).");
                }
                _ => eprintln!("Usage: {} session [list|end]", constants::NAME),
            }
        }

        "cleanup" => {
            run_cleanup(args);
        }

        _ => {
            if !subcmd.is_empty() {
                use install::term;
                eprintln!();
                term::print_colored(term::ERROR, &format!("  Unknown command: {}\n", subcmd));

                // Find closest match
                let mut best = ("", usize::MAX);
                for &cmd in USER_COMMANDS {
                    let dist = levenshtein(subcmd, cmd);
                    if dist < best.1 {
                        best = (cmd, dist);
                    }
                }
                if best.1 <= 3 && !best.0.is_empty() {
                    term::print_colored(term::DIM, "  Did you mean ");
                    term::print_bold(term::TEXT, best.0);
                    term::print_colored(term::DIM, "?\n");
                }
                eprintln!();
                term::hint(&format!(
                    "Run `{} --help` for available commands.",
                    constants::NAME
                ));
                eprintln!();
            } else {
                print_help();
            }
            process::exit(0);
        }
    }
}

fn run_cleanup(args: &[String]) {
    use install::term;
    let dry_run = args.iter().any(|a| a == "--dry-run" || a == "-n");
    let force = args.iter().any(|a| a == "--force" || a == "-f");
    let stale_days: u64 = args
        .iter()
        .position(|a| a == "--days")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let projects_dir = common::hooks_dir().join("projects");
    if !projects_dir.exists() {
        eprintln!("No projects directory found.");
        return;
    }

    let now = std::time::SystemTime::now();
    let stale_threshold = std::time::Duration::from_secs(stale_days * 86400);
    let mut stale_dirs: Vec<(std::path::PathBuf, String, u64)> = Vec::new();
    let mut active_count = 0u32;

    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let hash = entry.file_name().to_string_lossy().to_string();
            let project_name = std::fs::read_to_string(dir.join("project.txt"))
                .unwrap_or_else(|_| "unknown".into())
                .trim()
                .to_string();

            // Check most recent mtime across key files
            let mut latest_mtime = std::time::SystemTime::UNIX_EPOCH;
            for name in ["warden.redb", "warden.db", "session-state.json", "session-notes.jsonl"] {
                let p = dir.join(name);
                if let Ok(meta) = std::fs::metadata(&p) {
                    if let Ok(mt) = meta.modified() {
                        if mt > latest_mtime {
                            latest_mtime = mt;
                        }
                    }
                }
            }

            let age = now.duration_since(latest_mtime).unwrap_or_default();
            if age > stale_threshold {
                let days = age.as_secs() / 86400;
                stale_dirs.push((dir, format!("{} ({})", project_name, hash), days));
            } else {
                active_count += 1;
            }
        }
    }

    // Check for old global warden.db
    let global_db = common::hooks_dir().join("warden.db");
    let has_global_db = global_db.exists();

    if stale_dirs.is_empty() && !has_global_db {
        eprintln!("Nothing to clean up. {} active project(s).", active_count);
        return;
    }

    eprintln!(
        "Found {} stale project(s) (>{} days), {} active.",
        stale_dirs.len(),
        stale_days,
        active_count
    );

    if !stale_dirs.is_empty() {
        eprintln!();
        for (_, name, days) in &stale_dirs {
            eprintln!("  {} ({} days old)", name, days);
        }
    }

    if has_global_db {
        eprintln!();
        eprintln!("  Legacy global warden.db found at {}", global_db.display());
    }

    if dry_run {
        eprintln!("\nDry run — no changes made. Use --force to delete.");
        return;
    }

    if !force {
        eprintln!();
        eprint!("Delete stale projects? [y/N] ");
        use std::io::BufRead;
        let mut answer = String::new();
        let _ = std::io::stdin().lock().read_line(&mut answer);
        if !answer.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted.");
            return;
        }
    }

    let mut deleted = 0u32;
    for (dir, name, _) in &stale_dirs {
        match std::fs::remove_dir_all(dir) {
            Ok(_) => {
                term::print_colored(term::SUCCESS, &format!("  Deleted: {}\n", name));
                deleted += 1;
            }
            Err(e) => {
                term::print_colored(term::ERROR, &format!("  Failed to delete {}: {}\n", name, e));
            }
        }
    }

    if has_global_db {
        match std::fs::remove_file(&global_db) {
            Ok(_) => {
                term::print_colored(term::SUCCESS, "  Removed legacy global warden.db\n");
            }
            Err(e) => {
                term::print_colored(
                    term::ERROR,
                    &format!("  Failed to remove global warden.db: {}\n", e),
                );
            }
        }
    }

    eprintln!("\nCleaned up {} project(s).", deleted);
}

fn install_assistant<A: assistant::Assistant + Default>() {
    let adapter = A::default();
    let binary_name = if cfg!(windows) {
        "warden-relay.exe"
    } else {
        "warden"
    };
    let binary_path = install::bin_dir().join(binary_name);
    install_relay();

    let hooks_json = adapter.generate_hooks_config(&binary_path);
    let settings_path = adapter.settings_path();

    let hooks_value: serde_json::Value = match serde_json::from_str(&hooks_json) {
        Ok(v) => v,
        Err(_) => return,
    };

    let mut settings: serde_json::Value = if settings_path.exists() {
        match std::fs::read_to_string(&settings_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or(serde_json::json!({})),
            Err(_) => serde_json::json!({}),
        }
    } else {
        if let Some(parent) = settings_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        serde_json::json!({})
    };

    // Backup existing settings before any modification
    if settings_path.exists() {
        let backup_path = settings_path.with_extension("json.bak");
        let _ = std::fs::copy(&settings_path, &backup_path);
    }

    // Merge hooks: update only Warden-owned entries, preserve non-Warden hooks
    if let Some(new_hooks) = hooks_value.get("hooks").and_then(|h| h.as_object()) {
        let existing_hooks = settings
            .get("hooks")
            .and_then(|h| h.as_object())
            .cloned()
            .unwrap_or_default();

        let mut merged = serde_json::Map::new();

        // For each event type in the new Warden config
        for (event, new_entries) in new_hooks {
            let mut event_hooks: Vec<serde_json::Value> = Vec::new();

            // Keep non-Warden hooks from existing config for this event
            if let Some(existing_entries) = existing_hooks.get(event).and_then(|e| e.as_array()) {
                for entry in existing_entries {
                    if !is_warden_hook(entry) {
                        event_hooks.push(entry.clone());
                    }
                }
            }

            // Add all Warden hooks from the new config
            if let Some(new_arr) = new_entries.as_array() {
                for entry in new_arr {
                    event_hooks.push(entry.clone());
                }
            }

            merged.insert(event.clone(), serde_json::Value::Array(event_hooks));
        }

        // Preserve event types that exist in settings but not in Warden's config
        for (event, entries) in &existing_hooks {
            if !merged.contains_key(event) {
                merged.insert(event.clone(), entries.clone());
            }
        }

        settings["hooks"] = serde_json::Value::Object(merged);
    }

    if let Ok(output) = serde_json::to_string_pretty(&settings) {
        let _ = std::fs::write(&settings_path, &output);
    }
}

/// Check if a hook entry is owned by Warden (command contains warden/warden-relay)
fn is_warden_hook(entry: &serde_json::Value) -> bool {
    // Check nested hooks array: { matcher, hooks: [{ type, command }] }
    if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
        for hook in hooks {
            if let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) {
                let cmd_lower = cmd.to_lowercase();
                if cmd_lower.contains("warden") {
                    return true;
                }
            }
        }
    }
    // Also check top-level command (Gemini CLI format)
    if let Some(cmd) = entry.get("command").and_then(|c| c.as_str()) {
        let cmd_lower = cmd.to_lowercase();
        if cmd_lower.contains("warden") {
            return true;
        }
    }
    false
}

/// Install the relay binary next to warden.exe
fn install_relay() {
    let source = std::env::current_exe().unwrap_or_default();
    let source_dir = source.parent().unwrap_or(std::path::Path::new("."));
    let relay_name = if cfg!(windows) {
        "warden-relay.exe"
    } else {
        "warden-relay"
    };
    let relay_src = source_dir.join(relay_name);

    let dest = install::bin_dir().join(relay_name);

    if relay_src.exists() && relay_src != dest {
        let _ = std::fs::copy(&relay_src, &dest);
    }
}

fn toggle_restriction(id: &str, disable: bool) {
    let config_path = install::home_dir().join(constants::CONFIG_FILE);
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();

    // Parse current disabled list
    let mut disabled: Vec<String> = Vec::new();
    let mut in_restrictions = false;
    for line in content.lines() {
        if line.trim() == "[restrictions]" {
            in_restrictions = true;
        } else if line.trim().starts_with('[') {
            in_restrictions = false;
        } else if in_restrictions && line.trim().starts_with("disabled") {
            // Parse: disabled = ["a", "b"]
            if let Some(arr_str) = line.split('=').nth(1) {
                let trimmed = arr_str.trim().trim_matches('[').trim_matches(']');
                for item in trimmed.split(',') {
                    let clean = item.trim().trim_matches('"').trim_matches('\'').trim();
                    if !clean.is_empty() {
                        disabled.push(clean.to_string());
                    }
                }
            }
        }
    }

    if disable {
        if !disabled.iter().any(|d| d == id) {
            disabled.push(id.to_string());
            eprintln!("Disabled restriction: {}", id);
        } else {
            eprintln!("Already disabled: {}", id);
            return;
        }
    } else {
        let before = disabled.len();
        disabled.retain(|d| d != id);
        if disabled.len() == before {
            eprintln!("Not disabled: {}", id);
            return;
        }
        eprintln!("Enabled restriction: {}", id);
    }

    // Write back — update or create [restrictions] section
    let disabled_str = disabled
        .iter()
        .map(|d| format!("\"{}\"", d))
        .collect::<Vec<_>>()
        .join(", ");
    let new_line = format!("disabled = [{}]", disabled_str);

    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut found_section = false;
    let mut found_key = false;

    for (i, line) in lines.iter_mut().enumerate() {
        if line.trim() == "[restrictions]" {
            found_section = true;
        } else if found_section && line.trim().starts_with("disabled") {
            *line = new_line.clone();
            found_key = true;
            break;
        } else if found_section && line.trim().starts_with('[') {
            // Insert before next section
            lines.insert(i, new_line.clone());
            found_key = true;
            break;
        }
    }

    if !found_section {
        lines.push(String::new());
        lines.push("[restrictions]".to_string());
        lines.push(new_line);
    } else if !found_key {
        // Section exists but no disabled key — find end and append
        let mut insert_at = lines.len();
        let mut in_sect = false;
        for (i, line) in lines.iter().enumerate() {
            if line.trim() == "[restrictions]" {
                in_sect = true;
            } else if in_sect && line.trim().starts_with('[') {
                insert_at = i;
                break;
            } else if in_sect {
                insert_at = i + 1;
            }
        }
        lines.insert(insert_at, new_line);
    }

    let _ = std::fs::write(&config_path, lines.join("\n"));
}

fn set_config_value(path: &std::path::Path, key: &str, value: &str) {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    // Parse dotted key: "tools.justfile" -> section "[tools]", key "justfile"
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() != 2 {
        eprintln!("Key must be section.key format (e.g., tools.justfile)");
        return;
    }
    let (section, field) = (parts[0], parts[1]);
    let section_header = format!("[{}]", section);

    // Find or create section, then set key
    let mut section_idx = None;
    let mut key_idx = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == section_header {
            section_idx = Some(i);
        } else if section_idx.is_some() && trimmed.starts_with('[') {
            break; // Next section
        } else if section_idx.is_some()
            && (trimmed.starts_with(&format!("{} =", field))
                || trimmed.starts_with(&format!("# {} =", field))
                || trimmed.starts_with(&format!("#{} =", field)))
        {
            key_idx = Some(i);
        }
    }

    let new_line = format!("{} = {}", field, value);

    if let Some(idx) = key_idx {
        lines[idx] = new_line;
    } else if let Some(idx) = section_idx {
        lines.insert(idx + 1, new_line);
    } else {
        lines.push(String::new());
        lines.push(section_header);
        lines.push(new_line);
    }

    if std::fs::write(path, lines.join("\n")).is_ok() {
        eprintln!("Set {}.{} = {}", section, field, value);
    } else {
        eprintln!("Failed to write config");
    }
}

fn get_config_value(path: &std::path::Path, key: &str) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No config found");
            return;
        }
    };

    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() != 2 {
        eprintln!("Key must be section.key format");
        return;
    }
    let (section, field) = (parts[0], parts[1]);
    let section_header = format!("[{}]", section);

    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
        } else if in_section && trimmed.starts_with('[') {
            break;
        } else if in_section
            && trimmed.starts_with(&format!("{} =", field))
            && let Some(val) = trimmed.split('=').nth(1)
        {
            println!("{}", val.trim());
            return;
        }
    }
    eprintln!("{}: not set", key);
}

/// Check if warden hooks are already present in an assistant's settings file.
fn is_already_installed(adapter: &dyn assistant::Assistant) -> bool {
    let settings_path = adapter.settings_path();
    let content = match std::fs::read_to_string(&settings_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let settings: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    // Check if any hook command contains "warden"
    if let Some(hooks) = settings.get("hooks") {
        let hooks_str = hooks.to_string();
        return hooks_str.contains("warden");
    }
    false
}

/// Check GitHub releases for the latest version. Returns Some("vX.Y.Z") or None.
#[allow(dead_code)]
fn check_latest_version() -> Option<String> {
    // Try PowerShell on Windows, curl elsewhere
    let output = if cfg!(windows) {
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Invoke-RestMethod -Uri 'https://api.github.com/repos/ekud12/warden/releases/latest' -Headers @{'User-Agent'='warden'}).tag_name",
            ])
            .output()
            .ok()?
    } else {
        std::process::Command::new("curl")
            .args([
                "-s",
                "-H",
                "User-Agent: warden",
                "https://api.github.com/repos/ekud12/warden/releases/latest",
            ])
            .output()
            .ok()?
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() || !output.status.success() {
        return None;
    }

    if cfg!(windows) {
        // PowerShell already extracted tag_name
        if stdout.starts_with('v')
            || stdout
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        {
            Some(stdout)
        } else {
            None
        }
    } else {
        // Parse JSON from curl output
        let parsed: serde_json::Value = serde_json::from_str(&stdout).ok()?;
        parsed
            .get("tag_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

fn print_help() {
    use install::term;

    eprintln!();
    term::print_bold(term::BRAND, "  W A R D E N");
    term::print_colored(term::DIM, &format!("  v{}\n", env!("CARGO_PKG_VERSION")));
    term::print_colored(term::DIM, "  Runtime guardian for AI coding agents\n");
    eprintln!();
    term::print_colored(term::ACCENT, "  ───────────────────────────────────────\n");
    eprintln!();
    term::print_bold(term::TEXT, "  USAGE\n");
    term::print_colored(term::DIM, &format!("    {} <command>\n", constants::NAME));
    eprintln!();
    term::print_bold(term::TEXT, "  COMMANDS\n");
    term::print_colored(term::TEXT, "    init                  ");
    term::println_colored(term::DIM, "Interactive setup wizard");
    term::print_colored(term::TEXT, "    install <assistant>   ");
    term::println_colored(term::DIM, "Configure hooks (claude-code, gemini-cli)");
    term::print_colored(term::TEXT, "    update                ");
    term::println_colored(
        term::DIM,
        "Check + apply updates (--check print-only, --yes skip prompt)",
    );
    term::print_colored(term::TEXT, "    uninstall             ");
    term::println_colored(term::DIM, "Remove hooks, binary, and config");
    term::print_colored(term::TEXT, "    config                ");
    term::println_colored(term::DIM, "View or modify configuration");
    term::print_colored(term::TEXT, "    describe              ");
    term::println_colored(
        term::DIM,
        "Show active user overrides (--all for full dump)",
    );
    term::print_colored(term::TEXT, "    doctor                ");
    term::println_colored(term::DIM, "Verify installation health");
    term::print_colored(term::TEXT, "    version               ");
    term::println_colored(term::DIM, "Print version");
    eprintln!();
    term::print_bold(term::TEXT, "  DIAGNOSTICS\n");
    term::print_colored(term::TEXT, "    explain <rule>        ");
    term::println_colored(term::DIM, "Why a rule fired, with context");
    term::print_colored(term::TEXT, "    stats                 ");
    term::println_colored(term::DIM, "Learning and analytics data");
    term::print_colored(term::TEXT, "    scorecard             ");
    term::println_colored(term::DIM, "Session quality scorecard");
    term::print_colored(term::TEXT, "    replay                ");
    term::println_colored(term::DIM, "Replay a past session");
    term::print_colored(term::TEXT, "    tui                   ");
    term::println_colored(term::DIM, "Interactive terminal dashboard");
    term::print_colored(term::TEXT, "    export                ");
    term::println_colored(term::DIM, "Export session data");
    term::print_colored(term::TEXT, "    server-status         ");
    term::println_colored(term::DIM, "Check background server health");
    term::print_colored(term::TEXT, "    server-stop           ");
    term::println_colored(term::DIM, "Stop background server");
    eprintln!();
    term::print_bold(term::TEXT, "  GETTING STARTED\n");
    term::print_colored(
        term::DIM,
        "    1. warden init              Run the setup wizard\n",
    );
    term::print_colored(
        term::DIM,
        "    2. Start a coding session    Warden activates automatically\n",
    );
    eprintln!();
    term::print_colored(term::DIM, "  https://github.com/ekud12/warden\n");
    eprintln!();
}

fn run_doctor_intelligence() {
    use install::term;
    let tel = &config::CONFIG.telemetry;
    let state = common::read_session_state();
    let project_dir = common::project_dir();

    // Count events and find last turn per type — prefer redb, fall back to JSONL
    let mut event_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    let mut last_turn: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    let event_entries: Vec<serde_json::Value> = if common::storage::is_available() {
        let raw = common::storage::read_last_events(1000);
        raw.iter()
            .filter_map(|e| serde_json::from_slice(e).ok())
            .collect()
    } else {
        let session_path = project_dir.join("session-notes.jsonl");
        std::fs::read_to_string(&session_path)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    };
    for entry in &event_entries {
        let t = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !t.is_empty() {
            *event_counts.entry(t.to_string()).or_default() += 1;
            if let Some(turn_val) = entry.get("turn").and_then(|v| v.as_u64()) {
                let e = last_turn.entry(t.to_string()).or_default();
                if turn_val as u32 > *e {
                    *e = turn_val as u32;
                }
            }
        }
    }

    // Load intervention effectiveness scores
    let dream_scores = crate::engines::dream::get_intervention_scores();

    // Compute trust
    let trust = crate::engines::anchor::trust::compute_trust(&state);
    let budget = if trust > 85 {
        1
    } else if trust > 50 {
        3
    } else if trust > 25 {
        5
    } else {
        15
    };
    let budget_label = if trust > 85 {
        "minimal"
    } else if trust > 50 {
        "normal"
    } else if trust > 25 {
        "elevated"
    } else {
        "aggressive"
    };

    let phase = &state.adaptive.phase;
    term::print_bold(
        term::TEXT,
        &format!(
            "\nIntelligence diagnostics (turn {}, phase: {}, trust: {})\n\n",
            state.turn, phase, trust
        ),
    );

    // Table header
    let header = format!(
        "  {:<22} {:<8} {:<10} {:<8} {:<10} {}",
        "Feature", "Status", "Last Turn", "Events", "Injected", "Effect."
    );
    eprintln!("{header}");
    eprintln!("  {}", "─".repeat(76));

    // Feature definitions: (name, enabled, event_key, is_injected)
    // is_injected: true = competes for advisory budget, false = silent/logged only
    let features: Vec<(&str, bool, &str, bool)> = vec![
        ("phase_detection", true, "adaptation", true),
        ("goal_extraction", true, "goal", true),
        ("loop_detection", true, "loop", true),
        ("drift_detection", tel.drift_velocity, "drift", true),
        ("focus_scoring", true, "focus", true),
        ("verification_debt", true, "verification", true),
        ("compaction_forecast", tel.token_forecast, "forecast", false),
        ("anomaly_detection", tel.anomaly_detection, "anomaly", false),
        ("quality_score", tel.quality_predictor, "quality", false),
        ("markov_transitions", true, "markov", false),
        ("error_hints", tel.command_recovery, "error_hint", true),
        ("output_compression", tel.smart_truncation, "truncation", false),
    ];

    for (name, enabled, event_key, is_injected) in &features {
        let status = if *enabled { "active" } else { "off" };
        let count = event_counts.get(*event_key).copied().unwrap_or(0);
        let last = last_turn.get(*event_key).copied();

        let last_str = match last {
            Some(t) => format!("turn {}", t),
            None => "\u{2014}".to_string(),
        };
        let count_str = if count > 0 {
            format!("{}", count)
        } else {
            "\u{2014}".to_string()
        };
        let injected_str = if !*enabled {
            "off"
        } else if *is_injected {
            "yes"
        } else {
            "silent"
        };
        let effect_str = dream_scores
            .scores
            .get(*event_key)
            .map(|s| format!("{:.2}", s))
            .unwrap_or_else(|| "\u{2014}".to_string());

        let status_color = if *enabled { term::SUCCESS } else { term::DIM };
        eprint!("  {:<22} ", name);
        term::print_colored(status_color, &format!("{:<8} ", status));
        eprintln!(
            "{:<10} {:<8} {:<10} {}",
            last_str, count_str, injected_str, effect_str
        );
    }

    // Footer
    eprintln!();
    eprintln!(
        "  Advisory budget: {} ({} \u{2014} trust {})",
        budget, budget_label, trust
    );
    if !state.session_goal.is_empty() {
        eprintln!("  Session goal: \"{}\"", state.session_goal);
    }
    eprintln!(
        "  Files edited: {} | Errors unresolved: {}",
        state.files_edited.len(),
        state.errors_unresolved
    );
    eprintln!();
}
