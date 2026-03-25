// ─── analytics::markov — action transition prediction ─────────────────────────
//
// Tracks P(next_action | current_action) from transition counts.
// Predicts likely next action and warns when high-probability error patterns
// are detected. Updated every turn with O(1) operations.
// ──────────────────────────────────────────────────────────────────────────────

use std::collections::BTreeMap as HashMap;

/// Record an action transition (prev → current)
pub fn record_transition(transitions: &mut HashMap<String, u32>, prev: &str, current: &str) {
    let key = format!("{}→{}", prev, current);
    *transitions.entry(key).or_default() += 1;
}

/// Get transition probability P(to | from)
fn transition_probability(transitions: &HashMap<String, u32>, from: &str, to: &str) -> f64 {
    let prefix = format!("{}→", from);
    let total: u32 = transitions
        .iter()
        .filter(|(k, _)| k.starts_with(&prefix))
        .map(|(_, &v)| v)
        .sum();

    if total == 0 {
        return 0.0;
    }

    let key = format!("{}→{}", from, to);
    let count = transitions.get(&key).copied().unwrap_or(0);
    count as f64 / total as f64
}

/// Check for risky patterns in the transition matrix
pub fn check_patterns(
    transitions: &HashMap<String, u32>,
    current_action: &str,
    action_history: &[String],
) -> Option<String> {
    let total_transitions: u32 = transitions.values().sum();
    if total_transitions < 15 {
        return None; // Not enough data
    }

    // Pattern 1: read→read→read spiral (high probability of continuing reads)
    if current_action == "read" {
        let p_read_read = transition_probability(transitions, "read", "read");
        if p_read_read > 0.7 {
            // Check if we're actually in a read chain
            let recent_reads = action_history
                .iter()
                .rev()
                .take(3)
                .filter(|a| a.as_str() == "read")
                .count();
            if recent_reads >= 3 {
                return Some(format!(
                    "Read chains detected ({:.0}% probability of continuing). If exploring intentionally, continue. Otherwise, consider starting edits.",
                    p_read_read * 100.0
                ));
            }
        }
    }

    // Pattern 2: edit→error cycle (high probability of error after edit)
    if current_action == "edit" {
        let p_edit_error = transition_probability(transitions, "edit", "bash_fail")
            + transition_probability(transitions, "edit", "error");
        if p_edit_error > 0.5 {
            return Some(format!(
                "Pattern detected: {:.0}% of your edits lead to errors. Consider verifying approach before editing.",
                p_edit_error * 100.0
            ));
        }
    }

    // Pattern 3: bash_fail→bash_fail (retrying same failing command)
    if current_action == "bash_fail" {
        let p_fail_fail = transition_probability(transitions, "bash_fail", "bash_fail");
        if p_fail_fail > 0.5 {
            return Some(format!(
                "Pattern detected: {:.0}% of failed commands lead to another failure. Try a different approach.",
                p_fail_fail * 100.0
            ));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_tracking() {
        let mut transitions = HashMap::new();
        record_transition(&mut transitions, "read", "read");
        record_transition(&mut transitions, "read", "read");
        record_transition(&mut transitions, "read", "edit");

        let p = transition_probability(&transitions, "read", "read");
        assert!(
            (p - 0.667).abs() < 0.01,
            "P(read→read) should be ~0.667, got {}",
            p
        );
    }

    #[test]
    fn read_spiral_detection() {
        let mut transitions = HashMap::new();
        for _ in 0..20 {
            record_transition(&mut transitions, "read", "read");
        }
        record_transition(&mut transitions, "read", "edit");

        let history: Vec<String> = vec!["read"; 5].into_iter().map(String::from).collect();
        let result = check_patterns(&transitions, "read", &history);
        assert!(result.is_some(), "should detect read spiral");
    }
}
