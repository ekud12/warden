// ─── analytics::dna — project fingerprinting ────────────────────────────────
//
// Builds a statistical identity per project from accumulated session data.
// Loaded at session-start to provide baselines for anomaly detection
// and quality prediction. Updated on session-end.
// ──────────────────────────────────────────────────────────────────────────────

use super::anomaly::ProjectStats;

/// Generate a human-readable project profile for session-start context injection
pub fn format_profile(stats: &ProjectStats) -> Option<String> {
    if stats.session_length.n < 3 {
        return None; // Not enough sessions for a meaningful profile
    }

    let avg_turns = stats.session_length.mean as u32;
    let avg_quality = stats.quality_score.mean as u32;
    let avg_tokens = stats.tokens_per_turn.mean as u64;

    Some(format!(
        "Project profile: {} sessions avg {}K tokens/turn, {} turns, quality {}/100",
        stats.session_length.n,
        avg_tokens / 1000,
        avg_turns,
        avg_quality,
    ))
}

/// Session-end metrics for updating project DNA
pub struct SessionMetrics {
    pub turns: u32,
    pub quality: u32,
    pub avg_tokens_per_turn: u64,
    pub errors: u32,
    pub edit_velocity: f64,
    pub explore_ratio: f64,
    pub denial_rate: f64,
}

/// Update project stats with session-end data
pub fn update_stats(stats: &mut ProjectStats, m: &SessionMetrics) {
    stats.session_length.update(m.turns as f64);
    stats.quality_score.update(m.quality as f64);
    stats.tokens_per_turn.update(m.avg_tokens_per_turn as f64);
    stats.errors_per_session.update(m.errors as f64);
    stats.edit_velocity.update(m.edit_velocity);
    stats.explore_ratio.update(m.explore_ratio);
    stats.denial_rate.update(m.denial_rate);
}
