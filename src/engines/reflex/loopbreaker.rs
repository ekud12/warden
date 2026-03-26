// ─── Engine: Reflex — Loopbreaker ────────────────────────────────────────────
//
// Unified repetition detection combining structural motifs and entropy:
//   - 2-grams: A→B→A→B (ping-pong between two actions)
//   - 3-grams: A→B→C→A→B→C (three-step cycles)
//   - Read spirals: 5+ consecutive reads without edit
//   - Shannon entropy: low entropy = agent doing the same thing repeatedly
//   - Action novelty: fraction of unique actions in recent window
// ──────────────────────────────────────────────────────────────────────────────

use std::collections::{HashMap, HashSet};

use crate::engines::signal::{Signal, SignalCategory};

// ─── Action type constants ───────────────────────────────────────────────────

pub const ACTION_READ: &str = "read";
pub const ACTION_EDIT: &str = "edit";
pub const ACTION_BASH_OK: &str = "bash_ok";
pub const ACTION_BASH_FAIL: &str = "bash_fail";
pub const ACTION_ERROR: &str = "error";
pub const ACTION_MILESTONE: &str = "milestone";

// ─── N-gram pattern detection ────────────────────────────────────────────────

/// Check for behavioral loop patterns in action history
pub fn check_loop_patterns(history: &[String]) -> Option<String> {
    if history.len() < 6 {
        return None;
    }

    let recent = &history[history.len().saturating_sub(8)..];

    // Read spiral: 5+ consecutive reads without edit (check first — most actionable)
    let read_run = recent
        .iter()
        .rev()
        .take_while(|a| a.starts_with("read"))
        .count();
    if read_run >= 5 {
        return Some(format!(
            "{} consecutive reads without an edit. Choose one candidate and act on it.",
            read_run
        ));
    }

    // 2-gram detection: A→B→A→B (different actions alternating)
    if recent.len() >= 4 {
        let (a, b) = (&recent[recent.len() - 4], &recent[recent.len() - 3]);
        let (c, d) = (&recent[recent.len() - 2], &recent[recent.len() - 1]);
        if a == c && b == d && a != b {
            return Some(format!(
                "Repeating pattern: {} → {} → {} → {}. Break the loop — try a different approach.",
                a, b, c, d
            ));
        }
    }

    // 3-gram detection: A→B→C→A→B→C (three distinct actions cycling)
    if recent.len() >= 6 {
        let w = &recent[recent.len() - 6..];
        if w[0] == w[3] && w[1] == w[4] && w[2] == w[5] && !(w[0] == w[1] && w[1] == w[2]) {
            return Some(format!(
                "Repeating 3-step pattern: {} → {} → {}. Step back and reconsider.",
                w[0], w[1], w[2]
            ));
        }
    }

    None
}

// ─── Entropy detection ───────────────────────────────────────────────────────

/// Compute Shannon entropy of action distribution in a window.
/// Returns entropy in bits (0 = all same action, ~2.58 = uniform over 6 types).
pub fn shannon_entropy(actions: &[String]) -> f64 {
    if actions.is_empty() {
        return 0.0;
    }

    let mut counts: HashMap<&str, u32> = HashMap::new();
    for action in actions {
        *counts.entry(action.as_str()).or_default() += 1;
    }

    let n = actions.len() as f64;
    let mut entropy = 0.0f64;

    for &count in counts.values() {
        let p = count as f64 / n;
        if p > 0.0 {
            entropy -= p * p.log2();
        }
    }

    entropy
}

/// Analyze action entropy and return advisory if drift is detected.
pub fn check_drift(actions: &[String], has_recent_edits: bool) -> Option<String> {
    if actions.len() < 8 {
        return None;
    }

    let window = if actions.len() > 10 {
        &actions[actions.len() - 10..]
    } else {
        actions
    };

    let entropy = shannon_entropy(window);
    let read_count = window.iter().filter(|a| a.as_str() == ACTION_READ).count();
    let edit_count = window.iter().filter(|a| a.as_str() == ACTION_EDIT).count();

    // Low entropy + mostly reads + no edits = exploration spiral
    if entropy < 1.0 && read_count >= 7 && edit_count == 0 && !has_recent_edits {
        return Some(format!(
            "Action entropy: {:.2} (low). {} reads, 0 edits in last {} actions. If exploring for a new task, this is expected. Otherwise, consider narrowing focus.",
            entropy, read_count, window.len()
        ));
    }

    // Very low entropy with errors = stuck
    if entropy < 0.8 {
        let error_count = window
            .iter()
            .filter(|a| a.as_str() == ACTION_BASH_FAIL || a.as_str() == ACTION_ERROR)
            .count();
        if error_count >= 5 {
            return Some(format!(
                "Action entropy: {:.2} (very low). {} errors in last {} actions. Try a different approach or ask for guidance.",
                entropy, error_count, window.len()
            ));
        }
    }

    None
}

// ─── Novelty scoring ─────────────────────────────────────────────────────────

/// Action novelty scoring — fraction of unique actions in recent window.
/// Low novelty (< 0.3) + no recent milestone = stronger loop advisory.
pub fn action_novelty(history: &[String]) -> f64 {
    let window_size = 10;
    let recent = &history[history.len().saturating_sub(window_size)..];
    if recent.is_empty() {
        return 1.0;
    }
    let unique: HashSet<&String> = recent.iter().collect();
    unique.len() as f64 / recent.len() as f64
}

// ─── Signal production ───────────────────────────────────────────────────────

pub fn check_loop_signal(history: &[String]) -> Option<Signal> {
    check_loop_patterns(history).map(|msg| Signal::advisory(SignalCategory::Loop, 0.9, msg, "loopbreaker"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── N-gram tests ────────────────────────────────────────────────────────

    #[test]
    fn detects_2gram_loop() {
        let history: Vec<String> = vec![
            "bash_ok", "read", "bash_fail", "read", "bash_fail", "read", "bash_fail", "read",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let result = check_loop_patterns(&history);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Repeating pattern"));
    }

    #[test]
    fn detects_read_spiral() {
        let history: Vec<String> = vec![
            "edit", "bash_ok", "read", "read", "read", "read", "read", "read",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let result = check_loop_patterns(&history);
        assert!(result.is_some());
        assert!(result.unwrap().contains("consecutive reads"));
    }

    #[test]
    fn no_loop_in_normal_history() {
        let history: Vec<String> = vec![
            "read", "edit", "bash_ok", "read", "bash_ok", "edit", "bash_ok", "read",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert!(check_loop_patterns(&history).is_none());
    }

    // ─── Entropy tests ───────────────────────────────────────────────────────

    #[test]
    fn entropy_uniform() {
        let actions: Vec<String> =
            vec!["read", "edit", "bash_ok", "error", "milestone", "bash_fail"]
                .into_iter()
                .map(String::from)
                .collect();
        let e = shannon_entropy(&actions);
        assert!(e > 2.0, "uniform distribution should have high entropy: {}", e);
    }

    #[test]
    fn entropy_all_same() {
        let actions: Vec<String> = vec!["read"; 10].into_iter().map(String::from).collect();
        let e = shannon_entropy(&actions);
        assert!(e < 0.01, "all same should have zero entropy: {}", e);
    }

    #[test]
    fn drift_detected_all_reads() {
        let actions: Vec<String> = vec!["read"; 10].into_iter().map(String::from).collect();
        let result = check_drift(&actions, false);
        assert!(result.is_some(), "should detect drift with all reads");
    }

    #[test]
    fn no_drift_with_edits() {
        let actions: Vec<String> = vec![
            "read", "edit", "read", "edit", "read", "bash_ok", "read", "edit", "read", "bash_ok",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let result = check_drift(&actions, true);
        assert!(result.is_none(), "should not detect drift with edits");
    }
}
