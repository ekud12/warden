// ─── pretool_bash::truncation — truncation wrapping + auto-allow ─────────────
//
// Wraps verbose commands with `warden truncate-filter --mode MODE` pipe.
// Mode is detected from command type: test, build, install, or default.

use crate::common;
use crate::config;
use crate::engines::reflex::compiled::PATTERNS;

/// Handle truncation wrapping for verbose commands + auto-allow for safe commands
pub fn handle_truncation(cmd: &str) {
    // If command has unquoted pipes → auto-allow (already piped, safety checks passed)
    let unquoted = strip_quoted_strings(cmd);
    if unquoted.contains('|') {
        common::log(
            "pretool-bash",
            &format!("ALLOW (piped): {}", common::truncate(cmd, 60)),
        );
        common::allow("PreToolUse");
        return;
    }

    // Check compact tools — auto-allow
    for tool in config::COMPACT_TOOLS {
        if cmd.contains(tool) {
            common::log(
                "pretool-bash",
                &format!("ALLOW (compact): {}", common::truncate(cmd, 60)),
            );
            common::allow("PreToolUse");
            return;
        }
    }

    // Check short commands — auto-allow
    for re in &PATTERNS.short {
        if re.is_match(cmd) {
            common::log(
                "pretool-bash",
                &format!("ALLOW (short): {}", common::truncate(cmd, 60)),
            );
            common::allow("PreToolUse");
            return;
        }
    }

    // Check just short recipes — auto-allow
    if let Some(ref re) = PATTERNS.just_short_re
        && re.is_match(cmd)
    {
        common::log(
            "pretool-bash",
            &format!("ALLOW (just-short): {}", common::truncate(cmd, 60)),
        );
        common::allow("PreToolUse");
        return;
    }

    // Check just verbose recipes — wrap with truncation (detect mode from recipe name)
    if let Some(ref re) = PATTERNS.just_verbose_re
        && re.is_match(cmd)
    {
        let mode = detect_mode(cmd);
        wrap_with_truncation(cmd, &mode);
        return;
    }

    // Check verbose patterns — wrap with truncation
    for re in &PATTERNS.verbose {
        if re.is_match(cmd) {
            let mode = detect_mode(cmd);
            wrap_with_truncation(cmd, &mode);
            return;
        }
    }

    // Final check: auto-allow known safe commands (single RegexSet pass)
    if PATTERNS.auto_allow_set.is_match(cmd) {
        common::log(
            "pretool-bash",
            &format!("ALLOW (auto): {}", common::truncate(cmd, 60)),
        );
        common::allow("PreToolUse");
        return;
    }

    // Truly unknown command — silent passthrough (permission system decides)
    common::log(
        "pretool-bash",
        &format!("PASS: {}", common::truncate(cmd, 60)),
    );
}

/// Detect filter mode from command content
fn detect_mode(cmd: &str) -> String {
    if is_test_cmd(cmd) {
        "test".to_string()
    } else if is_install_cmd(cmd) {
        "install".to_string()
    } else if is_build_cmd(cmd) {
        "build".to_string()
    } else {
        "default".to_string()
    }
}

fn is_test_cmd(cmd: &str) -> bool {
    config::TEST_CMDS.iter().any(|p| cmd.contains(p)) || cmd.contains("just test")
}

fn is_build_cmd(cmd: &str) -> bool {
    config::BUILD_CMDS.iter().any(|p| cmd.contains(p))
        || cmd.contains("just build")
        || cmd.contains("just lint")
        || cmd.contains("just tsc")
        || cmd.contains("just typecheck")
}

fn is_install_cmd(cmd: &str) -> bool {
    cmd.contains("install")
        || cmd.contains("restore")
        || cmd.contains("add ")
        || cmd.contains("just install")
}

/// Wrap command with truncation filter
fn wrap_with_truncation(cmd: &str, mode: &str) {
    let filter_bin = truncation_binary();
    let wrapped = format!(
        "{} 2>&1 | \"{}\" truncate-filter --mode {}",
        cmd, filter_bin, mode
    );

    common::log(
        "pretool-bash",
        &format!("TRUNCATE ({}): {}", mode, common::truncate(cmd, 60)),
    );

    let updated = serde_json::json!({ "command": wrapped });
    common::allow_with_update("PreToolUse", updated);
}

/// Resolve path to binary for truncation pipe.
/// Prefers deployed ~/.claude/hooks/<name>.exe, falls back to current exe.
fn truncation_binary() -> String {
    let exe_name = format!("{}.exe", crate::constants::NAME);
    let hooks_bin = common::hooks_dir().join(&exe_name);
    if hooks_bin.exists() {
        return hooks_bin.to_string_lossy().replace('\\', "/");
    }
    // Fallback: use current exe path
    std::env::current_exe()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| crate::constants::NAME.to_string())
}

/// Strip quoted strings from command to check for unquoted pipe chars
pub fn strip_quoted_strings(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '"' {
            i += 1;
            while i < len && chars[i] != '"' {
                i += 1;
            }
            i += 1; // skip closing quote
        } else if chars[i] == '\'' {
            i += 1;
            while i < len && chars[i] != '\'' {
                i += 1;
            }
            i += 1; // skip closing quote
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}
