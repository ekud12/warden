// ─── analytics::verification — verification debt tracking ────────────────────
//
// Counts edits since last successful build/test. Emits advisory when debt
// exceeds threshold, preventing agents from over-editing without validation.

use crate::common::SessionState;

const DEBT_THRESHOLD: u32 = 4;

/// Check verification debt and return advisory if threshold exceeded
pub fn check_debt(state: &SessionState) -> Option<String> {
    let debt = state.edits_since_verification;
    if debt >= DEBT_THRESHOLD {
        Some(format!(
            "{} edits since last build/test. Verify before continuing.",
            debt
        ))
    } else {
        None
    }
}

/// Check if reads without edits indicate exploration without commitment
pub fn check_read_drift(state: &SessionState) -> Option<String> {
    if state.reads_since_edit >= 7 {
        Some(format!(
            "{} reads since last edit. Choose one candidate and act on it.",
            state.reads_since_edit
        ))
    } else {
        None
    }
}
