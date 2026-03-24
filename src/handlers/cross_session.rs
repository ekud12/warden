// ─── cross_session — recurring error detection across sessions ───────────────

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Detect recurring errors across sessions. Returns advisory if 3+ occurrences.
pub fn detect_recurring(session_notes_path: &Path) -> Option<String> {
    let content = fs::read_to_string(session_notes_path).ok()?;

    // Cap input to prevent excessive processing
    let content = if content.len() > 102_400 {
        &content[content.len() - 102_400..]
    } else {
        &content
    };

    // Split into sessions by "session-end" markers
    let mut sessions: Vec<Vec<String>> = Vec::new();
    let mut current_errors: Vec<String> = Vec::new();

    for line in content.lines() {
        let entry: serde_json::Value = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let note_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let detail = entry.get("detail").and_then(|v| v.as_str()).unwrap_or("");

        if note_type == "session-end" {
            if !current_errors.is_empty() {
                sessions.push(std::mem::take(&mut current_errors));
            }
        } else if note_type == "error"
            || note_type == "ts-error"
            || note_type == "build-fail"
            || note_type == "test-fail"
            || note_type == "dep-error"
        {
            // Use first word as error type key
            let error_key = detail.split_whitespace().next().unwrap_or(detail).to_string();
            current_errors.push(error_key);
        }
    }

    // Only look at last 5 sessions
    let recent: Vec<&Vec<String>> = sessions.iter().rev().take(5).collect();
    if recent.len() < 2 {
        return None;
    }

    // Count error types across sessions (count per-session, not per-occurrence)
    let mut error_session_count: HashMap<String, u32> = HashMap::new();
    for session_errors in &recent {
        // Deduplicate within session
        let mut seen: Vec<String> = Vec::new();
        for err in *session_errors {
            if !seen.contains(err) {
                seen.push(err.clone());
                *error_session_count.entry(err.clone()).or_insert(0) += 1;
            }
        }
    }

    // Filter to recurring (3+ sessions)
    let mut recurring: Vec<(String, u32)> = error_session_count
        .into_iter()
        .filter(|(_, count)| *count >= 3)
        .collect();

    if recurring.is_empty() {
        return None;
    }

    recurring.sort_by(|a, b| b.1.cmp(&a.1));
    let items: Vec<String> = recurring
        .iter()
        .map(|(key, count)| format!("{} ({}x)", key, count))
        .collect();

    Some(format!("Recurring issues across sessions: {}. Consider addressing root cause.", items.join(", ")))
}
