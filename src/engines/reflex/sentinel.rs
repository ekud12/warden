// ─── Sentinel — Unified safety pattern matching ─────────────────────────────
//
// Consolidates safety + hallucination + destructive + zero-trace checks into
// Signal-producing functions. Each match produces a Signal with a Verdict.
//
// Uses the merged rules (compiled defaults + TOML overrides).
// ──────────────────────────────────────────────────────────────────────────────

use crate::engines::signal::{Signal, SignalCategory, Verdict};

/// Check a command against safety patterns, returning Signals for matches.
pub fn check_command(cmd: &str) -> Vec<Signal> {
    let rules: &crate::rules::MergedRules = &crate::rules::RULES;
    let mut signals = Vec::new();

    // Safety patterns → Deny
    for (id, pattern, msg, shadow) in rules.safety_pairs.iter() {
        if *shadow {
            continue;
        }
        if let Ok(re) = regex::Regex::new(pattern)
            && re.is_match(cmd) {
                signals.push(Signal::with_verdict(
                    SignalCategory::Safety,
                    1.0,
                    msg.clone(),
                    "sentinel.safety",
                    Verdict::Deny(format!("[{}] {}", id, msg)),
                ));
            }
    }

    // Destructive patterns → Deny
    for (id, pattern, msg, shadow) in &rules.destructive_pairs {
        if *shadow {
            continue;
        }
        if let Ok(re) = regex::Regex::new(pattern)
            && re.is_match(cmd) {
                signals.push(Signal::with_verdict(
                    SignalCategory::Safety,
                    0.9,
                    msg.clone(),
                    "sentinel.destructive",
                    Verdict::Deny(format!("[{}] {}", id, msg)),
                ));
            }
    }

    // Hallucination patterns → Deny
    for (id, pattern, msg, shadow) in &rules.hallucination_pairs {
        if *shadow {
            continue;
        }
        if let Ok(re) = regex::Regex::new(pattern)
            && re.is_match(cmd) {
                signals.push(Signal::with_verdict(
                    SignalCategory::Safety,
                    0.95,
                    msg.clone(),
                    "sentinel.hallucination",
                    Verdict::Deny(format!("[{}] {}", id, msg)),
                ));
            }
    }

    // Hallucination advisory patterns → Advisory (non-blocking)
    for (_id, pattern, msg, _shadow) in &rules.hallucination_advisory_pairs {
        if let Ok(re) = regex::Regex::new(pattern)
            && re.is_match(cmd) {
                signals.push(Signal::with_verdict(
                    SignalCategory::Safety,
                    0.5,
                    msg.clone(),
                    "sentinel.advisory",
                    Verdict::Advisory(msg.clone()),
                ));
            }
    }

    signals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_blocks_rm_rf() {
        let signals = check_command("rm -rf /tmp/important");
        assert!(!signals.is_empty(), "rm -rf should produce signals");
        assert!(signals.iter().any(|s| matches!(&s.verdict, Some(Verdict::Deny(_)))));
    }

    #[test]
    fn sentinel_blocks_sudo() {
        let signals = check_command("sudo apt install foo");
        assert!(!signals.is_empty(), "sudo should produce signals");
    }

    #[test]
    fn sentinel_allows_safe_command() {
        let signals = check_command("cargo build --release");
        let denies: Vec<_> = signals.iter().filter(|s| matches!(&s.verdict, Some(Verdict::Deny(_)))).collect();
        assert!(denies.is_empty(), "cargo build should not be denied: {:?}", denies);
    }

    #[test]
    fn sentinel_blocks_chmod_777() {
        let signals = check_command("chmod 777 /tmp/app");
        assert!(!signals.is_empty(), "chmod 777 should produce signals");
    }
}
