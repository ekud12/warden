// ─── install::wizard — interactive first-run setup ───────────────────────────
//
// `warden init` flow:
//   1. Banner + welcome
//   2. Create ~/.warden/ directory structure
//   3. Install binary to ~/.warden/bin/
//   4. Add ~/.warden/bin/ to PATH
//   5. Detect available CLI tools, offer to install missing ones
//   6. Detect AI assistant (Claude Code / Gemini CLI)
//   7. Configure hooks for detected assistant(s)
//   8. Write default config.toml
//   9. Migrate from ~/.hookctl/ if exists
// ──────────────────────────────────────────────────────────────────────────────

use super::term::{self, CheckOption, SelectOption};
use super::{ensure_dirs, install_binary, path, tools, write_default_config};
use crate::assistant::Assistant;
use crate::constants;

/// Run the full init wizard
pub fn run() {
    // Banner
    term::banner();

    term::print_colored(
        term::DIM,
        "  Warden is a runtime guardian for AI coding agents.\n",
    );
    term::print_colored(
        term::DIM,
        "  It enforces tool policies, prevents drift, and keeps sessions efficient.\n",
    );
    term::print_colored(
        term::DIM,
        "  Works with Claude Code, Gemini CLI, and more.\n",
    );
    eprintln!();

    // ── Step 1: Setup directories + binary ───────────────────────────────────

    term::section("Setup");

    let sp = term::Spinner::start("Creating directory structure...");
    match ensure_dirs() {
        Ok(()) => sp.finish_ok(&format!("Created ~/{}/", constants::DIR)),
        Err(e) => {
            sp.finish_fail(&format!("Failed to create directories: {}", e));
            return;
        }
    }

    let sp = term::Spinner::start("Installing binary...");
    match install_binary() {
        Ok(()) => sp.finish_ok(&format!("Binary installed to ~/{}/bin/", constants::DIR)),
        Err(e) => sp.finish_warn(&format!("Binary install: {} (non-fatal)", e)),
    }

    // PATH
    if !path::is_on_path() {
        let sp = term::Spinner::start("Registering PATH...");
        match path::add_to_path() {
            Ok(msg) => sp.finish_ok(&msg),
            Err(e) => sp.finish_warn(&format!("PATH: {} (add manually)", e)),
        }
    } else {
        term::status_ok("PATH already configured");
    }

    // ── Step 2: Detect and install CLI tools ─────────────────────────────────

    term::section("CLI Tools");

    term::print_colored(
        term::DIM,
        "  Warden works best with modern CLI tools. It automatically redirects\n",
    );
    term::print_colored(
        term::DIM,
        "  legacy commands (grep, find, curl) to faster alternatives.\n",
    );

    let statuses = tools::detect_tools();
    let pm = tools::detect_package_manager();

    let installed_count = statuses.iter().filter(|s| s.installed).count();
    let total = statuses.len();

    eprintln!();
    term::info(
        "Found:",
        &format!("{}/{} tools installed", installed_count, total),
    );
    if let Some(pm_name) = pm {
        term::info("Package manager:", pm_name);
    }

    // Show installed tools
    for s in &statuses {
        if s.installed {
            term::status_ok(&format!("{} ({})", s.name, s.binary));
        }
    }

    // Build multi-select with ONLY missing tools + Skip
    let missing: Vec<&tools::ToolInfo> = statuses
        .iter()
        .filter(|s| !s.installed)
        .filter_map(|s| tools::TOOLS.iter().find(|t| t.name == s.name))
        .collect();

    if !missing.is_empty() {
        if let Some(pm_name) = pm {
            let mut check_options: Vec<CheckOption> = missing
                .iter()
                .map(|tool| {
                    let rec = if tool.recommended {
                        " (recommended)"
                    } else {
                        ""
                    };
                    CheckOption::new(
                        tool.name,
                        &format!("{}{}", tool.description, rec),
                        tool.recommended,
                    )
                })
                .collect();
            // Add skip option (always unchecked)
            check_options.push(CheckOption::new("Skip", "Don't install any tools", false));

            match term::multi_select("Install missing tools:", &mut check_options) {
                None => {
                    eprintln!();
                    term::status_skip("Setup cancelled");
                    term::hint(&format!(
                        "Run `{} init` again to complete setup.",
                        constants::NAME
                    ));
                    eprintln!();
                    return;
                }
                Some(selected) => {
                    for &idx in &selected {
                        let tool_name = check_options[idx].label.as_str();
                        if tool_name == "Skip" {
                            continue;
                        }
                        if let Some(tool) = missing.iter().find(|t| t.name == tool_name)
                            && let Some(cmd) = tools::install_command(tool, pm_name)
                        {
                            let sp = term::Spinner::start(&format!("Installing {}...", tool.name));
                            match tools::install_tool(cmd) {
                                Ok(()) => sp.finish_ok(&format!("{} installed", tool.name)),
                                Err(e) => sp.finish_fail(&format!("{}: {}", tool.name, e)),
                            }
                        }
                    }
                }
            }
        } else {
            eprintln!();
            term::status_warn("No package manager detected. Install tools manually:");
            for status in &statuses {
                if !status.installed
                    && let Some(tool) = tools::TOOLS.iter().find(|t| t.name == status.name)
                        && let Some(cmd) = tool.install_cargo {
                            term::hint(&format!("{}: {}", tool.name, cmd));
                        }
            }
        }
    } else {
        eprintln!();
        term::status_ok("All tools installed!");
    }

    // Show "why" for installed tools if user wants to know
    if !missing.is_empty() {
        eprintln!();
        term::print_colored(
            term::DIM,
            "  Tip: Each tool integrates with Warden's rule engine.\n",
        );
        term::print_colored(term::DIM, "  Run `warden describe <tool>` to learn more.\n");
    }

    // ── Step 3: AI Assistant configuration ───────────────────────────────────

    term::section("AI Assistant");

    let claude_dir = dirs_home().join(".claude");
    let gemini_dir = dirs_home().join(".gemini");

    let has_claude = claude_dir.exists();
    let has_gemini = gemini_dir.exists();

    if !has_claude && !has_gemini {
        term::status_warn("No AI assistant detected.");
        term::hint(&format!(
            "Run `{} install claude-code` or `{} install gemini-cli` after installing one.",
            constants::NAME,
            constants::NAME
        ));
    } else {
        // Build selection options based on what's detected
        let mut options: Vec<SelectOption> = Vec::new();

        if has_claude && has_gemini {
            options.push(SelectOption::new(
                "Both",
                "Configure Claude Code + Gemini CLI",
            ));
            options.push(SelectOption::new(
                "Claude Code",
                "Anthropic's AI coding assistant",
            ));
            options.push(SelectOption::new(
                "Gemini CLI",
                "Google's AI coding assistant",
            ));
            options.push(SelectOption::new("Skip", "Configure later"));
        } else if has_claude {
            term::status_ok("Claude Code detected");
            options.push(SelectOption::new(
                "Claude Code",
                "Configure hooks for Claude Code",
            ));
            options.push(SelectOption::new("Skip", "Configure later"));
        } else {
            term::status_ok("Gemini CLI detected");
            options.push(SelectOption::new(
                "Gemini CLI",
                "Configure hooks for Gemini CLI",
            ));
            options.push(SelectOption::new("Skip", "Configure later"));
        }

        match term::select("Which assistant should Warden guard?", &options) {
            None => {
                // Esc pressed — abort wizard
                eprintln!();
                term::status_skip("Setup cancelled");
                term::hint(&format!(
                    "Run `{} init` again to complete setup.",
                    constants::NAME
                ));
                eprintln!();
                return;
            }
            Some(choice) => {
                let label = &options[choice].label;
                match label.as_str() {
                    "Both" => {
                        configure_claude_code();
                        configure_gemini_cli();
                    }
                    "Claude Code" => configure_claude_code(),
                    "Gemini CLI" => configure_gemini_cli(),
                    _ => term::status_skip("Skipped assistant configuration"),
                }
            }
        }
    }

    // ── Step 4: Config + migration ───────────────────────────────────────────

    term::section("Configuration");

    let sp = term::Spinner::start("Writing default config...");
    match write_default_config() {
        Ok(()) => sp.finish_ok(&format!(
            "Config at ~/{}/{}",
            constants::DIR,
            constants::CONFIG_FILE
        )),
        Err(e) => sp.finish_fail(&format!("Config: {}", e)),
    }

    // Start daemon
    let sp = term::Spinner::start("Starting daemon...");
    if !crate::runtime::ipc::daemon_is_running() {
        crate::runtime::ipc::spawn_daemon();
        sp.finish_ok("Daemon started");
    } else {
        sp.finish_ok("Daemon already running");
    }

    // ── Done ─────────────────────────────────────────────────────────────────

    eprintln!();
    term::print_bold(
        term::BRAND,
        "  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n",
    );
    eprintln!();
    term::print_bold(term::SUCCESS, "  Warden is ready.\n");
    eprintln!();
    term::print_colored(term::TEXT, "  Quick reference:\n");
    term::info("warden version     ", "Verify installation");
    term::info("warden describe    ", "Show active rules & restrictions");
    term::info("warden config list ", "View current configuration");
    eprintln!();
    term::print_colored(
        term::DIM,
        "  Warden runs automatically via hooks — no manual intervention needed.\n",
    );
    term::print_colored(
        term::DIM,
        "  Start a coding session and Warden will guard it silently.\n",
    );
    eprintln!();
}

/// Configure Claude Code hooks in ~/.claude/settings.json
fn configure_claude_code() {
    let sp = term::Spinner::start("Configuring Claude Code hooks...");
    let adapter = crate::assistant::claude_code::ClaudeCode;
    let binary_name = if cfg!(windows) {
        "warden-relay.exe"
    } else {
        "warden"
    };
    let binary = super::bin_dir().join(binary_name);
    let hooks_json = adapter.generate_hooks_config(&binary);

    let settings_path = dirs_home().join(".claude").join("settings.json");

    if settings_path.exists() {
        // Backup + merge
        let bak = settings_path.with_extension("json.bak");
        let _ = std::fs::copy(&settings_path, &bak);

        if let Ok(content) = std::fs::read_to_string(&settings_path)
            && let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content)
            && let Ok(hooks) = serde_json::from_str::<serde_json::Value>(&hooks_json)
            && let Some(new_hooks) = hooks.get("hooks")
        {
            settings["hooks"] = new_hooks.clone();
            if let Ok(merged) = serde_json::to_string_pretty(&settings)
                && std::fs::write(&settings_path, &merged).is_ok()
            {
                sp.finish_ok("Claude Code hooks installed");
                return;
            }
        }
        sp.finish_fail("Could not merge hooks into settings.json");
    } else if std::fs::write(&settings_path, &hooks_json).is_ok() {
        sp.finish_ok("Claude Code hooks created");
    } else {
        sp.finish_fail("Could not create settings.json");
    }
}

/// Configure Gemini CLI hooks
fn configure_gemini_cli() {
    let sp = term::Spinner::start("Configuring Gemini CLI hooks...");
    let adapter = crate::assistant::gemini_cli::GeminiCli;
    let binary_name = if cfg!(windows) {
        "warden-relay.exe"
    } else {
        "warden"
    };
    let binary = super::bin_dir().join(binary_name);
    let hooks_json = adapter.generate_hooks_config(&binary);

    let settings_path = dirs_home().join(".gemini").join("settings.json");
    let Some(settings_dir) = settings_path.parent() else {
        sp.finish_fail("Could not determine Gemini settings path");
        return;
    };
    let _ = std::fs::create_dir_all(settings_dir);

    if settings_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&settings_path)
            && let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content)
            && let Ok(hooks) = serde_json::from_str::<serde_json::Value>(&hooks_json)
            && let Some(new_hooks) = hooks.get("hooks")
        {
            settings["hooks"] = new_hooks.clone();
            if let Ok(merged) = serde_json::to_string_pretty(&settings)
                && std::fs::write(&settings_path, &merged).is_ok()
            {
                sp.finish_ok("Gemini CLI hooks installed");
                return;
            }
        }
        sp.finish_fail("Could not merge hooks into Gemini settings");
    } else if std::fs::write(&settings_path, &hooks_json).is_ok() {
        sp.finish_ok("Gemini CLI hooks created");
    } else {
        sp.finish_fail("Could not create Gemini settings");
    }
}

/// Get user home directory
fn dirs_home() -> std::path::PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
}
