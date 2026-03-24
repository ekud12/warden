// ─── analytics::effectiveness — per-rule impact scoring ──────────────────────
//
// Tracks how often each rule fires and correlates with session quality.
// Updated on every deny/allow in the pipeline, scored on session-end.
// ──────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-rule effectiveness data
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RuleScore {
    /// Total times this rule fired (denied a command)
    pub fire_count: u32,
    /// Sum of session quality scores when this rule fired
    pub quality_sum_when_fired: u64,
    /// Number of sessions where this rule fired
    pub sessions_fired: u32,
    /// Sum of session quality scores when this rule didn't fire
    pub quality_sum_when_not: u64,
    /// Number of sessions where this rule didn't fire
    pub sessions_not_fired: u32,
}

impl RuleScore {
    /// Average quality when rule fires vs doesn't
    pub fn effectiveness(&self) -> f64 {
        let avg_fired = if self.sessions_fired > 0 {
            self.quality_sum_when_fired as f64 / self.sessions_fired as f64
        } else {
            50.0
        };
        let avg_not = if self.sessions_not_fired > 0 {
            self.quality_sum_when_not as f64 / self.sessions_not_fired as f64
        } else {
            50.0
        };
        // Positive = rule improves quality. Negative = rule hurts.
        avg_fired - avg_not
    }
}

/// All rule scores for a project
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RuleEffectiveness {
    pub rules: HashMap<String, RuleScore>,
}

/// Record a rule firing during the session
pub fn record_fire(effectiveness: &mut RuleEffectiveness, rule_id: &str) {
    let entry = effectiveness.rules.entry(rule_id.to_string()).or_default();
    entry.fire_count += 1;
}

/// Update effectiveness scores at session end
pub fn update_session_end(
    effectiveness: &mut RuleEffectiveness,
    fired_rules: &[String],
    quality_score: u32,
) {
    // Rules that fired this session
    for rule_id in fired_rules {
        let entry = effectiveness.rules.entry(rule_id.clone()).or_default();
        entry.quality_sum_when_fired += quality_score as u64;
        entry.sessions_fired += 1;
    }

    // Rules that exist but didn't fire
    let fired_set: std::collections::HashSet<&String> = fired_rules.iter().collect();
    let all_keys: Vec<String> = effectiveness.rules.keys().cloned().collect();
    for rule_id in &all_keys {
        if !fired_set.contains(rule_id) {
            let Some(entry) = effectiveness.rules.get_mut(rule_id) else { continue; };
            entry.quality_sum_when_not += quality_score as u64;
            entry.sessions_not_fired += 1;
        }
    }
}

/// Format top/bottom rules by effectiveness
pub fn format_report(effectiveness: &RuleEffectiveness) -> String {
    let mut scores: Vec<(&String, f64, u32)> = effectiveness.rules.iter()
        .filter(|(_, s)| s.sessions_fired >= 2) // need at least 2 sessions
        .map(|(id, s)| (id, s.effectiveness(), s.fire_count))
        .collect();

    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut output = String::from("Rule Effectiveness (quality delta when rule fires):\n\n");
    output.push_str(&format!("{:<30} {:>10} {:>8}\n", "Rule", "Delta", "Fires"));
    output.push_str(&format!("{}\n", "-".repeat(50)));

    for (id, delta, fires) in &scores {
        let _indicator = if *delta > 5.0 { "+" } else if *delta < -5.0 { "-" } else { " " };
        output.push_str(&format!("{:<30} {:>+9.1} {:>8}\n", id, delta, fires));
    }

    output
}

/// Load from disk
pub fn load(project_dir: &std::path::Path) -> RuleEffectiveness {
    let path = project_dir.join("rule-effectiveness.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save to disk
pub fn save(project_dir: &std::path::Path, data: &RuleEffectiveness) {
    let path = project_dir.join("rule-effectiveness.json");
    if let Ok(json) = serde_json::to_string_pretty(data) {
        let _ = std::fs::write(&path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effectiveness_positive() {
        let mut eff = RuleEffectiveness::default();
        // Rule fires in high-quality sessions
        record_fire(&mut eff, "safety.rm-rf");
        update_session_end(&mut eff, &["safety.rm-rf".to_string()], 80);
        update_session_end(&mut eff, &["safety.rm-rf".to_string()], 75);
        update_session_end(&mut eff, &[], 40); // low quality when not fired

        let score = eff.rules.get("safety.rm-rf").unwrap();
        assert!(score.effectiveness() > 0.0, "rule should show positive effectiveness");
    }

    #[test]
    fn report_format() {
        let mut eff = RuleEffectiveness::default();
        for _ in 0..3 {
            record_fire(&mut eff, "sub.grep");
            update_session_end(&mut eff, &["sub.grep".to_string()], 70);
        }
        let report = format_report(&eff);
        assert!(report.contains("sub.grep"));
    }
}
