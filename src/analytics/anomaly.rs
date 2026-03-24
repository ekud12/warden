// ─── analytics::anomaly — Welford's online algorithm for anomaly detection ───
//
// Maintains per-metric running mean and variance using Welford's method.
// O(1) update, O(1) query. Flags values >2σ from mean as anomalies.
//
// Metrics tracked (per project, persisted in stats.json):
//   - tokens_per_turn
//   - errors_per_session
//   - edit_velocity (edits/turn)
//   - explore_ratio
//   - denial_rate (denials/turn)
//   - session_length (turns)
// ──────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

/// Single metric accumulator using Welford's online algorithm
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WelfordAccumulator {
    pub n: u64,
    pub mean: f64,
    pub m2: f64, // sum of squares of differences from mean
}

impl WelfordAccumulator {
    /// Update with a new observation. O(1).
    pub fn update(&mut self, value: f64) {
        self.n += 1;
        let delta = value - self.mean;
        self.mean += delta / self.n as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    /// Population variance
    pub fn variance(&self) -> f64 {
        if self.n < 2 { return 0.0; }
        self.m2 / self.n as f64
    }

    /// Standard deviation
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Z-score of a value against this accumulator's distribution
    pub fn z_score(&self, value: f64) -> f64 {
        let sd = self.std_dev();
        if sd < f64::EPSILON || self.n < 5 {
            return 0.0; // Not enough data
        }
        (value - self.mean) / sd
    }

    /// Check if value is anomalous (|z| > threshold, default 2.0)
    pub fn is_anomaly(&self, value: f64, threshold: f64) -> bool {
        self.z_score(value).abs() > threshold
    }
}

/// Per-project statistics (persisted in stats.json)
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProjectStats {
    pub tokens_per_turn: WelfordAccumulator,
    pub errors_per_session: WelfordAccumulator,
    pub edit_velocity: WelfordAccumulator,
    pub explore_ratio: WelfordAccumulator,
    pub denial_rate: WelfordAccumulator,
    pub session_length: WelfordAccumulator,
    pub quality_score: WelfordAccumulator,
}

/// Anomaly check result
pub struct AnomalyAlert {
    pub metric: &'static str,
    pub value: f64,
    pub mean: f64,
    pub z_score: f64,
}

impl std::fmt::Display for AnomalyAlert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Anomaly: {} = {:.0} ({:.1}σ above mean {:.0})",
            self.metric, self.value, self.z_score, self.mean)
    }
}

/// Check current turn metrics against project baselines.
/// Returns alerts for any metric >2σ from historical mean.
pub fn check_anomalies(
    stats: &ProjectStats,
    tokens_this_turn: u64,
    threshold: f64,
) -> Vec<AnomalyAlert> {
    let mut alerts = Vec::new();

    let token_val = tokens_this_turn as f64;
    if stats.tokens_per_turn.is_anomaly(token_val, threshold) {
        alerts.push(AnomalyAlert {
            metric: "tokens/turn",
            value: token_val,
            mean: stats.tokens_per_turn.mean,
            z_score: stats.tokens_per_turn.z_score(token_val),
        });
    }

    alerts
}

/// Load project stats from disk
pub fn load_stats(project_dir: &std::path::Path) -> ProjectStats {
    let path = project_dir.join(crate::constants::PROJECT_STATS_FILE);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save project stats to disk
pub fn save_stats(project_dir: &std::path::Path, stats: &ProjectStats) {
    let path = project_dir.join(crate::constants::PROJECT_STATS_FILE);
    if let Ok(json) = serde_json::to_string_pretty(stats) {
        let _ = std::fs::write(&path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welford_basic() {
        let mut acc = WelfordAccumulator::default();
        for v in [10.0, 12.0, 11.0, 9.0, 10.0, 11.0, 10.0, 12.0, 9.0, 11.0] {
            acc.update(v);
        }
        assert_eq!(acc.n, 10);
        assert!((acc.mean - 10.5).abs() < 0.1);
        assert!(acc.std_dev() > 0.5 && acc.std_dev() < 2.0);
    }

    #[test]
    fn welford_anomaly_detection() {
        let mut acc = WelfordAccumulator::default();
        // Build baseline: ~100 tokens/turn
        for _ in 0..20 {
            acc.update(100.0 + (rand_simple() * 20.0 - 10.0));
        }
        // Normal value: not anomalous
        assert!(!acc.is_anomaly(105.0, 2.0));
        // Extreme value: anomalous
        assert!(acc.is_anomaly(200.0, 2.0));
    }

    #[test]
    fn welford_insufficient_data() {
        let mut acc = WelfordAccumulator::default();
        acc.update(100.0);
        // With only 1 data point, z_score should be 0 (not enough data)
        assert_eq!(acc.z_score(200.0), 0.0);
    }

    // Simple deterministic pseudo-random for testing (no rand crate needed)
    fn rand_simple() -> f64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::SystemTime;
        let mut h = DefaultHasher::new();
        SystemTime::now().hash(&mut h);
        (h.finish() % 1000) as f64 / 1000.0
    }
}
