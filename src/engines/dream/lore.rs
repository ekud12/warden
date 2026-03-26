// ─── Engine: Dream — Lore ────────────────────────────────────────────────────
//
// Convention learning + cross-project knowledge + cross-session error detection.
// Combines learning.rs (cross-project pattern tracking) and cross_session.rs
// (recurring error detection across sessions).
// ──────────────────────────────────────────────────────────────────────────────

// ═══════════════════════════════════════════════════════════════════════════════
// Part 1: Cross-project pattern tracking (from handlers/learning.rs)
// ═══════════════════════════════════════════════════════════════════════════════

use crate::common;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MAX_ENTRIES: usize = 50;

/// Global learning state — persisted at ~/.warden/learning.json
#[derive(Serialize, Deserialize, Default)]
pub struct LearningState {
    /// Total sessions recorded
    #[serde(default)]
    pub total_sessions: u64,
    /// Total tokens saved across all sessions
    #[serde(default)]
    pub total_tokens_saved: u64,
    /// Denial counts by category (safety, substitution, hallucination, etc.)
    #[serde(default)]
    pub denials_by_category: HashMap<String, u64>,
    /// Most denied commands (command prefix -> count)
    #[serde(default)]
    pub denied_commands: HashMap<String, u64>,
    /// Substitution hits (from -> count)
    #[serde(default)]
    pub substitution_hits: HashMap<String, u64>,
    /// Per-project session counts
    #[serde(default)]
    pub project_sessions: HashMap<String, u64>,
    /// Per-project tokens saved
    #[serde(default)]
    pub project_savings: HashMap<String, u64>,
}

impl LearningState {
    /// Enforce size bounds on all maps
    fn enforce_bounds(&mut self) {
        trim_map(&mut self.denials_by_category, MAX_ENTRIES);
        trim_map(&mut self.denied_commands, MAX_ENTRIES);
        trim_map(&mut self.substitution_hits, MAX_ENTRIES);
        trim_map(&mut self.project_sessions, MAX_ENTRIES);
        trim_map(&mut self.project_savings, MAX_ENTRIES);
    }
}

/// Load global learning state
pub fn load() -> LearningState {
    let path = common::hooks_dir().join("learning.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save global learning state
fn save(state: &LearningState) {
    let path = common::hooks_dir().join("learning.json");
    if let Ok(json) = serde_json::to_string(state) {
        let _ = std::fs::write(&path, json);
    }
}

/// Record session stats into learning state. Called from session-end.
pub fn record_session(project_name: &str) {
    let session = common::read_session_state();

    let mut state = load();
    state.total_sessions += 1;
    state.total_tokens_saved += session.estimated_tokens_saved;

    // Track per-project
    *state
        .project_sessions
        .entry(project_name.to_string())
        .or_insert(0) += 1;
    *state
        .project_savings
        .entry(project_name.to_string())
        .or_insert(0) += session.estimated_tokens_saved;

    // Track denial categories from log files
    record_denials_from_logs(&mut state);

    state.enforce_bounds();
    save(&state);
}

/// Parse log files to count denial categories and denied commands
fn record_denials_from_logs(state: &mut LearningState) {
    let log_dir = common::project_dir().join("logs");
    let bash_log = log_dir.join("pretool-bash.log");

    let content = match std::fs::read_to_string(&bash_log) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Only look at recent entries (last 200 lines)
    let lines: Vec<&str> = content.lines().rev().take(200).collect();

    for line in &lines {
        if line.contains("[DENY]") {
            if line.contains("safety") {
                *state
                    .denials_by_category
                    .entry("safety".into())
                    .or_insert(0) += 1;
            } else if line.contains("substitution") {
                *state
                    .denials_by_category
                    .entry("substitution".into())
                    .or_insert(0) += 1;
                // Extract the tool name from substitution denials
                if let Some(cmd) = extract_denied_command(line) {
                    *state.substitution_hits.entry(cmd).or_insert(0) += 1;
                }
            } else if line.contains("hallucination") {
                *state
                    .denials_by_category
                    .entry("hallucination".into())
                    .or_insert(0) += 1;
            } else if line.contains("destructive") {
                *state
                    .denials_by_category
                    .entry("destructive".into())
                    .or_insert(0) += 1;
            }

            // Track the denied command prefix
            if let Some(cmd) = extract_denied_command(line) {
                *state.denied_commands.entry(cmd).or_insert(0) += 1;
            }
        }
    }
}

/// Extract a short command prefix from a log line
fn extract_denied_command(line: &str) -> Option<String> {
    // Log format: "TIMESTAMP [DENY] category cmd_truncated"
    let after_bracket = line.rsplit(']').next()?;
    let parts: Vec<&str> = after_bracket.trim().splitn(3, ' ').collect();
    if parts.len() >= 2 {
        // Take first word of the command (after category)
        let cmd = parts.last()?.trim();
        let first_word = cmd.split_whitespace().next()?;
        Some(first_word.to_string())
    } else {
        None
    }
}

/// Generate insights from learning state for session-start injection.
pub fn get_insights() -> Option<String> {
    let state = load();
    if state.total_sessions < 3 {
        return None; // Not enough data
    }

    let mut insights = Vec::new();

    // Top substitution patterns (tools you keep reaching for)
    let mut subs: Vec<(&String, &u64)> = state.substitution_hits.iter().collect();
    subs.sort_by(|a, b| b.1.cmp(a.1));
    if let Some((tool, count)) = subs.first()
        && **count >= 5
    {
        insights.push(format!(
            "{} denied {}x — consider training habit",
            tool, count
        ));
    }

    // Most denied category
    let mut cats: Vec<(&String, &u64)> = state.denials_by_category.iter().collect();
    cats.sort_by(|a, b| b.1.cmp(a.1));
    if let Some((cat, count)) = cats.first()
        && **count >= 10
    {
        insights.push(format!("top denial category: {} ({}x)", cat, count));
    }

    // Cross-project savings comparison
    if state.project_savings.len() >= 2 {
        let total_savings: u64 = state.project_savings.values().sum();
        let avg = total_savings / state.project_savings.len() as u64;
        if avg > 1000 {
            insights.push(format!(
                "avg {}K tokens saved/project across {} projects",
                avg / 1000,
                state.project_savings.len()
            ));
        }
    }

    if insights.is_empty() {
        None
    } else {
        Some(format!("Cross-project insights: {}", insights.join("; ")))
    }
}

/// Format stats for `warden stats` output
pub fn format_stats() -> String {
    let state = load();

    let mut out = String::new();
    out.push_str(&format!("Sessions: {}\n", state.total_sessions));
    out.push_str(&format!(
        "Total tokens saved: {}\n\n",
        format_tokens(state.total_tokens_saved)
    ));

    // Denial categories
    if !state.denials_by_category.is_empty() {
        out.push_str("Denials by category:\n");
        let mut cats: Vec<(&String, &u64)> = state.denials_by_category.iter().collect();
        cats.sort_by(|a, b| b.1.cmp(a.1));
        for (cat, count) in &cats {
            out.push_str(&format!("  {}: {}\n", cat, count));
        }
        out.push('\n');
    }

    // Top substitutions
    if !state.substitution_hits.is_empty() {
        out.push_str("Substitution hits:\n");
        let mut subs: Vec<(&String, &u64)> = state.substitution_hits.iter().collect();
        subs.sort_by(|a, b| b.1.cmp(a.1));
        for (tool, count) in subs.iter().take(10) {
            out.push_str(&format!("  {}: {}\n", tool, count));
        }
        out.push('\n');
    }

    // Per-project
    if !state.project_sessions.is_empty() {
        out.push_str("Per-project:\n");
        let mut projects: Vec<(&String, &u64)> = state.project_sessions.iter().collect();
        projects.sort_by(|a, b| b.1.cmp(a.1));
        for (proj, sessions) in &projects {
            let savings = state.project_savings.get(*proj).unwrap_or(&0);
            out.push_str(&format!(
                "  {} — {} sessions, {} tokens saved\n",
                proj,
                sessions,
                format_tokens(*savings)
            ));
        }
    }

    out
}

fn format_tokens(t: u64) -> String {
    if t >= 1_000_000 {
        format!("{:.1}M", t as f64 / 1_000_000.0)
    } else if t >= 1_000 {
        format!("{:.1}K", t as f64 / 1_000.0)
    } else {
        t.to_string()
    }
}

/// Trim a HashMap to max_entries by removing lowest-count entries
fn trim_map(map: &mut HashMap<String, u64>, max: usize) {
    if map.len() <= max {
        return;
    }
    let mut entries: Vec<(String, u64)> = map.drain().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.truncate(max);
    *map = entries.into_iter().collect();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Part 2: Dream task — E9 LearnConventions
// ═══════════════════════════════════════════════════════════════════════════════

use super::ProjectConvention;

/// E9: Learn project conventions from recurring patterns
pub fn learn_conventions() {
    let state = common::read_session_state();
    let mut conventions: Vec<ProjectConvention> = common::storage::read_json("dream", "conventions").unwrap_or_default();

    // Convention: preferred build/test command (most successful)
    if state.last_build_turn > 0 {
        let build_conv_idx = conventions.iter().position(|c| c.kind == "build_preference");
        if let Some(idx) = build_conv_idx {
            conventions[idx].evidence_count += 1;
            conventions[idx].confidence = (conventions[idx].confidence + 0.05).min(1.0);
            conventions[idx].last_updated_turn = state.turn;
        } else {
            conventions.push(ProjectConvention {
                kind: "build_preference".to_string(),
                observation: format!("Project type: {}", state.project_type),
                confidence: 0.5,
                evidence_count: 1,
                last_updated_turn: state.turn,
            });
        }
    }

    // Convention: frequently edited files (co-change candidates)
    let edited_count = state.files_edited.len();
    if edited_count >= 3 {
        let conv_idx = conventions.iter().position(|c| c.kind == "common_edit_set");
        let obs = format!("Common edit set: {}", state.files_edited.iter().take(5).cloned().collect::<Vec<_>>().join(", "));
        if let Some(idx) = conv_idx {
            conventions[idx].observation = obs;
            conventions[idx].evidence_count += 1;
            conventions[idx].confidence = (conventions[idx].confidence + 0.03).min(1.0);
            conventions[idx].last_updated_turn = state.turn;
        } else {
            conventions.push(ProjectConvention {
                kind: "common_edit_set".to_string(),
                observation: obs,
                confidence: 0.3,
                evidence_count: 1,
                last_updated_turn: state.turn,
            });
        }
    }

    // Convention: verification frequency
    if state.edits_since_verification > 0 && state.last_build_turn > 0 {
        let avg_edits_per_verify = state.turn as f64 / (state.last_build_turn as f64).max(1.0);
        let conv_idx = conventions.iter().position(|c| c.kind == "verification_frequency");
        let obs = format!("Avg {:.1} turns between verifications", avg_edits_per_verify);
        if let Some(idx) = conv_idx {
            conventions[idx].observation = obs;
            conventions[idx].evidence_count += 1;
            conventions[idx].last_updated_turn = state.turn;
        } else {
            conventions.push(ProjectConvention {
                kind: "verification_frequency".to_string(),
                observation: obs,
                confidence: 0.4,
                evidence_count: 1,
                last_updated_turn: state.turn,
            });
        }
    }

    conventions.truncate(30);
    let _ = common::storage::write_json("dream", "conventions", &conventions);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Part 3: Recurring error detection across sessions (from handlers/cross_session.rs)
// ═══════════════════════════════════════════════════════════════════════════════

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
            let error_key = detail
                .split_whitespace()
                .next()
                .unwrap_or(detail)
                .to_string();
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

    Some(format!(
        "Recurring issues across sessions: {}. Consider addressing root cause.",
        items.join(", ")
    ))
}
