#![allow(dead_code)] // Application binary: many functions are called indirectly via handler dispatch
// ─── warden — AI Coding Session Guardian ─────────────────────────────────────
//
// Single-binary runtime intelligence layer for AI coding assistants.
// Supports Claude Code and Gemini CLI through adapter pattern.
// All hook handlers dispatched as subcommands.
//
// Architecture:
//   - Pipeline/middleware for hook processing (composable, panic-isolated)
//   - Multi-assistant adapter (same rules, different I/O formats)
//   - Tiered rules (core + community + personal + project)
//   - Runtime analytics (anomaly detection, forecasting, quality prediction)
//   - Phase-adaptive thresholds (Warmup → Productive → Exploring → Struggling → Late)
//
// Design: every handler exits 0 on error to never block the AI assistant.
// ──────────────────────────────────────────────────────────────────────────────

mod analytics;
mod assistant;
mod common;
mod config;
mod constants;
mod daemon;
mod dream;
mod handlers;
mod scorecard;
mod install;
mod ipc;
mod rules;

use std::process;

fn main() {
    // Capture backtraces on panic for post-mortem debugging
    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let msg = format!("PANIC: {}\n{}", info, bt);
        eprintln!("{}", msg);
        // Write to panic.log in project log dir if available
        let project_dir = common::project_dir();
        let log_path = project_dir.join("panic.log");
        let _ = std::fs::write(log_path, &msg);
    }));

    let args: Vec<String> = std::env::args().collect();
    let subcmd = args.get(1).map(|s| s.as_str()).unwrap_or("");

    match subcmd {
        // ── Management commands (no stdin) ──
        "version" => {
            println!("{} {}", constants::NAME, env!("CARGO_PKG_VERSION"));
        }
        "init" => install::wizard::run(),
        "uninstall" => install::uninstall::run(),
        "install" => {
            let target = args.get(2).map(|s| s.as_str()).unwrap_or("");
            match target {
                "claude-code" => {
                    let _ = install::ensure_dirs();
                    let _ = install::install_binary();
                    install_assistant::<assistant::claude_code::ClaudeCode>();
                    if !install::path::is_on_path()
                        && let Ok(msg) = install::path::add_to_path() {
                            eprintln!("{}", msg);
                        }
                    // Pre-start daemon so first session connects instantly
                    if !ipc::daemon_is_running() {
                        ipc::spawn_daemon();
                        eprintln!("Daemon started");
                    }
                }
                "gemini-cli" => {
                    let _ = install::ensure_dirs();
                    let _ = install::install_binary();
                    install_assistant::<assistant::gemini_cli::GeminiCli>();
                    if !install::path::is_on_path()
                        && let Ok(msg) = install::path::add_to_path() {
                            eprintln!("{}", msg);
                        }
                    if !ipc::daemon_is_running() {
                        ipc::spawn_daemon();
                        eprintln!("Daemon started");
                    }
                }
                _ => eprintln!("Usage: {} install <claude-code|gemini-cli>", constants::NAME),
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
                _ => eprintln!("Usage: {} config <list|get|set|path>", constants::NAME),
            }
        }

        // ── No-stdin subcommands ──
        "describe" => handlers::describe::run(),
        "debug-explain" => {
            let target = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if target.is_empty() {
                eprintln!("Usage: {} debug-explain <rule-id>", constants::NAME);
            } else {
                handlers::explain::explain_rule(target);
            }
        }
        "debug-explain-session" => handlers::explain::explain_session(),
        "project-dir" => println!("{}", common::project_dir().display()),
        "rules" => {
            let r = &*rules::RULES;
            let out = serde_json::json!({
                "safety": r.safety_pairs.len(),
                "substitutions": r.substitutions_pairs.len(),
                "advisories": r.advisories_pairs.len(),
                "auto_allow": r.auto_allow_patterns.len(),
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        }
        "debug-restrictions" => {
            let action = args.get(2).map(|s| s.as_str()).unwrap_or("");
            match action {
                "enable" => {
                    let id = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    if id.is_empty() {
                        eprintln!("Usage: {} restrictions enable <restriction-id>", constants::NAME);
                    } else {
                        toggle_restriction(id, false);
                    }
                }
                "disable" => {
                    let id = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    if id.is_empty() {
                        eprintln!("Usage: {} restrictions disable <restriction-id>", constants::NAME);
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
        "debug-export" => handlers::export_sessions::run(&args[2..]),
        "debug-stats" => print!("{}", handlers::learning::format_stats()),
        "debug-scorecard" => scorecard::run(),
        "debug-replay" => handlers::replay::run(&args[2..]),
        "debug-diff" if args.len() >= 4 => {
            handlers::replay::run(&["diff".to_string(), args[2].clone(), args[3].clone()]);
        }
        "debug-tui" => {
            if let Err(e) = handlers::tui::run() {
                eprintln!("TUI error: {}", e);
                process::exit(1);
            }
        }
        "mcp" => handlers::mcp_server::run(),
        "truncate-filter" => handlers::truncate_filter::run(),

        // ── Daemon subcommands ──
        "daemon" => {
            let mtime: u64 = args.get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(ipc::get_binary_mtime);
            daemon::run_server(mtime);
        }
        "debug-daemon-stop" => {
            if let Some(resp) = ipc::try_daemon("shutdown", "") {
                if resp.exit_code == 0 {
                    eprintln!("Daemon stopped");
                }
            } else {
                eprintln!("Daemon not running");
            }
        }
        "debug-daemon-status" => {
            if let Some(resp) = ipc::try_daemon("daemon-status", "") {
                println!("{}", resp.stdout);
            } else {
                eprintln!("Daemon not running");
                process::exit(1);
            }
        }

        // ── Hook subcommands (JSON stdin) ──
        _ if is_hook_subcmd(subcmd) => {
            // Set per-project CWD
            if let Ok(cwd) = std::env::current_dir() {
                common::set_project_cwd(&cwd.to_string_lossy());
            }

            // CI mode: safety rules only, skip daemon, skip analytics
            if common::is_ci() {
                let raw = read_stdin();
                dispatch_hook_ci(subcmd, &raw);
                return;
            }

            // IPC fast-path: try daemon first unless WARDEN_NO_DAEMON is set
            if std::env::var("WARDEN_NO_DAEMON").is_err() {
                let raw = read_stdin();
                if let Some(resp) = ipc::try_daemon(subcmd, &raw) {
                    if resp.exit_code == ipc::EXIT_RESTART {
                        // Daemon detected rebuild — fall through to direct execution
                        dispatch_hook(subcmd, &raw);
                    } else {
                        if !resp.stdout.is_empty() {
                            print!("{}", resp.stdout);
                        }
                        process::exit(resp.exit_code);
                    }
                } else {
                    // Daemon not running — direct execution
                    dispatch_hook(subcmd, &raw);
                }
            } else {
                let raw = read_stdin();
                dispatch_hook(subcmd, &raw);
            }
        }

        _ => {
            if !subcmd.is_empty() {
                eprintln!("Unknown command: {}", subcmd);
            } else {
                print_help();
            }
            process::exit(0);
        }
    }
}

fn read_stdin() -> String {
    use std::io::Read;
    // Use read() not read_to_string() — read() returns on first data without
    // waiting for EOF. read_to_string() blocks until the write end closes,
    // which deadlocks when called through the relay (Claude Code may not
    // close stdin before reading stdout).
    let mut buf = vec![0u8; 1_048_576];
    let n = std::io::stdin().read(&mut buf).unwrap_or(0);
    String::from_utf8_lossy(&buf[..n]).to_string()
}

fn install_assistant<A: assistant::Assistant + Default>() {
    let adapter = A::default();
    // On Windows, use relay (windowless) for hook commands to prevent CMD flicker.
    // On Unix, use warden directly (no console flash issue).
    let binary_name = if cfg!(windows) { "warden-relay.exe" } else { "warden" };
    let binary_path = install::bin_dir().join(binary_name);
    install_relay();

    let hooks_json = adapter.generate_hooks_config(&binary_path);
    let settings_path = adapter.settings_path();

    // Parse the generated hooks config
    let hooks_value: serde_json::Value = match serde_json::from_str(&hooks_json) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse generated hooks: {}", e);
            return;
        }
    };

    // Read existing settings or start fresh
    let mut settings: serde_json::Value = if settings_path.exists() {
        match std::fs::read_to_string(&settings_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or(serde_json::json!({})),
            Err(_) => serde_json::json!({}),
        }
    } else {
        // Ensure parent dir exists
        if let Some(parent) = settings_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        serde_json::json!({})
    };

    // Backup existing settings if they have hooks (another system was installed)
    if settings.get("hooks").is_some() {
        let backup_path = settings_path.with_extension("json.bak");
        let _ = std::fs::copy(&settings_path, &backup_path);
        eprintln!("Backed up existing settings to {}", backup_path.display());
    }

    // Merge: replace hooks section, keep everything else (permissions, etc.)
    if let Some(hooks) = hooks_value.get("hooks") {
        settings["hooks"] = hooks.clone();
    }

    // Write back
    match serde_json::to_string_pretty(&settings) {
        Ok(output) => {
            if std::fs::write(&settings_path, &output).is_ok() {
                eprintln!("Installed {} hooks into {}", adapter.name(), settings_path.display());
            } else {
                eprintln!("Failed to write {}", settings_path.display());
                eprintln!("Generated config (paste manually):");
                println!("{}", hooks_json);
            }
        }
        Err(e) => {
            eprintln!("Failed to serialize settings: {}", e);
        }
    }
}

/// Install the relay binary next to warden.exe
fn install_relay() {
    let source = std::env::current_exe().unwrap_or_default();
    let source_dir = source.parent().unwrap_or(std::path::Path::new("."));
    let relay_name = if cfg!(windows) { "warden-relay.exe" } else { "warden-relay" };
    let relay_src = source_dir.join(relay_name);

    let dest = install::bin_dir().join(relay_name);

    if relay_src.exists() && relay_src != dest {
        let _ = std::fs::copy(&relay_src, &dest);
    }
}

const HOOK_SUBCMDS: &[&str] = &[
    "pretool-bash", "pretool-read", "pretool-write", "pretool-redirect",
    "permission-approve", "posttool-session", "posttool-mcp",
    "session-start", "session-end", "precompact-memory", "postcompact",
    "stop-check", "userprompt-context", "subagent-context", "subagent-stop",
    "postfailure-guide", "task-completed",
];

fn is_hook_subcmd(s: &str) -> bool {
    HOOK_SUBCMDS.contains(&s)
}

/// CI mode dispatch: safety-critical handlers only, no analytics or session tracking.
fn dispatch_hook_ci(subcmd: &str, raw: &str) {
    match subcmd {
        "pretool-bash" => handlers::pretool_bash::run(raw),
        "pretool-write" => handlers::pretool_write::run(raw),
        "pretool-redirect" => handlers::pretool_redirect::run(raw),
        "postfailure-guide" => handlers::postfailure_guide::run(raw),
        _ => {} // Skip analytics, session tracking, context injection in CI
    }
}

fn dispatch_hook(subcmd: &str, raw: &str) {
    // Panic isolation: catch_unwind ensures a panicking handler never blocks the AI.
    // Fail open: on panic, log error and exit 0.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match subcmd {
            "pretool-bash" => handlers::pretool_bash::run(raw),
            "pretool-read" => handlers::pretool_read::run(raw),
            "pretool-write" => handlers::pretool_write::run(raw),
            "pretool-redirect" => handlers::pretool_redirect::run(raw),
            "permission-approve" => handlers::permission_approve::run(raw),
            "posttool-session" => handlers::posttool_session::run(raw),
            "posttool-mcp" => handlers::posttool_mcp::run(raw),
            "session-start" => handlers::session_start::run(raw),
            "session-end" => handlers::session_end::run(raw),
            "precompact-memory" => handlers::precompact_memory::run(raw),
            "postcompact" => handlers::postcompact::run(raw),
            "stop-check" => handlers::stop_check::run(raw),
            "userprompt-context" => handlers::userprompt_context::run(raw),
            "subagent-context" => handlers::subagent_context::run(raw),
            "subagent-stop" => handlers::subagent_stop::run(raw),
            "postfailure-guide" => handlers::postfailure_guide::run(raw),
            "task-completed" => handlers::task_completed::run(raw),
            _ => {}
        }
    }));
    if result.is_err() {
        eprintln!("{}: handler '{}' panicked — failing open", constants::NAME, subcmd);
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
    let disabled_str = disabled.iter()
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
            if line.trim() == "[restrictions]" { in_sect = true; }
            else if in_sect && line.trim().starts_with('[') { insert_at = i; break; }
            else if in_sect { insert_at = i + 1; }
        }
        lines.insert(insert_at, new_line);
    }

    let _ = std::fs::write(&config_path, lines.join("\n"));
}

fn set_config_value(path: &std::path::Path, key: &str, value: &str) {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    // Parse dotted key: "tools.justfile" → section "[tools]", key "justfile"
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
        Err(_) => { eprintln!("No config found"); return; }
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
        } else if in_section && trimmed.starts_with(&format!("{} =", field))
            && let Some(val) = trimmed.split('=').nth(1) {
                println!("{}", val.trim());
                return;
            }
    }
    eprintln!("{}: not set", key);
}

fn print_help() {
    eprintln!("{} v{} — AI Coding Session Guardian", constants::NAME, env!("CARGO_PKG_VERSION"));
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  {} <command>", constants::NAME);
    eprintln!();
    eprintln!("COMMANDS:");
    eprintln!("  init                    First-run setup wizard");
    eprintln!("  install <assistant>     Configure hooks for claude-code or gemini-cli");
    eprintln!("  uninstall               Remove hooks, binary, and config");
    eprintln!("  mcp                     Run as MCP server (stdio, JSON-RPC)");
    eprintln!("  version                 Print version");
    eprintln!();
    eprintln!("HOOK SUBCOMMANDS (called by AI assistants):");
    eprintln!("  pretool-bash            Safety, substitution, advisory pipeline");
    eprintln!("  pretool-read            Read governance (large files, dedup)");
    eprintln!("  pretool-write           Write governance (sensitive paths, zero-trace)");
    eprintln!("  session-start           Session initialization + context injection");
    eprintln!("  session-end             Session summary + analytics update");
    eprintln!("  userprompt-context      Per-turn telemetry + adaptation + advisories");
    eprintln!("  ...and more (see docs)");
}

