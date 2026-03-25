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
use super::{ensure_dirs, home_dir, install_binary, path, tools, write_default_config};
use crate::assistant::Assistant;
use crate::constants;

/// Run the full init wizard
pub fn run() {
    // Banner
    term::banner();

    term::print_colored(term::DIM, "  Warden is a runtime guardian for AI coding agents.\n");
    term::print_colored(term::DIM, "  It enforces tool policies, prevents drift, and keeps sessions efficient.\n");
    term::print_colored(term::DIM, "  Works with Claude Code, Gemini CLI, and more.\n");
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

    term::print_colored(term::DIM, "  Warden works best with modern CLI tools. It automatically redirects\n");
    term::print_colored(term::DIM, "  legacy commands (grep, find, curl) to faster alternatives.\n");

    let statuses = tools::detect_tools();
    let pm = tools::detect_package_manager();

    let installed_count = statuses.iter().filter(|s| s.installed).count();
    let total = statuses.len();

    eprintln!();
    term::info("Found:", &format!("{}/{} tools installed", installed_count, total));
    if let Some(pm_name) = pm {
        term::info("Package manager:", pm_name);
    }

    // Build multi-select with installed tools pre-checked and disabled
    let mut check_options: Vec<CheckOption> = statuses
        .iter()
        .map(|s| {
            let tool = tools::TOOLS.iter().find(|t| t.name == s.name).unwrap();
            if s.installed {
                let mut opt = CheckOption::installed(
                    tool.name,
                    &format!("\u{2713} installed  {}", tool.description),
                );
                opt.checked = true;
                opt
            } else {
                let rec = if tool.recommended { " (recommended)" } else { "" };
                CheckOption::new(
                    tool.name,
                    &format!("{}{}", tool.description, rec),
                    tool.recommended,
                )
            }
        })
        .collect();

    let has_missing = statuses.iter().any(|s| !s.installed);

    if has_missing {
        if pm.is_some() {
            let selected = term::multi_select("Select tools to install:", &mut check_options);

            // Install selected tools that aren't already installed
            let pm_name = pm.unwrap();
            for &idx in &selected {
                let tool_name = check_options[idx].label.as_str();
                if statuses[idx].installed {
                    continue; // Already installed, skip
                }
                if let Some(tool) = tools::TOOLS.iter().find(|t| t.name == tool_name)
                    && let Some(cmd) = tools::install_command(tool, pm_name)
                {
                    let sp = term::Spinner::start(&format!("Installing {}...", tool.name));
                    match tools::install_tool(cmd) {
                        Ok(()) => sp.finish_ok(&format!("{} installed", tool.name)),
                        Err(e) => sp.finish_fail(&format!("{}: {}", tool.name, e)),
                    }
                }
            }
        } else {
            eprintln!();
            term::status_warn("No package manager detected. Install tools manually:");
            for status in &statuses {
                if !status.installed {
                    if let Some(tool) = tools::TOOLS.iter().find(|t| t.name == status.name) {
                        if let Some(cmd) = tool.install_cargo {
                            term::hint(&format!("{}: {}", tool.name, cmd));
                        }
                    }
                }
            }
        }
    } else {
        eprintln!();
        term::status_ok("All tools installed!");
    }

    // Show "why" for installed tools if user wants to know
    if has_missing {
        eprintln!();
        term::print_colored(term::DIM, "  Tip: Each tool integrates with Warden's rule engine.\n");
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
            constants::NAME, constants::NAME
        ));
    } else {
        // Build selection options based on what's detected
        let mut options: Vec<SelectOption> = Vec::new();

        if has_claude && has_gemini {
            options.push(SelectOption::new("Both", "Configure Claude Code + Gemini CLI"));
            options.push(SelectOption::new("Claude Code", "Anthropic's AI coding assistant"));
            options.push(SelectOption::new("Gemini CLI", "Google's AI coding assistant"));
            options.push(SelectOption::new("Skip", "Configure later"));
        } else if has_claude {
            term::status_ok("Claude Code detected");
            options.push(SelectOption::new("Claude Code", "Configure hooks for Claude Code"));
            options.push(SelectOption::new("Skip", "Configure later"));
        } else {
            term::status_ok("Gemini CLI detected");
            options.push(SelectOption::new("Gemini CLI", "Configure hooks for Gemini CLI"));
            options.push(SelectOption::new("Skip", "Configure later"));
        }

        if let Some(choice) = term::select("Which assistant should Warden guard?", &options) {
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

    // Migration from hookctl
    migrate_from_hookctl();

    // Start daemon
    let sp = term::Spinner::start("Starting daemon...");
    if !crate::ipc::daemon_is_running() {
        crate::ipc::spawn_daemon();
        sp.finish_ok("Daemon started");
    } else {
        sp.finish_ok("Daemon already running");
    }

    // ── Done ─────────────────────────────────────────────────────────────────

    eprintln!();
    term::print_bold(term::BRAND, "  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n");
    eprintln!();
    term::print_bold(term::SUCCESS, "  Warden is ready.\n");
    eprintln!();
    term::print_colored(term::TEXT, "  Quick reference:\n");
    term::info("warden version     ", "Verify installation");
    term::info("warden describe    ", "Show active rules & restrictions");
    term::info("warden config list ", "View current configuration");
    eprintln!();
    term::print_colored(term::DIM, "  Warden runs automatically via hooks — no manual intervention needed.\n");
    term::print_colored(term::DIM, "  Start a coding session and Warden will guard it silently.\n");
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
    } else {
        if std::fs::write(&settings_path, &hooks_json).is_ok() {
            sp.finish_ok("Claude Code hooks created");
        } else {
            sp.finish_fail("Could not create settings.json");
        }
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

/// Migrate state from ~/.hookctl/ → ~/.warden/
fn migrate_from_hookctl() {
    let old_dir = dirs_home().join(constants::LEGACY_DIR);
    if !old_dir.exists() {
        return;
    }

    eprintln!();
    term::status_work(&format!("Found ~/{} (legacy)", constants::LEGACY_DIR));

    if !term::confirm("Migrate data to new location?", true) {
        term::status_skip("Migration skipped");
        return;
    }

    let new_dir = home_dir();

    // Migrate projects/
    let old_projects = old_dir.join("projects");
    let new_projects = new_dir.join("projects");
    if old_projects.exists() && !new_projects.exists() {
        let sp = term::Spinner::start("Migrating projects...");
        if copy_dir_recursive(&old_projects, &new_projects).is_ok() {
            sp.finish_ok("Projects migrated");
        } else {
            sp.finish_fail("Could not migrate projects");
        }
    }

    // Migrate rules.toml → rules/personal.toml
    let old_rules = old_dir.join("rules.toml");
    let new_rules = new_dir.join("rules").join(constants::PERSONAL_RULES);
    if old_rules.exists() && !new_rules.exists() {
        let sp = term::Spinner::start("Migrating rules...");
        if std::fs::copy(&old_rules, &new_rules).is_ok() {
            sp.finish_ok(&format!("Rules migrated to rules/{}", constants::PERSONAL_RULES));
        } else {
            sp.finish_fail("Could not migrate rules");
        }
    }

    // Migrate logs/
    let old_logs = old_dir.join("logs");
    let new_logs = new_dir.join("logs");
    if old_logs.exists() && !new_logs.exists() {
        let _ = copy_dir_recursive(&old_logs, &new_logs);
    }

    term::status_ok("Migration complete");
    term::hint(&format!(
        "Old ~/{} can be removed manually",
        constants::LEGACY_DIR
    ));
}

/// Get user home directory
fn dirs_home() -> std::path::PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}
