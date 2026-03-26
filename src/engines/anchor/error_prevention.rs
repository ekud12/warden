// ─── Engine: Anchor — Bayesian error pattern prediction ───────────────────────
//
// Tracks patterns that historically lead to errors:
//   P(error | N edits without build)
//   P(error | cross-module edits)
//
// Uses conjugate Beta prior — updated incrementally on session-end.
// Runtime: checks current pattern against priors after each edit.
// ──────────────────────────────────────────────────────────────────────────────

use crate::engines::signal::{Signal, SignalCategory};
use serde::{Deserialize, Serialize};

/// Beta distribution parameters for Bayesian probability estimation
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BetaPrior {
    pub alpha: f64, // successes (error occurred)
    pub beta: f64,  // failures (no error)
}

impl Default for BetaPrior {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            beta: 1.0,
        } // Uniform prior
    }
}

impl BetaPrior {
    /// Posterior mean: P(error | data)
    pub fn probability(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    /// Update with observation: error_occurred = true/false
    pub fn update(&mut self, error_occurred: bool) {
        if error_occurred {
            self.alpha += 1.0;
        } else {
            self.beta += 1.0;
        }
    }

    /// Whether we have enough data to make predictions (>5 observations)
    pub fn has_enough_data(&self) -> bool {
        (self.alpha + self.beta) > 7.0 // 5 observations + 2 prior
    }
}

/// Error patterns tracked per project
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ErrorPriors {
    /// P(error | 3+ edits without build)
    pub edits_without_build: BetaPrior,
    /// P(error | editing files in 2+ different directories)
    pub cross_module_edits: BetaPrior,
    /// P(error | 5+ turns since last test)
    pub turns_without_test: BetaPrior,
}

/// Check current session patterns against priors.
/// Returns advisory message if a high-risk pattern is detected.
pub fn check_patterns(
    priors: &ErrorPriors,
    edits_since_build: u32,
    edited_dirs: u32,
    turns_since_test: u32,
    threshold: f64,
) -> Option<String> {
    let mut warnings = Vec::new();

    if edits_since_build >= 3 && priors.edits_without_build.has_enough_data() {
        let p = priors.edits_without_build.probability();
        if p > threshold {
            warnings.push(format!(
                "{} edits without building — historical error rate: {:.0}%. Consider running build/test.",
                edits_since_build, p * 100.0
            ));
        }
    }

    if edited_dirs >= 2 && priors.cross_module_edits.has_enough_data() {
        let p = priors.cross_module_edits.probability();
        if p > threshold {
            warnings.push(format!(
                "Editing across {} directories — historical error rate: {:.0}%.",
                edited_dirs,
                p * 100.0
            ));
        }
    }

    if turns_since_test >= 5 && priors.turns_without_test.has_enough_data() {
        let p = priors.turns_without_test.probability();
        if p > threshold {
            warnings.push(format!(
                "{} turns without testing — historical error rate: {:.0}%.",
                turns_since_test,
                p * 100.0
            ));
        }
    }

    if warnings.is_empty() {
        None
    } else {
        Some(warnings.join(" "))
    }
}

/// Load error priors from project stats
pub fn load_priors(project_dir: &std::path::Path) -> ErrorPriors {
    let path = project_dir.join("error-priors.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save error priors to project stats
pub fn save_priors(project_dir: &std::path::Path, priors: &ErrorPriors) {
    let path = project_dir.join("error-priors.json");
    if let Ok(json) = serde_json::to_string_pretty(priors) {
        let _ = std::fs::write(&path, json);
    }
}

pub fn check_patterns_signal(
    priors: &ErrorPriors,
    edits_since_build: u32,
    edited_dirs: u32,
    turns_since_test: u32,
    threshold: f64,
) -> Option<Signal> {
    check_patterns(priors, edits_since_build, edited_dirs, turns_since_test, threshold).map(|msg| Signal::advisory(SignalCategory::Recovery, 0.6, msg, "error_prevention"))
}

/// Update priors based on session outcome
pub fn update_from_session(
    priors: &mut ErrorPriors,
    had_edits_without_build: bool,
    had_cross_module: bool,
    had_turns_without_test: bool,
    had_errors: bool,
) {
    if had_edits_without_build {
        priors.edits_without_build.update(had_errors);
    }
    if had_cross_module {
        priors.cross_module_edits.update(had_errors);
    }
    if had_turns_without_test {
        priors.turns_without_test.update(had_errors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta_prior_uniform_start() {
        let prior = BetaPrior::default();
        assert!((prior.probability() - 0.5).abs() < 0.01);
    }

    #[test]
    fn beta_prior_updates() {
        let mut prior = BetaPrior::default();
        for _ in 0..8 {
            prior.update(true);
        } // 8 errors
        for _ in 0..2 {
            prior.update(false);
        } // 2 no-errors
        assert!(prior.probability() > 0.7);
        assert!(prior.has_enough_data());
    }

    #[test]
    fn check_patterns_high_risk() {
        let mut priors = ErrorPriors::default();
        for _ in 0..10 {
            priors.edits_without_build.update(true);
        }
        for _ in 0..2 {
            priors.edits_without_build.update(false);
        }

        let result = check_patterns(&priors, 3, 1, 2, 0.5);
        assert!(result.is_some());
        assert!(result.unwrap().contains("edits without building"));
    }

    #[test]
    fn check_patterns_low_risk() {
        let priors = ErrorPriors::default(); // uniform prior, not enough data
        let result = check_patterns(&priors, 3, 1, 2, 0.5);
        assert!(result.is_none(), "insufficient data should not trigger");
    }
}
