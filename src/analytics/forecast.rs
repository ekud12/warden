// ─── analytics::forecast — token budget forecasting via linear regression ────
//
// Fits a line to (turn, cumulative_tokens) from recent snapshots.
// Extrapolates to predict when compaction will occur.
// Pure math, O(n) where n = number of snapshots (max 20).
// ──────────────────────────────────────────────────────────────────────────────

/// Forecast result
pub struct TokenForecast {
    /// Estimated turn when compaction will occur
    pub compaction_turn: u32,
    /// Turns remaining until compaction
    pub turns_remaining: u32,
    /// Estimated tokens per turn (slope of regression line)
    pub tokens_per_turn: u64,
}

/// Predict compaction timing from snapshot data.
/// Returns None if not enough data or consumption rate is too low.
pub fn predict_compaction(
    snapshots: &[crate::common::TurnSnapshot],
    current_turn: u32,
    cumulative_tokens: u64,
    budget: u64,
) -> Option<TokenForecast> {
    if snapshots.len() < 5 || budget == 0 {
        return None;
    }

    // Build (x, y) pairs: (turn, estimated cumulative tokens at that turn)
    // We approximate cumulative by summing deltas from snapshots
    let mut cumulative = 0u64;
    let mut points: Vec<(f64, f64)> = Vec::with_capacity(snapshots.len());
    for snap in snapshots {
        cumulative += snap.tokens_in_delta + snap.tokens_out_delta;
        points.push((snap.turn as f64, cumulative as f64));
    }

    // Add current point
    points.push((current_turn as f64, cumulative_tokens as f64));

    // Linear regression: y = mx + b
    let n = points.len() as f64;
    let sum_x: f64 = points.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = points.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = points.iter().map(|(x, y)| x * y).sum();
    let sum_xx: f64 = points.iter().map(|(x, _)| x * x).sum();

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < f64::EPSILON {
        return None;
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denom; // tokens per turn
    let intercept = (sum_y - slope * sum_x) / n;

    if slope <= 0.0 {
        return None; // Not consuming tokens (shouldn't happen)
    }

    // Predict turn when cumulative hits budget
    // budget = slope * turn + intercept → turn = (budget - intercept) / slope
    let compaction_turn = ((budget as f64 - intercept) / slope).ceil() as u32;
    let turns_remaining = compaction_turn.saturating_sub(current_turn);

    Some(TokenForecast {
        compaction_turn,
        turns_remaining,
        tokens_per_turn: slope as u64,
    })
}

/// Format a human-readable forecast message
pub fn format_forecast(forecast: &TokenForecast) -> String {
    if forecast.turns_remaining <= 5 {
        format!(
            "Context compaction imminent (~{} turns). Wrap up current task and save state.",
            forecast.turns_remaining
        )
    } else if forecast.turns_remaining <= 15 {
        format!(
            "Compaction in ~{} turns at current rate (~{}K/turn). Plan ahead.",
            forecast.turns_remaining,
            forecast.tokens_per_turn / 1000
        )
    } else {
        String::new() // No warning needed
    }
}

/// Signal wrapper: returns Pressure signal when compaction is imminent
pub fn predict_compaction_signal(
    snapshots: &[crate::common::TurnSnapshot],
    current_turn: u32,
    cumulative_tokens: u64,
    budget: u64,
) -> Option<crate::engines::signal::Signal> {
    let forecast = predict_compaction(snapshots, current_turn, cumulative_tokens, budget)?;
    if forecast.turns_remaining > 5 {
        return None;
    }
    let msg = format_forecast(&forecast);
    if msg.is_empty() {
        return None;
    }
    Some(crate::engines::signal::Signal {
        category: crate::engines::signal::SignalCategory::Pressure,
        utility: 0.4,
        message: msg,
        source: "forecast",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::TurnSnapshot;

    #[test]
    fn forecast_linear_growth() {
        let snaps: Vec<TurnSnapshot> = (1..=10)
            .map(|t| TurnSnapshot {
                turn: t,
                tokens_in_delta: 10_000,
                tokens_out_delta: 2_000,
                ..Default::default()
            })
            .collect();

        let result = predict_compaction(&snaps, 10, 120_000, 700_000);
        assert!(result.is_some());
        let f = result.unwrap();
        assert!(f.tokens_per_turn > 5_000 && f.tokens_per_turn < 20_000);
        assert!(f.turns_remaining > 20);
    }

    #[test]
    fn forecast_insufficient_data() {
        let snaps = vec![TurnSnapshot {
            turn: 1,
            tokens_in_delta: 10_000,
            ..Default::default()
        }];
        let result = predict_compaction(&snaps, 1, 10_000, 700_000);
        assert!(result.is_none());
    }
}
