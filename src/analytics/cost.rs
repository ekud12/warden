// ─── analytics::cost — token cost categorization and tracking ────────────────
//
// Categorizes token usage by activity type:
//   - Explore: reads, searches, file browsing
//   - Implement: edits, builds, tests
//   - Waste: denied operations, re-reads, truncated outputs
//   - Saved: tokens prevented by Warden interventions
//
// Cost estimated at configurable $/1K tokens rate.
// ──────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

/// Token cost breakdown per session
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CostBreakdown {
    pub explore_tokens: u64,
    pub implement_tokens: u64,
    pub waste_tokens: u64,
    pub saved_tokens: u64,
    pub total_tokens: u64,
}

impl CostBreakdown {
    /// Categorize a tool call's token usage
    pub fn record(&mut self, tool_name: &str, tokens: u64, was_denied: bool, was_dedup: bool) {
        self.total_tokens += tokens;

        if was_denied || was_dedup {
            self.waste_tokens += tokens;
        } else {
            match tool_name {
                "Read" | "Glob" | "Grep" | "WebSearch" | "WebFetch" | "Agent" => {
                    self.explore_tokens += tokens;
                }
                "Edit" | "Write" | "MultiEdit" | "Bash" => {
                    self.implement_tokens += tokens;
                }
                _ if tool_name.starts_with("mcp__") => {
                    self.explore_tokens += tokens;
                }
                _ => {
                    self.implement_tokens += tokens;
                }
            }
        }
    }

    /// Record tokens saved by Warden intervention
    pub fn record_saved(&mut self, tokens: u64) {
        self.saved_tokens += tokens;
    }

    /// Estimate cost at given rate ($/1K tokens)
    pub fn estimate_cost(&self, rate_per_1k: f64) -> f64 {
        self.total_tokens as f64 / 1000.0 * rate_per_1k
    }

    /// Estimate savings at given rate
    pub fn estimate_savings(&self, rate_per_1k: f64) -> f64 {
        self.saved_tokens as f64 / 1000.0 * rate_per_1k
    }

    /// Format as human-readable summary
    pub fn format(&self, rate_per_1k: f64) -> String {
        let cost = self.estimate_cost(rate_per_1k);
        let savings = self.estimate_savings(rate_per_1k);
        let waste_pct = if self.total_tokens > 0 {
            self.waste_tokens * 100 / self.total_tokens
        } else { 0 };

        format!(
            "Tokens: {}K total ({}K explore, {}K implement, {}K waste [{}%])\n\
             Saved: {}K tokens (~${:.2})\n\
             Estimated cost: ~${:.2}",
            self.total_tokens / 1000,
            self.explore_tokens / 1000,
            self.implement_tokens / 1000,
            self.waste_tokens / 1000,
            waste_pct,
            self.saved_tokens / 1000,
            savings,
            cost,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_categorization() {
        let mut cost = CostBreakdown::default();
        cost.record("Read", 5000, false, false);
        cost.record("Edit", 3000, false, false);
        cost.record("Bash", 2000, true, false); // denied
        cost.record_saved(1000);

        assert_eq!(cost.explore_tokens, 5000);
        assert_eq!(cost.implement_tokens, 3000);
        assert_eq!(cost.waste_tokens, 2000);
        assert_eq!(cost.saved_tokens, 1000);
        assert_eq!(cost.total_tokens, 10000);
    }

    #[test]
    fn cost_estimation() {
        let cost = CostBreakdown {
            total_tokens: 100_000,
            saved_tokens: 20_000,
            ..Default::default()
        };
        // At $0.01 per 1K tokens
        assert!((cost.estimate_cost(0.01) - 1.0).abs() < 0.01);
        assert!((cost.estimate_savings(0.01) - 0.20).abs() < 0.01);
    }
}
