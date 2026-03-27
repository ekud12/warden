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
    "daemon-status",
    "daemon-stop",
    "allow",
    "status",
    "daemon-start",
    "daemon-restart",
    "rules",
    "session",
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
                        let sp = term::Spinner::start("Starting daemon...");
                        runtime::ipc::spawn_daemon();
                        sp.finish_ok("Daemon started");
                    } else {
                        term::status_ok("Daemon already running");
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
                        let sp = term::Spinner::start("Starting daemon...");
                        runtime::ipc::spawn_daemon();
                        sp.finish_ok("Daemon started");
                    } else {
                        term::status_ok("Daemon already running");
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
            install::update::run_doctor();
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
        "debug-daemon-stop" => {
            if let Some(resp) = runtime::ipc::try_daemon("shutdown", "") {
                if resp.exit_code == 0 {
                    eprintln!("Daemon stopped");
                }
            } else {
                eprintln!("Daemon not running");
            }
        }
        "debug-daemon-status" => {
            if let Some(resp) = runtime::ipc::try_daemon("daemon-status", "") {
                println!("{}", resp.stdout);
            } else {
                eprintln!("Daemon not running");
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
        "daemon-status" => {
            if let Some(resp) = runtime::ipc::try_daemon("daemon-status", "") {
                println!("{}", resp.stdout);
            } else {
                eprintln!("Daemon not running");
            }
        }
        "daemon-stop" => {
            if let Some(resp) = runtime::ipc::try_daemon("shutdown", "") {
                if resp.exit_code == 0 {
                    eprintln!("Daemon stopped");
                }
            } else {
                eprintln!("Daemon not running");
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

        "daemon-start" => {
            if runtime::ipc::daemon_is_running() {
                eprintln!("Daemon already running.");
            } else {
                runtime::ipc::spawn_daemon();
                std::thread::sleep(std::time::Duration::from_millis(300));
                if runtime::ipc::daemon_is_running() {
                    eprintln!("Daemon started.");
                } else {
                    eprintln!("Failed to start daemon.");
                }
            }
        }

        "daemon-restart" => {
            eprintln!("Stopping daemon...");
            runtime::ipc::stop_daemon_graceful(2000);
            std::thread::sleep(std::time::Duration::from_millis(200));
            runtime::ipc::spawn_daemon();
            std::thread::sleep(std::time::Duration::from_millis(300));
            if runtime::ipc::daemon_is_running() {
                eprintln!("Daemon restarted.");
            } else {
                eprintln!("Daemon stopped but failed to restart.");
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
                            let state_path = dir.join("session-state.json");
                            if let Ok(content) = std::fs::read_to_string(&state_path)
                                && let Ok(state) =
                                    serde_json::from_str::<serde_json::Value>(&content)
                            {
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
    term::print_colored(term::TEXT, "    daemon-status         ");
    term::println_colored(term::DIM, "Check daemon health");
    term::print_colored(term::TEXT, "    daemon-stop           ");
    term::println_colored(term::DIM, "Stop background daemon");
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
