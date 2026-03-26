// ─── Replay — Resume packet generation + working set ranking ─────────────────
//
// Builds compact session grounding for compaction survival: high-salience files,
// dead ends, verified state. Ranks files by recency-frequency-outcome (RFO).
// Source: dream.rs (E2 BuildResumePacket, E3 UpdateWorkingSetRanking)
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use super::{ResumePacket, RankedItem, SuccessfulSequence, ProjectConvention};
use std::collections::BTreeMap;

/// E2: Build compact resume packet from current session state
pub fn build_resume_packet() {
    let state = common::read_session_state();

    // Top 5 files by recency
    let mut files: Vec<(&String, &common::FileReadEntry)> = state.files_read.iter().collect();
    files.sort_by(|a, b| b.1.turn.cmp(&a.1.turn));
    let high_salience: Vec<String> = files.iter().take(5).map(|(k, _)| k.to_string()).collect();

    // V2: Get top playbook
    let sequences: BTreeMap<String, SuccessfulSequence> =
        common::storage::read_json("dream", "sequences").unwrap_or_default();
    let top_playbook = sequences.values()
        .max_by_key(|s| s.occurrences)
        .map(|s| s.actions.join(" → "))
        .unwrap_or_default();

    // V2: Get high-confidence conventions
    let conventions: Vec<ProjectConvention> =
        common::storage::read_json("dream", "conventions").unwrap_or_default();
    let convention_hints: Vec<String> = conventions.iter()
        .filter(|c| c.confidence > 0.6)
        .take(3)
        .map(|c| c.observation.clone())
        .collect();

    let packet = ResumePacket {
        high_salience_files: high_salience,
        last_verified_state: if state.last_build_turn > 0 {
            format!("Last build at turn {}", state.last_build_turn)
        } else {
            "No verification yet".to_string()
        },
        current_issue: state.goal_stack.blocked_on.clone(),
        dead_ends: state.dead_ends.iter().take(3).cloned().collect(),
        probable_next_actions: Vec::new(),
        top_playbook,
        convention_hints,
        verification_debt: state.edits_since_verification,
    };

    let _ = common::storage::write_json("resume_packets", "current", &packet);
}

/// E3: Update working set rankings by recency-frequency-outcome
pub fn update_working_set() {
    let state = common::read_session_state();
    let mut rankings: Vec<RankedItem> = Vec::new();

    for (path, entry) in &state.files_read {
        let recency = if state.turn > 0 {
            1.0 - ((state.turn - entry.turn) as f64 / state.turn as f64)
        } else {
            1.0
        };
        let frequency = 1.0; // Simplified — would need per-file access count
        let outcome = if state.files_edited.contains(path) {
            1.5
        } else {
            1.0
        };
        let score = recency * frequency * outcome;

        rankings.push(RankedItem {
            path: path.clone(),
            score,
            last_turn: entry.turn,
            frequency: 1,
            led_to_progress: state.files_edited.contains(path),
        });
    }

    rankings.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rankings.truncate(20);

    let _ = common::storage::write_json("dream", "working_set", &rankings);
}
