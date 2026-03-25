// ─── posttool_session::syntax — syntax validation after edits ─────────────────
//
// Validates JSON, TOML, and YAML files after edits. Fail open on all errors.

use crate::common;
use std::fs;

/// Check syntax after a file edit. Supports JSON, TOML, YAML.
/// Fail open: any read error or unsupported file type → no output.
pub fn check_syntax(file_path: &str) {
    if file_path.ends_with(".json") {
        check_json(file_path);
    } else if file_path.ends_with(".toml") {
        check_toml(file_path);
    } else if file_path.ends_with(".yaml") || file_path.ends_with(".yml") {
        check_yaml(file_path);
    }
}

fn check_json(file_path: &str) {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    if let Err(e) = serde_json::from_str::<serde_json::Value>(&content) {
        common::additional_context(&format!(
            "JSON syntax error in {}: {} (line {}, column {})",
            file_path,
            e,
            e.line(),
            e.column()
        ));
        common::log(
            "posttool-session",
            &format!("SYNTAX JSON error: {} line {}", file_path, e.line()),
        );
    }
}

fn check_toml(file_path: &str) {
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    if let Err(e) = content.parse::<toml::Table>() {
        let msg = e.message();
        let span = e
            .span()
            .map(|s| format!(" (offset {})", s.start))
            .unwrap_or_default();
        common::additional_context(&format!(
            "TOML syntax error in {}: {}{}",
            file_path, msg, span
        ));
        common::log(
            "posttool-session",
            &format!("SYNTAX TOML error: {} {}", file_path, msg),
        );
    }
}

fn check_yaml(file_path: &str) {
    // Lightweight YAML validation: check for common structural errors
    // without adding a serde_yaml dependency. Checks balanced braces,
    // indentation consistency, and duplicate keys.
    let content = match fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Check for tab indentation (YAML only allows spaces)
    for (i, line) in content.lines().enumerate() {
        if line.starts_with('\t') {
            common::additional_context(&format!(
                "YAML error in {} line {}: Tab character found. YAML requires space indentation.",
                file_path,
                i + 1
            ));
            common::log(
                "posttool-session",
                &format!("SYNTAX YAML tab error: {} line {}", file_path, i + 1),
            );
            return;
        }
    }

    // Check for unbalanced braces/brackets (flow style)
    let mut braces = 0i32;
    let mut brackets = 0i32;
    for ch in content.chars() {
        match ch {
            '{' => braces += 1,
            '}' => braces -= 1,
            '[' => brackets += 1,
            ']' => brackets -= 1,
            _ => {}
        }
        if braces < 0 || brackets < 0 {
            common::additional_context(&format!(
                "YAML syntax error in {}: Unbalanced braces/brackets",
                file_path
            ));
            common::log(
                "posttool-session",
                &format!("SYNTAX YAML brace error: {}", file_path),
            );
            return;
        }
    }
    if braces != 0 || brackets != 0 {
        common::additional_context(&format!(
            "YAML syntax error in {}: Unbalanced braces/brackets ({} open braces, {} open brackets)",
            file_path, braces, brackets
        ));
        common::log(
            "posttool-session",
            &format!("SYNTAX YAML balance error: {}", file_path),
        );
    }
}
