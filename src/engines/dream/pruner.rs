// ─── Engine: Dream — Pruner (effectiveness scoring) ─────────────────────────
//
// Tracks how often each rule fires and correlates with session quality.
// Updated on every deny/allow in the pipeline, scored on session-end.
// ──────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap as HashMap;

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
            let Some(entry) = effectiveness.rules.get_mut(rule_id) else {
                continue;
            };
            entry.quality_sum_when_not += quality_score as u64;
            entry.sessions_not_fired += 1;
        }
    }
}

/// Format top/bottom rules by effectiveness
pub fn format_report(effectiveness: &RuleEffectiveness) -> String {
    let mut scores: Vec<(&String, f64, u32)> = effectiveness
        .rules
        .iter()
        .filter(|(_, s)| s.sessions_fired >= 2) // need at least 2 sessions
        .map(|(id, s)| (id, s.effectiveness(), s.fire_count))
        .collect();

    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut output = String::from("Rule Effectiveness (quality delta when rule fires):\n\n");
    output.push_str(&format!("{:<30} {:>10} {:>8}\n", "Rule", "Delta", "Fires"));
    output.push_str(&format!("{}\n", "-".repeat(50)));

    for (id, delta, fires) in &scores {
        let _indicator = if *delta > 5.0 {
            "+"
        } else if *delta < -5.0 {
            "-"
        } else {
            " "
        };
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

// ═══════════════════════════════════════════════════════════════════════════════
// Part 1b: Artifact caps and decay constants
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum artifacts per type. Enforced on session-start.
pub const MAX_SEQUENCES: usize = 50;
pub const MAX_REPAIR_PATTERNS: usize = 30;
pub const MAX_CONVENTIONS: usize = 20;
pub const MAX_ERROR_CLUSTERS: usize = 50;
pub const MAX_RANKED_ITEMS: usize = 30;

/// Artifacts older than this many turns are candidates for pruning.
pub const STALE_TURN_THRESHOLD: u32 = 200;

/// Prune all dream artifacts to their caps. Called on session-start.
pub fn prune_on_session_start() {
    use super::{ErrorCluster, ProjectConvention, RankedItem, RepairPattern, SuccessfulSequence};
    use crate::common;
    use std::collections::BTreeMap;

    // Sequences: cap at MAX_SEQUENCES, keep highest occurrence
    let mut sequences: BTreeMap<String, SuccessfulSequence> =
        common::storage::read_json("dream", "sequences").unwrap_or_default();
    if sequences.len() > MAX_SEQUENCES {
        let mut sorted: Vec<(String, SuccessfulSequence)> = sequences.into_iter().collect();
        sorted.sort_by(|a, b| b.1.occurrences.cmp(&a.1.occurrences));
        sorted.truncate(MAX_SEQUENCES);
        sequences = sorted.into_iter().collect();
        let _ = common::storage::write_json("dream", "sequences", &sequences);
    }

    // Repair patterns: cap at MAX_REPAIR_PATTERNS, keep highest success_count
    let mut patterns: Vec<RepairPattern> =
        common::storage::read_json("dream", "repair_patterns").unwrap_or_default();
    if patterns.len() > MAX_REPAIR_PATTERNS {
        patterns.sort_by(|a, b| b.success_count.cmp(&a.success_count));
        patterns.truncate(MAX_REPAIR_PATTERNS);
        let _ = common::storage::write_json("dream", "repair_patterns", &patterns);
    }

    // Conventions: cap at MAX_CONVENTIONS, keep highest confidence
    let mut conventions: Vec<ProjectConvention> =
        common::storage::read_json("dream", "conventions").unwrap_or_default();
    if conventions.len() > MAX_CONVENTIONS {
        conventions.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        conventions.truncate(MAX_CONVENTIONS);
        let _ = common::storage::write_json("dream", "conventions", &conventions);
    }

    // Error clusters: cap at MAX_ERROR_CLUSTERS, keep most recent
    let mut clusters: Vec<ErrorCluster> =
        common::storage::read_json("dream", "error_clusters").unwrap_or_default();
    if clusters.len() > MAX_ERROR_CLUSTERS {
        clusters.sort_by(|a, b| b.last_turn.cmp(&a.last_turn));
        clusters.truncate(MAX_ERROR_CLUSTERS);
        let _ = common::storage::write_json("dream", "error_clusters", &clusters);
    }

    // Ranked items: cap at MAX_RANKED_ITEMS
    let mut ranked: Vec<RankedItem> =
        common::storage::read_json("dream", "ranked_items").unwrap_or_default();
    if ranked.len() > MAX_RANKED_ITEMS {
        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ranked.truncate(MAX_RANKED_ITEMS);
        let _ = common::storage::write_json("dream", "ranked_items", &ranked);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Part 2: Dream tasks — E5 LearnEffectiveness, E10 ScoreArtifacts
// ═══════════════════════════════════════════════════════════════════════════════

use super::{InterventionScores, ProjectConvention, RepairPattern, SuccessfulSequence};

/// E5: Learn which interventions preceded progress
pub fn learn_effectiveness() {
    use crate::common;

    let events = common::storage::read_last_events(200);
    if events.len() < 10 {
        return;
    }

    let mut scores: InterventionScores =
        common::storage::read_json("dream", "intervention_scores").unwrap_or_default();

    let mut last_advisory_category: Option<String> = None;
    let mut last_advisory_turn: u32 = 0;

    for raw in &events {
        let entry: serde_json::Value = match serde_json::from_slice(raw) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match event_type {
            // Track advisory emissions
            t if t.contains("advisory") || t.contains("injection") => {
                let category = entry
                    .get("detail")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .split_whitespace()
                    .next()
                    .unwrap_or("unknown")
                    .to_string();
                last_advisory_category = Some(category);
                last_advisory_turn = entry.get("turn").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            }
            // Milestone within 5 turns of advisory = positive signal
            "milestone" => {
                let turn = entry.get("turn").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                if let Some(ref cat) = last_advisory_category
                    && turn > 0
                    && turn.saturating_sub(last_advisory_turn) <= 5
                {
                    let score = scores.scores.entry(cat.clone()).or_insert(0.5);
                    let old_score = *score;
                    *score = (old_score + crate::config::DREAM_LEARNING_RATE).min(1.0);
                    // Log score change to session notes
                    let note = serde_json::json!({
                        "type": "dream_score_update",
                        "category": cat,
                        "old": old_score,
                        "new": *score,
                        "reason": "milestone_within_5_turns"
                    });
                    let path = common::project_dir().join("session-notes.jsonl");
                    let _ = std::fs::OpenOptions::new()
                        .create(true).append(true).open(&path)
                        .and_then(|mut f| {
                            use std::io::Write;
                            writeln!(f, "{}", note)
                        });
                }
            }
            _ => {}
        }
    }

    let _ = common::storage::write_json("dream", "intervention_scores", &scores);
}

/// E10: Score and prune dream artifacts by usefulness
pub fn score_artifacts() {
    use crate::common;
    use std::collections::BTreeMap;

    // Prune sequences with low occurrence
    let mut sequences: BTreeMap<String, SuccessfulSequence> =
        common::storage::read_json("dream", "sequences").unwrap_or_default();
    let before = sequences.len();
    sequences.retain(|_, s| s.occurrences >= 2);
    if sequences.len() < before {
        let _ = common::storage::write_json("dream", "sequences", &sequences);
    }

    // Prune repair patterns with low success count
    let mut patterns: Vec<RepairPattern> =
        common::storage::read_json("dream", "repair_patterns").unwrap_or_default();
    let before = patterns.len();
    patterns.retain(|p| p.success_count >= 1);
    if patterns.len() < before {
        let _ = common::storage::write_json("dream", "repair_patterns", &patterns);
    }

    // Decay convention confidence for stale conventions
    let mut conventions: Vec<ProjectConvention> =
        common::storage::read_json("dream", "conventions").unwrap_or_default();
    let state = common::read_session_state();
    for conv in &mut conventions {
        let staleness = state.turn.saturating_sub(conv.last_updated_turn);
        if staleness > 50 {
            conv.confidence *= 0.95; // Slow decay
        }
    }
    conventions.retain(|c| c.confidence > 0.1);
    let _ = common::storage::write_json("dream", "conventions", &conventions);

    // Decay intervention scores
    let mut scores: InterventionScores =
        common::storage::read_json("dream", "intervention_scores").unwrap_or_default();
    for score in scores.scores.values_mut() {
        // Regress toward 0.5 (neutral) slowly
        *score = *score * 0.98 + 0.5 * 0.02;
    }
    let _ = common::storage::write_json("dream", "intervention_scores", &scores);
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
        assert!(
            score.effectiveness() > 0.0,
            "rule should show positive effectiveness"
        );
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
