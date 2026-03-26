// ─── Gatekeeper — Central decision point ─────────────────────────────────────
//
// All Reflex signals feed into a single decision. Replaces the current
// sequential 10-stage pipeline in pretool_bash with a unified interface.
//
// Priority: Deny > Transform > Advisory > Allow.
// First Deny wins. First Transform wins. Advisories merge.
// ──────────────────────────────────────────────────────────────────────────────

use crate::engines::signal::{Signal, Verdict};

/// Central decision trait — all Reflex checks produce signals,
/// the Gatekeeper weighs them into a single Verdict.
pub trait Gate {
    fn decide(&self, signals: &[Signal]) -> Verdict;
}

/// Default implementation: priority-ordered collapse of signals into one verdict.
pub struct DefaultGatekeeper;

impl Gate for DefaultGatekeeper {
    fn decide(&self, signals: &[Signal]) -> Verdict {
        // Pass 1: any Deny blocks immediately (first wins)
        for s in signals {
            if let Some(Verdict::Deny(ref msg)) = s.verdict {
                return Verdict::Deny(msg.clone());
            }
        }

        // Pass 2: first Transform wins
        for s in signals {
            if let Some(Verdict::Transform(ref payload)) = s.verdict {
                return Verdict::Transform(payload.clone());
            }
        }

        // Pass 3: collect all Advisory messages
        let advisories: Vec<&str> = signals
            .iter()
            .filter_map(|s| match &s.verdict {
                Some(Verdict::Advisory(msg)) => Some(msg.as_str()),
                _ => None,
            })
            .collect();

        if !advisories.is_empty() {
            return Verdict::Advisory(advisories.join("\n"));
        }

        Verdict::Allow
    }
}

/// Convenience: run the default gatekeeper on a slice of signals.
pub fn evaluate(signals: &[Signal]) -> Verdict {
    DefaultGatekeeper.decide(signals)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::signal::{Signal, SignalCategory, Verdict};

    /// Helper: build a signal with a given verdict.
    fn sig(source: &'static str, verdict: Option<Verdict>) -> Signal {
        Signal {
            category: SignalCategory::Safety,
            utility: 0.5,
            message: format!("from {}", source),
            source,
            verdict,
        }
    }

    // ── Empty signals → Allow ────────────────────────────────────────────

    #[test]
    fn empty_signals_allow() {
        assert_eq!(evaluate(&[]), Verdict::Allow);
    }

    // ── All None verdicts → Allow ────────────────────────────────────────

    #[test]
    fn none_verdicts_allow() {
        let signals = vec![sig("a", None), sig("b", None)];
        assert_eq!(evaluate(&signals), Verdict::Allow);
    }

    // ── Explicit Allow verdicts → Allow ──────────────────────────────────

    #[test]
    fn explicit_allow_verdicts() {
        let signals = vec![
            sig("a", Some(Verdict::Allow)),
            sig("b", Some(Verdict::Allow)),
        ];
        assert_eq!(evaluate(&signals), Verdict::Allow);
    }

    // ── Single Deny → Deny ───────────────────────────────────────────────

    #[test]
    fn single_deny() {
        let signals = vec![sig("sentinel", Some(Verdict::Deny("blocked".into())))];
        assert_eq!(evaluate(&signals), Verdict::Deny("blocked".into()));
    }

    // ── Deny beats Transform ─────────────────────────────────────────────

    #[test]
    fn deny_beats_transform() {
        let signals = vec![
            sig(
                "substitution",
                Some(Verdict::Transform(serde_json::json!({"cmd": "rg"}))),
            ),
            sig("safety", Some(Verdict::Deny("unsafe".into()))),
        ];
        assert_eq!(evaluate(&signals), Verdict::Deny("unsafe".into()));
    }

    // ── Deny beats Advisory ──────────────────────────────────────────────

    #[test]
    fn deny_beats_advisory() {
        let signals = vec![
            sig("loop", Some(Verdict::Advisory("slow down".into()))),
            sig("safety", Some(Verdict::Deny("rm -rf".into()))),
        ];
        assert_eq!(evaluate(&signals), Verdict::Deny("rm -rf".into()));
    }

    // ── First Deny wins when multiple ────────────────────────────────────

    #[test]
    fn first_deny_wins() {
        let signals = vec![
            sig("a", Some(Verdict::Deny("first".into()))),
            sig("b", Some(Verdict::Deny("second".into()))),
        ];
        assert_eq!(evaluate(&signals), Verdict::Deny("first".into()));
    }

    // ── Transform beats Advisory ─────────────────────────────────────────

    #[test]
    fn transform_beats_advisory() {
        let payload = serde_json::json!({"command": "rg pattern"});
        let signals = vec![
            sig("redirect", Some(Verdict::Transform(payload.clone()))),
            sig("loop", Some(Verdict::Advisory("careful".into()))),
        ];
        assert_eq!(evaluate(&signals), Verdict::Transform(payload));
    }

    // ── First Transform wins when multiple ───────────────────────────────

    #[test]
    fn first_transform_wins() {
        let t1 = serde_json::json!({"cmd": "rg"});
        let t2 = serde_json::json!({"cmd": "fd"});
        let signals = vec![
            sig("a", Some(Verdict::Transform(t1.clone()))),
            sig("b", Some(Verdict::Transform(t2))),
        ];
        assert_eq!(evaluate(&signals), Verdict::Transform(t1));
    }

    // ── Multiple advisories merge ────────────────────────────────────────

    #[test]
    fn advisories_merge() {
        let signals = vec![
            sig("loop", Some(Verdict::Advisory("loop detected".into()))),
            sig("drift", Some(Verdict::Advisory("drifting off-task".into()))),
        ];
        let result = evaluate(&signals);
        assert_eq!(
            result,
            Verdict::Advisory("loop detected\ndrifting off-task".into())
        );
    }

    // ── Single advisory passes through ───────────────────────────────────

    #[test]
    fn single_advisory() {
        let signals = vec![sig("loop", Some(Verdict::Advisory("slow".into())))];
        assert_eq!(evaluate(&signals), Verdict::Advisory("slow".into()));
    }

    // ── Mixed None + Advisory → Advisory ─────────────────────────────────

    #[test]
    fn none_mixed_with_advisory() {
        let signals = vec![
            sig("a", None),
            sig("b", Some(Verdict::Advisory("heads up".into()))),
            sig("c", None),
        ];
        assert_eq!(evaluate(&signals), Verdict::Advisory("heads up".into()));
    }

    // ── Mixed Allow + Advisory → Advisory ────────────────────────────────

    #[test]
    fn allow_mixed_with_advisory() {
        let signals = vec![
            sig("a", Some(Verdict::Allow)),
            sig("b", Some(Verdict::Advisory("note".into()))),
        ];
        assert_eq!(evaluate(&signals), Verdict::Advisory("note".into()));
    }

    // ── Full priority chain: Deny present among all types ────────────────

    #[test]
    fn full_priority_chain() {
        let signals = vec![
            sig("a", None),
            sig("b", Some(Verdict::Allow)),
            sig("c", Some(Verdict::Advisory("info".into()))),
            sig(
                "d",
                Some(Verdict::Transform(serde_json::json!({"x": 1}))),
            ),
            sig("e", Some(Verdict::Deny("nope".into()))),
        ];
        assert_eq!(evaluate(&signals), Verdict::Deny("nope".into()));
    }

    // ── Without Deny: Transform wins over Advisory + Allow ───────────────

    #[test]
    fn no_deny_transform_wins() {
        let payload = serde_json::json!({"rewrite": true});
        let signals = vec![
            sig("a", Some(Verdict::Allow)),
            sig("b", Some(Verdict::Advisory("info".into()))),
            sig("c", Some(Verdict::Transform(payload.clone()))),
        ];
        assert_eq!(evaluate(&signals), Verdict::Transform(payload));
    }

    // ── Gate trait is object-safe (can be used as dyn) ────────────────────

    #[test]
    fn gate_trait_object_safe() {
        let gk: Box<dyn Gate> = Box::new(DefaultGatekeeper);
        let signals = vec![sig("x", Some(Verdict::Allow))];
        assert_eq!(gk.decide(&signals), Verdict::Allow);
    }
}
