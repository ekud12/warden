// ─── install::uninstall — clean removal of Warden ─────────────────────────────
//
// `warden uninstall` flow:
//   1. Stop the daemon if running
//   2. Remove hooks from assistant config (Claude Code / Gemini CLI)
//   3. Remove binary from PATH (best-effort)
//   4. Remove ~/.warden/ directory (with confirmation)
//
// Does NOT remove:
//   - Session data in project directories (user's data)
//   - The running binary itself (user ran it to uninstall)
// ──────────────────────────────────────────────────────────────────────────────

use crate::constants;
use std::fs;
use std::path::PathBuf;

/// Run the uninstall wizard
pub fn run() {
    eprintln!("=== {} uninstall ===", constants::NAME);
    eprintln!();

    // 1. Stop daemon
    eprint!("Stopping daemon... ");
    if let Some(resp) = crate::ipc::try_daemon("shutdown", "") {
        if resp.exit_code == 0 {
            eprintln!("stopped");
        } else {
            eprintln!("exit code {}", resp.exit_code);
        }
    } else {
        eprintln!("not running");
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
        eprintln!(
            "Remove {}? This deletes config, rules, and cached state.",
            home.display()
        );
        eprintln!("  Type 'yes' to confirm:");

        let answer = read_line();
        if answer.trim() == "yes" {
            match fs::remove_dir_all(&home) {
                Ok(()) => eprintln!("  Removed {}", home.display()),
                Err(e) => eprintln!("  Failed to remove: {} (remove manually)", e),
            }
        } else {
            eprintln!("  Skipped (directory preserved)");
        }
    }

    eprintln!();
    eprintln!("=== {} uninstalled ===", constants::NAME);
    eprintln!(
        "The running binary at {} can be deleted manually.",
        std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
}

/// Remove Warden hooks from Claude Code settings.json
fn remove_claude_code_hooks() {
    let home = dirs_home();
    let settings_path = home.join(".claude").join("settings.json");

    eprint!("Removing Claude Code hooks... ");
    if !settings_path.exists() {
        eprintln!("not found");
        return;
    }

    if let Ok(content) = fs::read_to_string(&settings_path)
        && let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content)
    {
        // Remove hooks that reference warden
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
            // Remove empty hook events
            hooks_obj.retain(|_, v| v.as_array().map(|a| !a.is_empty()).unwrap_or(true));

            if cleaned {
                if let Ok(json) = serde_json::to_string_pretty(&settings) {
                    let _ = fs::write(&settings_path, json);
                }
                eprintln!("cleaned");
            } else {
                eprintln!("no warden hooks found");
            }
        } else {
            eprintln!("no hooks section");
        }
    } else {
        eprintln!("could not parse settings");
    }
}

/// Remove Warden hooks from Gemini CLI settings.json
fn remove_gemini_cli_hooks() {
    let home = dirs_home();
    let settings_path = home.join(".gemini").join("settings.json");

    eprint!("Removing Gemini CLI hooks... ");
    if !settings_path.exists() {
        eprintln!("not found");
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
                eprintln!("cleaned");
            } else {
                eprintln!("no warden hooks found");
            }
        } else {
            eprintln!("no hooks section");
        }
    } else {
        eprintln!("could not parse settings");
    }
}

/// Best-effort PATH removal
fn remove_from_path() {
    eprint!("Removing from PATH... ");

    #[cfg(windows)]
    {
        // Windows: remove from user PATH via registry
        // Best-effort — don't fail if it can't be modified
        if let Ok(current) = std::env::var("PATH") {
            let bin_dir = super::bin_dir();
            let bin_str = bin_dir.to_string_lossy();
            if current.contains(bin_str.as_ref()) {
                eprintln!("found in PATH (remove manually from System > Environment Variables)");
            } else {
                eprintln!("not in PATH");
            }
        }
    }

    #[cfg(not(windows))]
    {
        let bin_dir = super::bin_dir();
        let bin_str = bin_dir.to_string_lossy();
        let home = dirs_home();

        // Check shell config files for PATH entries
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
                eprintln!("removed from {}", config.display());
                found = true;
            }
        }
        if !found {
            eprintln!("not found in shell configs");
        }
    }
}

fn dirs_home() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
}

fn read_line() -> String {
    use std::io::{self, BufRead, Write};
    let _ = io::stdout().flush();
    let mut line = String::new();
    let stdin = io::stdin();
    let _ = stdin.lock().read_line(&mut line);
    line
}
