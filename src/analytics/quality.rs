// ─── analytics::quality — session quality prediction ─────────────────────────
//
// Heuristic ensemble that predicts session quality at turn 10, then every 5 turns.
// Pure math — no ML. Compares early signals against project DNA baselines.
//
// Components:
//   - Edit velocity score (0-100): edits/turn normalized
//   - Error trajectory score (0-100): inverse of error slope
//   - Token efficiency score (0-100): tokens_saved / total ratio
//   - Milestone rate score (0-100): milestones/turn normalized
// ──────────────────────────────────────────────────────────────────────────────

use crate::common::TurnSnapshot;

/// Predict session quality from accumulated snapshots
pub fn predict_quality(
    snapshots: &[TurnSnapshot],
    turn: u32,
    errors_unresolved: u32,
    tokens_saved: u64,
    total_tokens: u64,
) -> Option<QualityPrediction> {
    // Need at least 5 snapshots for meaningful prediction
    if snapshots.len() < 5 || turn < 5 {
        return None;
    }

    // Only predict at turn 10, then every 5 turns
    if turn < 10 || (turn > 10 && !turn.is_multiple_of(5)) {
        return None;
    }

    let n = snapshots.len() as f64;

    // Edit velocity: fraction of turns with edits (0-1)
    let edits_count = snapshots.iter().filter(|s| s.edits_this_turn).count() as f64;
    let edit_velocity = (edits_count / n * 100.0).min(100.0);

    // Error trajectory: inverse of error slope (rising errors = bad)
    let error_slope = crate::handlers::userprompt_context::error_slope(snapshots, snapshots.len());
    let error_score = ((1.0 - error_slope.clamp(0.0, 1.0)) * 100.0).max(0.0);

    // Token efficiency: saved / (total + saved)
    let efficiency = if total_tokens + tokens_saved > 0 {
        (tokens_saved as f64 / (total_tokens + tokens_saved) as f64 * 100.0).min(100.0)
    } else {
        50.0 // neutral
    };

    // Milestone rate: milestones per turn, scaled up
    let milestones = snapshots.iter().filter(|s| s.milestones_hit).count() as f64;
    let milestone_score = (milestones / n * 5.0 * 100.0).min(100.0);

    // Weighted ensemble
    let quality = (
        edit_velocity * 0.30 +
        error_score * 0.30 +
        efficiency * 0.20 +
        milestone_score * 0.20
    ).round() as u32;

    let quality = quality.min(100);

    Some(QualityPrediction {
        score: quality,
        edit_velocity: edit_velocity as u32,
        error_score: error_score as u32,
        efficiency: efficiency as u32,
        milestone_score: milestone_score as u32,
        errors_unresolved,
    })
}

pub struct QualityPrediction {
    pub score: u32,
    pub edit_velocity: u32,
    pub error_score: u32,
    pub efficiency: u32,
    pub milestone_score: u32,
    pub errors_unresolved: u32,
}

impl QualityPrediction {
    pub fn format(&self, project_avg: Option<u32>) -> String {
        let comparison = if let Some(avg) = project_avg {
            if self.score >= avg + 10 {
                " (above average)".to_string()
            } else if self.score + 10 <= avg {
                format!(" (below project avg {})", avg)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if self.score < 40 {
            format!("Session quality: {}/100{}. Consider refocusing — errors: {}, edit velocity low.",
                self.score, comparison, self.errors_unresolved)
        } else if self.score < 60 {
            format!("Session quality: {}/100{}.", self.score, comparison)
        } else {
            String::new() // Don't inject for good sessions — no noise
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::TurnSnapshot;

    #[test]
    fn quality_productive_session() {
        let snaps: Vec<TurnSnapshot> = (1..=10).map(|t| TurnSnapshot {
            turn: t,
            edits_this_turn: t > 3, // editing from turn 4
            milestones_hit: t == 7 || t == 10,
            errors_unresolved: 0,
            ..Default::default()
        }).collect();

        let result = predict_quality(&snaps, 10, 0, 5000, 100000);
        assert!(result.is_some());
        let q = result.unwrap();
        assert!(q.score > 50, "productive session should score >50, got {}", q.score);
    }

    #[test]
    fn quality_struggling_session() {
        let snaps: Vec<TurnSnapshot> = (1..=10).map(|t| TurnSnapshot {
            turn: t,
            edits_this_turn: false,
            milestones_hit: false,
            errors_unresolved: t,
            ..Default::default()
        }).collect();

        let result = predict_quality(&snaps, 10, 10, 0, 100000);
        assert!(result.is_some());
        let q = result.unwrap();
        assert!(q.score < 50, "struggling session should score <50, got {}", q.score);
    }
}
