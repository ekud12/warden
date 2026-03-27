// ─── Sentinel — Unified safety pattern matching ─────────────────────────────
//
// Consolidates safety + hallucination + destructive + zero-trace checks into
// Signal-producing functions. Each match produces a Signal with a Verdict.
//
// Uses compiled RegexSets from reflex::compiled::PATTERNS for O(1) matching
// instead of recompiling Regex::new() per pattern per call.
// ──────────────────────────────────────────────────────────────────────────────

use super::compiled::PATTERNS;
use super::normalize;
use crate::engines::signal::{Signal, SignalCategory, Verdict};

/// Check a command against safety patterns, returning Signals for matches.
/// Normalizes the command first (whitespace, quotes, aliases) and checks
/// each compound sub-command (split on &&, ||, ;) independently.
pub fn check_command(cmd: &str) -> Vec<Signal> {
    let parts = normalize::normalize(cmd);
    let mut signals = Vec::new();
    for part in &parts {
        check_single_command(part, &mut signals);
    }
    signals
}

/// Check a single (non-compound) normalized command against all pattern sets.
fn check_single_command(cmd: &str, signals: &mut Vec<Signal>) {
    let p = &*PATTERNS;

    // Safety patterns → Deny (using compiled RegexSet for single-pass matching)
    for idx in p.safety_set.matches(cmd).into_iter() {
        if p.safety_shadow.get(idx).copied().unwrap_or(false) {
            continue;
        }
        let id = &p.safety_ids[idx];
        let msg = &p.safety_messages[idx];
        signals.push(Signal::with_verdict(
            SignalCategory::Safety,
            1.0,
            msg.clone(),
            "sentinel.safety",
            Verdict::Deny(format!("[{}] {}", id, msg)),
        ));
    }

    // Destructive patterns → Deny
    for idx in p.destructive_set.matches(cmd).into_iter() {
        if p.destructive_shadow.get(idx).copied().unwrap_or(false) {
            continue;
        }
        let id = &p.destructive_ids[idx];
        let msg = &p.destructive_messages[idx];
        signals.push(Signal::with_verdict(
            SignalCategory::Safety,
            0.9,
            msg.clone(),
            "sentinel.destructive",
            Verdict::Deny(format!("[{}] {}", id, msg)),
        ));
    }

    // Hallucination patterns → Deny
    for idx in p.hallucination_set.matches(cmd).into_iter() {
        if p.hallucination_shadow.get(idx).copied().unwrap_or(false) {
            continue;
        }
        let id = &p.hallucination_ids[idx];
        let msg = &p.hallucination_messages[idx];
        signals.push(Signal::with_verdict(
            SignalCategory::Safety,
            0.95,
            msg.clone(),
            "sentinel.hallucination",
            Verdict::Deny(format!("[{}] {}", id, msg)),
        ));
    }

    // Hallucination advisory patterns → Advisory (non-blocking)
    for idx in p.hallucination_advisory_set.matches(cmd).into_iter() {
        let msg = &p.hallucination_advisory_messages[idx];
        signals.push(Signal::with_verdict(
            SignalCategory::Safety,
            0.5,
            msg.clone(),
            "sentinel.advisory",
            Verdict::Advisory(msg.clone()),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_blocks_rm_rf() {
        let signals = check_command("rm -rf /tmp/important");
        assert!(!signals.is_empty(), "rm -rf should produce signals");
        assert!(
            signals
                .iter()
                .any(|s| matches!(&s.verdict, Some(Verdict::Deny(_))))
        );
    }

    #[test]
    fn sentinel_blocks_sudo() {
        let signals = check_command("sudo apt install foo");
        assert!(!signals.is_empty(), "sudo should produce signals");
    }

    #[test]
    fn sentinel_allows_safe_command() {
        let signals = check_command("cargo build --release");
        let denies: Vec<_> = signals
            .iter()
            .filter(|s| matches!(&s.verdict, Some(Verdict::Deny(_))))
            .collect();
        assert!(
            denies.is_empty(),
            "cargo build should not be denied: {:?}",
            denies
        );
    }

    #[test]
    fn sentinel_blocks_chmod_777() {
        let signals = check_command("chmod 777 /tmp/app");
        assert!(!signals.is_empty(), "chmod 777 should produce signals");
    }
}
