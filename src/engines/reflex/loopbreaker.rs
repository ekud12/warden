// ─── Engine: Reflex — Loopbreaker ────────────────────────────────────────────
//
// Detects repeated behavioral motifs beyond single-command repeats:
//   - 2-grams: A→B→A→B (ping-pong between two actions)
//   - 3-grams: A→B→C→A→B→C (three-step cycles)
//   - Read spirals: 5+ consecutive reads without edit
//
// TODO: Consider absorbing entropy detection (reflex/entropy.rs) — both modules
// detect repetition patterns. Loopbreaker checks structural motifs while entropy
// tracks Shannon entropy of action distributions. A unified "repetition detector"
// could combine n-gram detection with entropy scoring for stronger signals.

use crate::engines::signal::{Signal, SignalCategory};

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
        if w[0] == w[3] && w[1] == w[4] && w[2] == w[5] && !(w[0] == w[1] && w[1] == w[2])
        // skip all-same (caught by read spiral)
        {
            return Some(format!(
                "Repeating 3-step pattern: {} → {} → {}. Step back and reconsider.",
                w[0], w[1], w[2]
            ));
        }
    }

    None
}

/// F.4: Action novelty scoring — fraction of unique actions in recent window.
/// Low novelty (< 0.3) + no recent milestone = stronger loop advisory.
/// Returns a novelty score 0.0-1.0
pub fn action_novelty(history: &[String]) -> f64 {
    let window_size = 10;
    let recent = &history[history.len().saturating_sub(window_size)..];
    if recent.is_empty() {
        return 1.0;
    }

    let unique: std::collections::HashSet<&String> = recent.iter().collect();
    unique.len() as f64 / recent.len() as f64
}

pub fn check_loop_signal(history: &[String]) -> Option<Signal> {
    check_loop_patterns(history).map(|msg| Signal::advisory(SignalCategory::Loop, 0.9, msg, "loopbreaker"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_2gram_loop() {
        let history: Vec<String> = vec![
            "bash_ok",
            "read",
            "bash_fail",
            "read",
            "bash_fail",
            "read",
            "bash_fail",
            "read",
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
}
