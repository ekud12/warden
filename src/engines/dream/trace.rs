// ─── Trace — Successful sequence + repair pattern learning ───────────────────
//
// Mines 3-gram action sequences that precede milestones. Maps error signatures
// to successful remediations. Produces DreamPlaybook and RepairPattern artifacts.
// Source: dream.rs (E7 LearnSequences, E8 LearnRepairPatterns)
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use super::{SuccessfulSequence, RepairPattern, text_similarity};
use std::collections::BTreeMap;

/// E7: Mine successful action sequences from events
pub fn learn_sequences() {
    let events = common::storage::read_last_events(200);
    if events.len() < 20 {
        return;
    }

    // Extract action types from events
    let mut actions: Vec<(String, u32)> = Vec::new();
    for raw in &events {
        let entry: serde_json::Value = match serde_json::from_slice(raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let turn = entry.get("turn").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if !event_type.is_empty() {
            actions.push((event_type, turn));
        }
    }

    // Mine 3-grams that precede milestones
    let mut sequences: BTreeMap<String, SuccessfulSequence> = common::storage::read_json("dream", "sequences").unwrap_or_default();

    for window in actions.windows(4) {
        if window[3].0 == "milestone" {
            let key = format!("{}→{}→{}", window[0].0, window[1].0, window[2].0);
            let seq = sequences.entry(key).or_insert_with(|| SuccessfulSequence {
                actions: vec![window[0].0.clone(), window[1].0.clone(), window[2].0.clone()],
                led_to_milestone: true,
                occurrences: 0,
                last_seen_turn: 0,
            });
            seq.occurrences += 1;
            seq.last_seen_turn = window[3].1;
        }
    }

    // Keep only sequences seen 2+ times
    sequences.retain(|_, s| s.occurrences >= 2);
    let _ = common::storage::write_json("dream", "sequences", &sequences);
}

/// E8: Learn repair patterns from error → successful fix sequences
pub fn learn_repair_patterns() {
    let events = common::storage::read_last_events(200);
    if events.len() < 10 {
        return;
    }

    let mut patterns: Vec<RepairPattern> = common::storage::read_json("dream", "repair_patterns").unwrap_or_default();

    let mut last_error: Option<(String, Vec<String>, u32)> = None; // (signature, files, turn)

    for raw in &events {
        let entry: serde_json::Value = match serde_json::from_slice(raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let turn = entry.get("turn").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let detail = entry.get("detail").and_then(|v| v.as_str()).unwrap_or("").to_string();

        match event_type {
            "error" => {
                let file = detail.split_whitespace()
                    .find(|w| w.contains('/') || w.contains('\\') || w.contains('.'))
                    .unwrap_or("unknown").to_string();
                let sig = detail.chars().take(60).collect::<String>();
                last_error = Some((sig, vec![file], turn));
            }
            "milestone" | "build_success" => {
                if let Some((sig, files, error_turn)) = last_error.take() {
                    if turn.saturating_sub(error_turn) <= 10 {
                        // Find or create repair pattern
                        if let Some(existing) = patterns.iter_mut().find(|p| text_similarity(&p.error_signature, &sig) > 0.6) {
                            existing.success_count += 1;
                            existing.last_seen_turn = turn;
                        } else {
                            patterns.push(RepairPattern {
                                error_signature: sig,
                                affected_files: files,
                                commands_that_helped: Vec::new(),
                                verification_step: "build/test".to_string(),
                                success_count: 1,
                                last_seen_turn: turn,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    patterns.truncate(50);
    let _ = common::storage::write_json("dream", "repair_patterns", &patterns);
}
