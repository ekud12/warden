// ─── signal — Shared vocabulary for all engines ──────────────────────────────
//
// Every analytics module produces Signal values. The injection budget in
// userprompt_context consumes them through a unified scoring pipeline.
// ──────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

/// Advisory signal categories — maps to the 7-slot injection budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalCategory {
    Safety,
    Loop,
    Verify,
    Phase,
    Recovery,
    Focus,
    Pressure,
}

/// A signal produced by any engine module, competing for injection into context.
#[derive(Debug, Clone)]
pub struct Signal {
    pub category: SignalCategory,
    /// Injection priority 0.0..1.0 — higher = more likely to be injected.
    pub utility: f64,
    /// The advisory message text.
    pub message: String,
    /// Source module name for tracing.
    pub source: &'static str,
    /// What this signal recommends the Gatekeeper do.
    /// `None` = purely advisory (context injection only, no blocking/transform).
    pub verdict: Option<Verdict>,
}

/// The outcome of a Reflex engine decision.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Allow the action to proceed.
    Allow,
    /// Block the action with a reason.
    Deny(String),
    /// Allow but transform the action (e.g., substitution).
    Transform(serde_json::Value),
    /// Allow but attach an advisory message.
    Advisory(String),
}

/// Resource bounds for a Dream engine task.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Maximum events to scan from the event log.
    pub max_events: usize,
    /// Maximum artifacts to produce or retain.
    pub max_artifacts: usize,
    /// Maximum characters in output.
    pub max_output_chars: usize,
    /// Maximum wall-clock milliseconds before early exit.
    pub max_ms: u64,
}

// ─── Shared pure functions (no handler deps) ─────────────────────────────────

/// Linear regression slope of error count over recent turn snapshots.
/// Positive slope = errors increasing. Used by Compass (adaptation) and Anchor.
pub fn error_slope(snapshots: &[crate::common::TurnSnapshot], window: usize) -> f64 {
    let snaps = if snapshots.len() > window {
        &snapshots[snapshots.len() - window..]
    } else {
        snapshots
    };
    let n = snaps.len() as f64;
    if n < 2.0 {
        return 0.0;
    }

    let mut sum_x = 0.0f64;
    let mut sum_y = 0.0f64;
    let mut sum_xy = 0.0f64;
    let mut sum_xx = 0.0f64;

    for (i, snap) in snaps.iter().enumerate() {
        let x = i as f64;
        let y = snap.errors_unresolved as f64;
        sum_x += x;
        sum_y += y;
        sum_xy += x * y;
        sum_xx += x * x;
    }

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < f64::EPSILON {
        return 0.0;
    }
    (n * sum_xy - sum_x * sum_y) / denom
}
