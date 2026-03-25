// ─── scorecard — session quality measurement ─────────────────────────────────
//
// Computes measurable metrics from session traces. Used by:
//   - `warden scorecard` command (developer tool)
//   - Regression tests against golden traces
//   - Dream state effectiveness learning
//
// Metrics follow the self_improvement_loop.md scorecard design:
//   Safety: denials fired, false positives
//   Efficiency: tokens saved, compression ratio, retries
//   Focus: loops, advisory follow-through, milestone spacing
//   UX: injection count, repeated advisories, silent ratio
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;

/// Measured session quality metrics
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Scorecard {
    // Safety
    pub denials_fired: u32,
    pub false_positives: u32,

    // Efficiency
    pub tokens_saved: u64,
    pub compression_events: u32,
    pub retries_after_compression: u32,

    // Focus
    pub loop_recurrences: u32,
    pub advisories_emitted: u32,
    pub milestones_reached: u32,
    pub advisory_follow_through: f64,
    pub milestone_spacing: f64,

    // UX
    pub total_injections: u32,
    pub repeated_advisories: u32,
    pub total_turns: u32,
    pub silent_turns: u32,
    pub silent_ratio: f64,

    // Overall
    pub overall_score: u32,
}

/// Compute scorecard from redb events for the current project
pub fn compute_from_redb() -> Scorecard {
    let events = common::storage::read_last_events(1000);
    compute_from_events(&events)
}

/// Compute scorecard from raw event bytes
pub fn compute_from_events(events: &[Vec<u8>]) -> Scorecard {
    let mut sc = Scorecard::default();

    let mut last_denial_cmd: Option<String> = None;
    let mut last_denial_turn: u32 = 0;
    let mut last_advisory_turn: u32 = 0;
    let mut advisory_categories: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut turns_with_injection: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut milestone_turns: Vec<u32> = Vec::new();
    let mut max_turn: u32 = 0;

    for raw in events {
        let entry: serde_json::Value = match serde_json::from_slice(raw) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let turn = entry.get("turn").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let detail = entry.get("detail").and_then(|v| v.as_str()).unwrap_or("");

        if turn > max_turn { max_turn = turn; }

        match event_type {
            "deny" | "denial" => {
                sc.denials_fired += 1;
                let cmd = detail.to_string();

                // False positive detection: same command retried within 2 turns
                if let Some(ref prev_cmd) = last_denial_cmd
                    && *prev_cmd == cmd && turn.saturating_sub(last_denial_turn) <= 2 {
                        sc.false_positives += 1;
                    }
                last_denial_cmd = Some(cmd);
                last_denial_turn = turn;
            }
            "milestone" => {
                sc.milestones_reached += 1;
                milestone_turns.push(turn);

                // Advisory follow-through: milestone within 5 turns of advisory
                if turn.saturating_sub(last_advisory_turn) <= 5 && last_advisory_turn > 0 {
                    sc.advisory_follow_through += 1.0;
                }
            }
            t if t.contains("advisory") || t.contains("injection") => {
                sc.advisories_emitted += 1;
                sc.total_injections += 1;
                last_advisory_turn = turn;
                turns_with_injection.insert(turn);

                // Track category for repetition detection
                let category = detail.split_whitespace().next().unwrap_or("unknown").to_string();
                *advisory_categories.entry(category).or_insert(0) += 1;
            }
            "truncation" | "compression" => {
                sc.compression_events += 1;
            }
            "edit" | "error" | "read" => {
                // Track turns for silent ratio
            }
            _ => {}
        }
    }

    // Compute derived metrics
    sc.total_turns = max_turn;

    if sc.total_turns > 0 {
        sc.silent_turns = sc.total_turns - turns_with_injection.len() as u32;
        sc.silent_ratio = sc.silent_turns as f64 / sc.total_turns as f64;
    }

    // Advisory follow-through rate
    if sc.advisories_emitted > 0 {
        sc.advisory_follow_through /= sc.advisories_emitted as f64;
    }

    // Milestone spacing
    if milestone_turns.len() >= 2 {
        let mut spacings: Vec<u32> = Vec::new();
        for w in milestone_turns.windows(2) {
            spacings.push(w[1] - w[0]);
        }
        sc.milestone_spacing = spacings.iter().sum::<u32>() as f64 / spacings.len() as f64;
    }

    // Repeated advisories: categories that fired 3+ times
    sc.repeated_advisories = advisory_categories.values().filter(|&&c| c >= 3).count() as u32;

    // Read session state for token savings
    let state = common::read_session_state();
    sc.tokens_saved = state.estimated_tokens_saved;
    sc.retries_after_compression = state.retries_after_truncation;
    sc.loop_recurrences = state.dead_ends.len() as u32;

    // Overall score (balanced, 0-100)
    sc.overall_score = compute_overall(&sc);

    sc
}

/// Compute balanced overall score
fn compute_overall(sc: &Scorecard) -> u32 {
    use crate::config;
    let mut score: i32 = config::SCORECARD_BASELINE;

    score += if sc.false_positives == 0 { config::SCORECARD_SAFETY_BONUS } else { -(sc.false_positives as i32 * config::SCORECARD_FP_PENALTY) };
    score += if sc.tokens_saved > 10_000 { config::SCORECARD_EFFICIENCY_HIGH } else if sc.tokens_saved > 1_000 { config::SCORECARD_EFFICIENCY_MED } else { 0 };

    // Focus: +15 for milestones, -5 per loop
    score += (sc.milestones_reached as i32 * config::SCORECARD_MILESTONE_PER).min(15);
    score -= (sc.loop_recurrences as i32 * config::SCORECARD_LOOP_PENALTY).min(15);

    score += if sc.silent_ratio > 0.7 { config::SCORECARD_SILENCE_BONUS_HIGH } else if sc.silent_ratio > 0.5 { config::SCORECARD_SILENCE_BONUS_MED } else { 0 };
    score -= (sc.repeated_advisories as i32 * config::SCORECARD_REPEAT_PENALTY).min(10);

    score.clamp(0, 100) as u32
}

/// Format scorecard for display
pub fn format_scorecard(sc: &Scorecard) -> String {
    let mut out = String::new();
    out.push_str(&format!("Safety:     {} denials, {} false positives\n", sc.denials_fired, sc.false_positives));
    out.push_str(&format!("Efficiency: {}K tokens saved, {} compression events, {} retries\n",
        sc.tokens_saved / 1000, sc.compression_events, sc.retries_after_compression));
    out.push_str(&format!("Focus:      {} loops, {:.0}% advisory follow-through, {:.1} turns/milestone\n",
        sc.loop_recurrences, sc.advisory_follow_through * 100.0, sc.milestone_spacing));
    out.push_str(&format!("UX:         {} injections, {} repeated, {:.0}% silent turns\n",
        sc.total_injections, sc.repeated_advisories, sc.silent_ratio * 100.0));
    out.push_str(&format!("\nOverall: {}/100\n", sc.overall_score));
    out
}

/// Entry point for `warden scorecard` command
pub fn run() {
    if !common::storage::is_available() {
        // Try to open redb for current project
        let proj = common::project_dir();
        common::storage::open_db(&proj);
    }

    let sc = compute_from_redb();
    print!("{}", format_scorecard(&sc));
}
