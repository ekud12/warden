// ─── Gatekeeper — Central decision point ─────────────────────────────────────
//
// All Reflex signals feed into a single decision. Replaces the current
// sequential 10-stage pipeline in pretool_bash with a unified interface.
//
// Future: implement the Gate trait to centralize all allow/deny/advisory logic.
// ──────────────────────────────────────────────────────────────────────────────

use crate::engines::signal::{Signal, Verdict};

/// Central decision trait — all Reflex checks produce signals,
/// the Gatekeeper weighs them into a single Verdict.
pub trait Gate {
    fn decide(&self, signals: &[Signal]) -> Verdict;
}
