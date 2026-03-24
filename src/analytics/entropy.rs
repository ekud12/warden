// ─── analytics::entropy — action entropy + drift detection ──────────────────
//
// Tracks Shannon entropy of action types over a sliding window.
// Low entropy + no edits = exploration spiral (likely stuck).
// High entropy + milestones = productive session.
// ──────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;

/// Action types for entropy calculation
pub const ACTION_READ: &str = "read";
pub const ACTION_EDIT: &str = "edit";
pub const ACTION_BASH_OK: &str = "bash_ok";
pub const ACTION_BASH_FAIL: &str = "bash_fail";
pub const ACTION_ERROR: &str = "error";
pub const ACTION_MILESTONE: &str = "milestone";

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
/// Window: last 10 actions.
pub fn check_drift(actions: &[String], has_recent_edits: bool) -> Option<String> {
    if actions.len() < 8 {
        return None; // Not enough data
    }

    let window = if actions.len() > 10 {
        &actions[actions.len() - 10..]
    } else {
        actions
    };

    let entropy = shannon_entropy(window);

    // Count reads in window
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
        let error_count = window.iter().filter(|a| a.as_str() == ACTION_BASH_FAIL || a.as_str() == ACTION_ERROR).count();
        if error_count >= 5 {
            return Some(format!(
                "Action entropy: {:.2} (very low). {} errors in last {} actions. Try a different approach or ask for guidance.",
                entropy, error_count, window.len()
            ));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_uniform() {
        let actions: Vec<String> = vec!["read", "edit", "bash_ok", "error", "milestone", "bash_fail"]
            .into_iter().map(String::from).collect();
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
        let actions: Vec<String> = vec!["read", "edit", "read", "edit", "read", "bash_ok", "read", "edit", "read", "bash_ok"]
            .into_iter().map(String::from).collect();
        let result = check_drift(&actions, true);
        assert!(result.is_none(), "should not detect drift with edits");
    }
}
