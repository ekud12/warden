// ─── posttool_session::compress — bash output compression ────────────────────

use crate::common;
use std::collections::HashMap;

/// Summarize long bash output. Returns Some(summary) if output > 2KB.
pub fn summarize(output: &str) -> Option<String> {
    if output.len() <= 2048 {
        return None;
    }

    let clean = common::strip_ansi(output);
    let mut parts: Vec<String> = Vec::new();

    // Collapse repeated warning lines (group by first 60 chars)
    let mut warning_groups: HashMap<String, u32> = HashMap::new();
    let mut error_lines: Vec<String> = Vec::new();
    let mut last_nonempty = String::new();

    for line in clean.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        last_nonempty = trimmed.to_string();

        if trimmed.starts_with("warning") {
            let key = if trimmed.len() > 60 {
                trimmed[..60].to_string()
            } else {
                trimmed.to_string()
            };
            *warning_groups.entry(key).or_insert(0) += 1;
        } else if trimmed.starts_with("error") && error_lines.len() < 5 {
            error_lines.push(trimmed.to_string());
        }
    }

    // Warnings summary
    if !warning_groups.is_empty() {
        let mut warnings: Vec<String> = warning_groups
            .iter()
            .filter(|(_, count)| **count > 1)
            .map(|(key, count)| format!("{} (x{})", common::truncate(key, 60), count))
            .collect();
        warnings.sort();
        if !warnings.is_empty() {
            parts.push(format!("Warnings: {}", warnings.join("; ")));
        }
    }

    // Errors
    if !error_lines.is_empty() {
        parts.push(format!("Errors:\n{}", error_lines.join("\n")));
    }

    // Final line
    if !last_nonempty.is_empty() {
        parts.push(format!("Final: {}", common::truncate(&last_nonempty, 100)));
    }

    if parts.is_empty() {
        return None;
    }

    Some(format!(
        "Output summary ({} bytes):\n{}",
        output.len(),
        parts.join("\n")
    ))
}
