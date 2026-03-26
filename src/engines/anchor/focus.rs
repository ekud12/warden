// ─── Engine: Anchor — Focus score computation ─────────────────────────────────
//
// Composite 0-100 score measuring session focus. Penalizes directory spread,
// subsystem switches without milestones, and excessive exploration without edits.

use crate::common::SessionState;
use crate::engines::signal::{Signal, SignalCategory};

pub struct FocusReport {
    pub score: u32,
    pub advisory: Option<String>,
}

/// Compute focus score from session state
pub fn compute_focus(state: &SessionState) -> FocusReport {
    let dir_count = state.directories_touched.len();
    let switches = state.subsystem_switches;
    let reads_no_edit = state.reads_since_edit;

    // Penalties
    let dir_penalty = (dir_count.saturating_sub(3) * 10).min(40) as u32;
    let switch_penalty = (switches * 8).min(30);
    let explore_penalty = (reads_no_edit.saturating_sub(5) * 5).min(30);

    let score = 100u32.saturating_sub(dir_penalty + switch_penalty + explore_penalty);

    let advisory = if score < 40 && state.turn >= 8 {
        Some(format!(
            "Focus score {}/100. {} dirs touched, {} subsystem switches without milestone. Narrow scope.",
            score, dir_count, switches
        ))
    } else {
        None
    };

    FocusReport { score, advisory }
}

pub fn compute_focus_signal(state: &SessionState) -> Option<Signal> {
    let report = compute_focus(state);
    report.advisory.map(|msg| Signal {
        category: SignalCategory::Focus,
        utility: 0.5,
        message: msg,
        source: "focus",
    })
}
