// ─── install::wizard — first-run interactive setup ───────────────────────────
//
// `warden init` flow:
//   1. Create ~/.warden/ directory structure
//   2. Install binary to ~/.warden/bin/
//   3. Add ~/.warden/bin/ to PATH
//   4. Detect available CLI tools, offer to install missing ones
//   5. Detect AI assistant (Claude Code / Gemini CLI)
//   6. Configure hooks for detected assistant(s)
//   7. Write default config.toml
//   8. Migrate from ~/.hookctl/ if exists
// ──────────────────────────────────────────────────────────────────────────────

use crate::assistant::Assistant;
use crate::constants;
use super::{path, tools, ensure_dirs, install_binary, write_default_config, home_dir};
use std::io::{self, Write, BufRead};

/// Run the full init wizard
pub fn run() {
    eprintln!("=== {} init ===", constants::NAME);
    eprintln!();

    // 1. Create directory structure
    eprint!("Creating {}/ structure... ", constants::DIR);
    match ensure_dirs() {
        Ok(()) => eprintln!("ok"),
        Err(e) => {
            eprintln!("FAILED: {}", e);
            return;
        }
    }

    // 2. Install binary
    eprint!("Installing binary to {}/bin/... ", constants::DIR);
    match install_binary() {
        Ok(()) => eprintln!("ok"),
        Err(e) => eprintln!("FAILED: {} (non-fatal)", e),
    }

    // 3. PATH registration
    if !path::is_on_path() {
        eprint!("Adding {}/bin/ to PATH... ", constants::DIR);
        match path::add_to_path() {
            Ok(msg) => eprintln!("{}", msg),
            Err(e) => eprintln!("FAILED: {} (add manually)", e),
        }
    } else {
        eprintln!("PATH: already configured");
    }

    // 4. Detect tools
    eprintln!();
    eprintln!("Detecting CLI tools:");
    let statuses = tools::detect_tools();
    let pm = tools::detect_package_manager();
    let mut missing: Vec<&tools::ToolInfo> = Vec::new();

    for status in &statuses {
        let icon = if status.installed { "+" } else { "-" };
        eprintln!("  [{}] {} ({})", icon, status.name, status.binary);
        if !status.installed
            && let Some(tool) = tools::TOOLS.iter().find(|t| t.name == status.name) {
                missing.push(tool);
            }
    }

    // 5. Offer to install missing tools
    if !missing.is_empty() {
        if let Some(pm_name) = pm {
            eprintln!();
            eprintln!("Package manager detected: {}", pm_name);
            eprintln!("Install missing tools? (y/n/select)");

            let answer = read_line().to_lowercase();
            match answer.trim() {
                "y" | "yes" => {
                    for tool in &missing {
                        if let Some(cmd) = tools::install_command(tool, pm_name) {
                            eprint!("  Installing {}... ", tool.name);
                            match tools::install_tool(cmd) {
                                Ok(()) => eprintln!("ok"),
                                Err(e) => eprintln!("FAILED: {}", e),
                            }
                        }
                    }
                }
                "s" | "select" => {
                    for tool in &missing {
                        if let Some(cmd) = tools::install_command(tool, pm_name) {
                            eprint!("  Install {} ({})? [y/n] ", tool.name, tool.description);
                            let a = read_line().to_lowercase();
                            if a.trim() == "y" || a.trim() == "yes" {
                                eprint!("    Installing... ");
                                match tools::install_tool(cmd) {
                                    Ok(()) => eprintln!("ok"),
                                    Err(e) => eprintln!("FAILED: {}", e),
                                }
                            }
                        }
                    }
                }
                _ => eprintln!("  Skipping tool installation"),
            }
        } else {
            eprintln!();
            eprintln!("No package manager detected. Install tools manually:");
            for tool in &missing {
                if let Some(cmd) = tool.install_cargo {
                    eprintln!("  {}: {}", tool.name, cmd);
                }
            }
        }
    }

    // 6. Detect and configure AI assistant
    eprintln!();
    detect_and_configure_assistants();

    // 7. Write default config
    eprint!("Writing default config... ");
    match write_default_config() {
        Ok(()) => eprintln!("ok"),
        Err(e) => eprintln!("FAILED: {}", e),
    }

    // 8. Migration from hookctl
    migrate_from_hookctl();

    // Done
    eprintln!();
    eprintln!("=== {} ready ===", constants::NAME);
    eprintln!("Run `{} version` to verify.", constants::NAME);
}

/// Detect which AI assistants are installed and configure hooks
fn detect_and_configure_assistants() {
    let claude_dir = dirs_home().join(".claude");
    let gemini_dir = dirs_home().join(".gemini");

    let has_claude = claude_dir.exists();
    let has_gemini = gemini_dir.exists();

    if has_claude {
        eprint!("Claude Code detected. Configure hooks? [y/n] ");
        if read_line().trim().to_lowercase().starts_with('y') {
            configure_claude_code();
        }
    }

    if has_gemini {
        eprint!("Gemini CLI detected. Configure hooks? [y/n] ");
        if read_line().trim().to_lowercase().starts_with('y') {
            configure_gemini_cli();
        }
    }

    if !has_claude && !has_gemini {
        eprintln!("No AI assistant detected. Run `{} install claude-code` or `{} install gemini-cli` later.",
            constants::NAME, constants::NAME);
    }
}

/// Configure Claude Code hooks in ~/.claude/settings.json
fn configure_claude_code() {
    let adapter = crate::assistant::claude_code::ClaudeCode;
    let binary_name = if cfg!(windows) { "warden-relay.exe" } else { "warden" };
    let binary = super::bin_dir().join(binary_name);
    let hooks_json = adapter.generate_hooks_config(&binary);

    let settings_path = dirs_home().join(".claude").join("settings.json");

    if settings_path.exists() {
        // Merge hooks into existing settings
        if let Ok(content) = std::fs::read_to_string(&settings_path)
            && let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content)
                && let Ok(hooks) = serde_json::from_str::<serde_json::Value>(&hooks_json)
                    && let Some(new_hooks) = hooks.get("hooks") {
                        settings["hooks"] = new_hooks.clone();
                        if let Ok(merged) = serde_json::to_string_pretty(&settings)
                            && std::fs::write(&settings_path, &merged).is_ok() {
                                eprintln!("  Hooks configured in {}", settings_path.display());
                                return;
                            }
                    }
        eprintln!("  Could not merge hooks. Add manually from: {} install claude-code", constants::NAME);
    } else {
        // Create new settings with hooks
        if std::fs::write(&settings_path, &hooks_json).is_ok() {
            eprintln!("  Created {}", settings_path.display());
        }
    }
}

/// Configure Gemini CLI hooks
fn configure_gemini_cli() {
    let adapter = crate::assistant::gemini_cli::GeminiCli;
    let binary_name = if cfg!(windows) { "warden-relay.exe" } else { "warden" };
    let binary = super::bin_dir().join(binary_name);
    let hooks_json = adapter.generate_hooks_config(&binary);

    let settings_path = dirs_home().join(".gemini").join("settings.json");
    let Some(settings_dir) = settings_path.parent() else { return; };
    let _ = std::fs::create_dir_all(settings_dir);

    if settings_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&settings_path)
            && let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content)
                && let Ok(hooks) = serde_json::from_str::<serde_json::Value>(&hooks_json)
                    && let Some(new_hooks) = hooks.get("hooks") {
                        settings["hooks"] = new_hooks.clone();
                        if let Ok(merged) = serde_json::to_string_pretty(&settings)
                            && std::fs::write(&settings_path, &merged).is_ok() {
                                eprintln!("  Hooks configured in {}", settings_path.display());
                            }
                    }
    } else if std::fs::write(&settings_path, &hooks_json).is_ok() {
        eprintln!("  Created {}", settings_path.display());
    }
}

/// Migrate state from ~/.hookctl/ → ~/.warden/
fn migrate_from_hookctl() {
    let old_dir = dirs_home().join(constants::LEGACY_DIR);
    if !old_dir.exists() {
        return;
    }

    eprintln!();
    eprint!("Found {}/ — migrate to {}/? [y/n] ", constants::LEGACY_DIR, constants::DIR);
    if !read_line().trim().to_lowercase().starts_with('y') {
        return;
    }

    let new_dir = home_dir();

    // Migrate projects/
    let old_projects = old_dir.join("projects");
    let new_projects = new_dir.join("projects");
    if old_projects.exists() && !new_projects.exists() {
        eprint!("  Migrating projects/... ");
        if copy_dir_recursive(&old_projects, &new_projects).is_ok() {
            eprintln!("ok");
        } else {
            eprintln!("FAILED");
        }
    }

    // Migrate rules.toml → rules/personal.toml
    let old_rules = old_dir.join("rules.toml");
    let new_rules = new_dir.join("rules").join(constants::PERSONAL_RULES);
    if old_rules.exists() && !new_rules.exists() {
        eprint!("  Migrating rules.toml... ");
        if std::fs::copy(&old_rules, &new_rules).is_ok() {
            eprintln!("ok → rules/{}", constants::PERSONAL_RULES);
        } else {
            eprintln!("FAILED");
        }
    }

    // Migrate logs/
    let old_logs = old_dir.join("logs");
    let new_logs = new_dir.join("logs");
    if old_logs.exists() && !new_logs.exists() {
        let _ = copy_dir_recursive(&old_logs, &new_logs);
    }

    eprintln!("  Migration complete. Old {} can be removed manually.", constants::LEGACY_DIR);
}

/// Read a line from stdin (for interactive prompts)
fn read_line() -> String {
    let _ = io::stdout().flush();
    let stdin = io::stdin();
    let mut line = String::new();
    let _ = stdin.lock().read_line(&mut line);
    line
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
