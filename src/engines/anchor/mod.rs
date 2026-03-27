// ─── Anchor Engine — "Stay Grounded" ─────────────────────────────────────────
//
// Maintains compact session state and prevents drift.
// SLA: <100ms per hook call.
//
// Modules:
//   Compass — drift detection + phase adaptation (5 session phases)
//   Focus   — salience / importance ranking + explore budget
//   Ledger  — session state tracking (turn-by-turn events)
//   Debt    — verification debt tracking (edits since last build/test)
//   Trust   — composite trust score (gates injection budget)
// ──────────────────────────────────────────────────────────────────────────────

pub mod budget;
pub mod compass;
pub mod debt;
pub mod error_prevention;
pub mod focus;
pub mod git_summary;
pub mod ledger;
pub mod postcompact;
pub mod precompact;
pub mod session_end;
pub mod session_start;
pub mod trust;

use super::signal::Signal;
use super::{Engine, EngineContext};

/// Anchor engine: session state and drift prevention.
pub struct AnchorEngine;

impl Engine for AnchorEngine {
    fn name(&self) -> &'static str {
        "anchor"
    }

    fn process(&self, ctx: &EngineContext) -> Vec<Signal> {
        let mut signals = Vec::new();
        let state = crate::common::read_session_state();
        if let Some(sig) = focus::compute_focus_signal(&state) {
            signals.push(sig);
        }
        if let Some(msg) = debt::check_debt(&state) {
            signals.push(Signal::advisory(
                super::signal::SignalCategory::Verify,
                0.8,
                msg,
                "anchor.debt",
            ));
        }
        let _ = ctx; // cmd/output used by ledger (called separately via PostToolUse)
        signals
    }
}
