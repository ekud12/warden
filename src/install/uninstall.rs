// ─── install::uninstall — clean removal of Warden ─────────────────────────────
//
// `warden uninstall` flow:
//   1. Stop the daemon if running
//   2. Remove hooks from assistant config (Claude Code / Gemini CLI)
//   3. Remove binary from PATH (best-effort)
//   4. Remove ~/.warden/ directory (with confirmation)
// ──────────────────────────────────────────────────────────────────────────────

use super::term;
use crate::constants;
use std::fs;
use std::path::PathBuf;

/// Run the uninstall wizard
pub fn run() {
    eprintln!();
    term::print_bold(term::ERROR, &format!("  {} uninstall\n", constants::NAME));
    eprintln!();

    // 1. Stop daemon
    let sp = term::Spinner::start("Stopping daemon...");
    if let Some(resp) = crate::ipc::try_daemon("shutdown", "") {
        if resp.exit_code == 0 {
            sp.finish_ok("Daemon stopped");
        } else {
            sp.finish_warn(&format!("Daemon exit code {}", resp.exit_code));
        }
    } else {
        sp.finish_ok("Daemon not running");
    }

    // 2. Remove hooks from assistant configs
    remove_claude_code_hooks();
    remove_gemini_cli_hooks();

    // 3. Remove binary from PATH
    remove_from_path();

    // 4. Remove home directory
    let home = super::home_dir();
    if home.exists() {
        eprintln!();
        term::status_warn(&format!("This will delete {}", home.display()));
        term::hint("Config, rules, and cached state will be removed.");

        if term::confirm("Remove Warden data directory?", false) {
            let sp = term::Spinner::start("Removing directory...");
            match fs::remove_dir_all(&home) {
                Ok(()) => sp.finish_ok(&format!("Removed {}", home.display())),
                Err(e) => sp.finish_fail(&format!("Failed: {} (remove manually)", e)),
            }
        } else {
            term::status_skip("Directory preserved");
        }
    }

    eprintln!();
    term::print_bold(term::DIM, &format!("  {} has been uninstalled.\n", constants::NAME));
    term::hint(&format!(
        "The running binary at {} can be deleted manually.",
        std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    ));
    eprintln!();
}

/// Remove Warden hooks from Claude Code settings.json
fn remove_claude_code_hooks() {
    let home = dirs_home();
    let settings_path = home.join(".claude").join("settings.json");

    let sp = term::Spinner::start("Removing Claude Code hooks...");
    if !settings_path.exists() {
        sp.finish_ok("Claude Code: not found");
        return;
    }

    if let Ok(content) = fs::read_to_string(&settings_path)
        && let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content)
    {
        if let Some(hooks) = settings.get_mut("hooks")
            && let Some(hooks_obj) = hooks.as_object_mut()
        {
            let mut cleaned = false;
            for (_event, hook_list) in hooks_obj.iter_mut() {
                if let Some(arr) = hook_list.as_array_mut() {
                    let before = arr.len();
                    arr.retain(|hook| {
                        let cmd = hook
                            .get("hooks")
                            .and_then(|h| h.as_array())
                            .and_then(|a| a.first())
                            .and_then(|h| h.get("command"))
                            .and_then(|c| c.as_str())
                            .unwrap_or("");
                        !cmd.contains("warden")
                    });
                    if arr.len() < before {
                        cleaned = true;
                    }
                }
            }
            hooks_obj.retain(|_, v| v.as_array().map(|a| !a.is_empty()).unwrap_or(true));

            if cleaned {
                if let Ok(json) = serde_json::to_string_pretty(&settings) {
                    let _ = fs::write(&settings_path, json);
                }
                sp.finish_ok("Claude Code hooks removed");
            } else {
                sp.finish_ok("Claude Code: no warden hooks found");
            }
        } else {
            sp.finish_ok("Claude Code: no hooks section");
        }
    } else {
        sp.finish_warn("Claude Code: could not parse settings");
    }
}

/// Remove Warden hooks from Gemini CLI settings.json
fn remove_gemini_cli_hooks() {
    let home = dirs_home();
    let settings_path = home.join(".gemini").join("settings.json");

    let sp = term::Spinner::start("Removing Gemini CLI hooks...");
    if !settings_path.exists() {
        sp.finish_ok("Gemini CLI: not found");
        return;
    }

    if let Ok(content) = fs::read_to_string(&settings_path)
        && let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content)
    {
        if let Some(hooks) = settings.get_mut("hooks")
            && let Some(hooks_obj) = hooks.as_object_mut()
        {
            let mut cleaned = false;
            for (_event, hook_list) in hooks_obj.iter_mut() {
                if let Some(arr) = hook_list.as_array_mut() {
                    let before = arr.len();
                    arr.retain(|hook| {
                        let cmd = hook
                            .get("hooks")
                            .and_then(|h| h.as_array())
                            .and_then(|a| a.first())
                            .and_then(|h| h.get("command"))
                            .and_then(|c| c.as_str())
                            .unwrap_or("");
                        !cmd.contains("warden")
                    });
                    if arr.len() < before {
                        cleaned = true;
                    }
                }
            }
            hooks_obj.retain(|_, v| v.as_array().map(|a| !a.is_empty()).unwrap_or(true));

            if cleaned {
                if let Ok(json) = serde_json::to_string_pretty(&settings) {
                    let _ = fs::write(&settings_path, json);
                }
                sp.finish_ok("Gemini CLI hooks removed");
            } else {
                sp.finish_ok("Gemini CLI: no warden hooks found");
            }
        } else {
            sp.finish_ok("Gemini CLI: no hooks section");
        }
    } else {
        sp.finish_warn("Gemini CLI: could not parse settings");
    }
}

/// Best-effort PATH removal
fn remove_from_path() {
    let sp = term::Spinner::start("Removing from PATH...");

    #[cfg(windows)]
    {
        if let Ok(current) = std::env::var("PATH") {
            let bin_dir = super::bin_dir();
            let bin_str = bin_dir.to_string_lossy();
            if current.contains(bin_str.as_ref()) {
                sp.finish_warn("Found in PATH (remove via System > Environment Variables)");
            } else {
                sp.finish_ok("Not in PATH");
            }
        } else {
            sp.finish_ok("Not in PATH");
        }
    }

    #[cfg(not(windows))]
    {
        let bin_dir = super::bin_dir();
        let bin_str = bin_dir.to_string_lossy();
        let home = dirs_home();

        let configs = [
            home.join(".bashrc"),
            home.join(".zshrc"),
            home.join(".profile"),
            home.join(".bash_profile"),
        ];

        let mut found = false;
        for config in &configs {
            if let Ok(content) = fs::read_to_string(config)
                && content.contains(bin_str.as_ref())
            {
                let cleaned: Vec<&str> = content
                    .lines()
                    .filter(|line| !line.contains(bin_str.as_ref()))
                    .collect();
                let _ = fs::write(config, cleaned.join("\n"));
                found = true;
            }
        }
        if found {
            sp.finish_ok("Removed from shell configs");
        } else {
            sp.finish_ok("Not in PATH");
        }
    }
}

fn dirs_home() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
}
