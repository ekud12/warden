// ─── Trust — Composite session trust score ───────────────────────────────────
//
// Internal 0-100 score from existing session state signals.
// Never shown to user — used by adaptive silence budget and intervention levels.
//
// High trust = healthy session, Warden stays quiet.
// Low trust = degraded session, Warden increases intervention.
//
// Engine: Anchor (gates the injection budget)
// ──────────────────────────────────────────────────────────────────────────────

use crate::common::SessionState;

/// Compute session trust score (0-100) from current state
pub fn compute_trust(state: &SessionState) -> u32 {
    use crate::config;
    let mut score: i32 = 100;

    score -= (state.errors_unresolved as i32) * config::TRUST_WEIGHT_ERRORS;
    score -= (state.edits_since_verification as i32) * config::TRUST_WEIGHT_VERIFICATION_DEBT;
    score -= (state.subsystem_switches as i32) * config::TRUST_WEIGHT_SUBSYSTEM_SWITCHES;
    score -= (state.dead_ends.len() as i32) * config::TRUST_WEIGHT_DEAD_ENDS;
    score -= state.turns_since_checkpoint as i32 * config::TRUST_WEIGHT_CHECKPOINT_GAP;

    let recent_denials = state
        .recent_denial_turns
        .iter()
        .filter(|&&t| t + 10 >= state.turn)
        .count() as i32;
    score -= recent_denials * config::TRUST_WEIGHT_RECENT_DENIALS;

    let milestone_bonus = if state.last_milestone.is_empty() {
        0
    } else {
        config::TRUST_MILESTONE_BONUS
    };
    score += milestone_bonus;

    // Reward low exploration ratio
    if state.reads_since_edit < 3 && state.edits_since_verification < 3 {
        score += 5;
    }

    score.clamp(0, 100) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_session_high_trust() {
        let state = SessionState {
            last_milestone: "test passed".to_string(),
            ..SessionState::default()
        };
        assert!(compute_trust(&state) >= 75);
    }

    #[test]
    fn degraded_session_low_trust() {
        let state = SessionState {
            errors_unresolved: 3,
            edits_since_verification: 6,
            subsystem_switches: 4,
            dead_ends: vec!["a".into(), "b".into(), "c".into()],
            turns_since_checkpoint: 10,
            ..SessionState::default()
        };
        assert!(compute_trust(&state) < 40);
    }
}
