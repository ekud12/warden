// ─── dream — background learning during daemon idle time ─────────────────────
//
// The dream worker runs in a low-priority thread during daemon idle periods.
// It consolidates raw events into higher-level knowledge, builds resume packets,
// learns intervention effectiveness, and ranks the working set.
//
// Design principles:
//   - Stops immediately when activity resumes (check before + after each batch)
//   - Writes only to redb (dream + resume_packets tables)
//   - Never injects context directly — produces data consumed on demand
//   - Falls back to heuristics if semantic model unavailable
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A unit of dream work to process
pub struct DreamBatch {
    pub kind: DreamTask,
    pub project_dir: PathBuf,
}

/// Types of dream processing tasks
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DreamTask {
    /// E1: Consolidate raw events into higher-level facts
    ConsolidateEvents,
    /// E2: Build compact session resume packet
    BuildResumePacket,
    /// E3: Update file/directory rankings by recency-frequency-outcome
    UpdateWorkingSetRanking,
    /// E4: Cluster repeated errors into durable knowledge
    ClusterErrors,
    /// E5: Learn which interventions preceded progress
    LearnEffectiveness,
    /// E6: Build durable dead-end memory
    BuildDeadEndMemory,
}

/// Compact session grounding — built during idle, consumed on demand
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ResumePacket {
    /// Top files by recency-frequency-outcome score
    pub high_salience_files: Vec<String>,
    /// Last verified state description
    pub last_verified_state: String,
    /// Current issue from goal stack
    pub current_issue: String,
    /// Top dead ends to avoid
    pub dead_ends: Vec<String>,
    /// Most probable next actions from markov
    pub probable_next_actions: Vec<String>,
}

/// Per-advisory-category effectiveness score
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct InterventionScores {
    pub scores: BTreeMap<String, f64>,
}

/// Recency-frequency-outcome ranking entry
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RankedItem {
    pub path: String,
    pub score: f64,
    pub last_turn: u32,
    pub frequency: u32,
    pub led_to_progress: bool,
}

/// Error cluster — durable compressed knowledge from repeated errors
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorCluster {
    pub file: String,
    pub error_stem: String,
    pub count: u32,
    pub first_turn: u32,
    pub last_turn: u32,
}

const TASK_ORDER: &[DreamTask] = &[
    DreamTask::LearnEffectiveness,    // highest value — feeds back into injection budget
    DreamTask::BuildResumePacket,     // second — ready for session resume
    DreamTask::ClusterErrors,         // third — compress noisy error history
    DreamTask::UpdateWorkingSetRanking,
    DreamTask::BuildDeadEndMemory,
    DreamTask::ConsolidateEvents,     // lowest priority — general housekeeping
];

/// Cycle counter for round-robin task selection
static DREAM_CYCLE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Get the next dream batch to process
pub fn next_batch() -> Option<DreamBatch> {
    let project_dir = common::project_dir();
    if !common::storage::is_available() {
        return None;
    }

    let cycle = DREAM_CYCLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let task_idx = (cycle as usize) % TASK_ORDER.len();

    Some(DreamBatch {
        kind: TASK_ORDER[task_idx],
        project_dir,
    })
}

/// Process a single dream batch
pub fn process_batch(batch: DreamBatch) {
    match batch.kind {
        DreamTask::LearnEffectiveness => learn_effectiveness(),
        DreamTask::BuildResumePacket => build_resume_packet(),
        DreamTask::ClusterErrors => cluster_errors(),
        DreamTask::UpdateWorkingSetRanking => update_working_set(),
        DreamTask::BuildDeadEndMemory => build_dead_end_memory(),
        DreamTask::ConsolidateEvents => consolidate_events(),
    }
}

/// E5: Learn which interventions preceded progress
fn learn_effectiveness() {
    let events = common::storage::read_last_events(200);
    if events.len() < 10 { return; }

    let mut scores: InterventionScores = common::storage::read_json("dream", "intervention_scores")
        .unwrap_or_default();

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
                let category = entry.get("detail")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .split_whitespace().next()
                    .unwrap_or("unknown")
                    .to_string();
                last_advisory_category = Some(category);
                last_advisory_turn = entry.get("turn")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
            }
            // Milestone within 5 turns of advisory = positive signal
            "milestone" => {
                let turn = entry.get("turn")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                if let Some(ref cat) = last_advisory_category
                    && turn > 0 && turn.saturating_sub(last_advisory_turn) <= 5 {
                        let score = scores.scores.entry(cat.clone()).or_insert(0.5);
                        *score = (*score + crate::config::DREAM_LEARNING_RATE).min(1.0);
                    }
            }
            _ => {}
        }
    }

    let _ = common::storage::write_json("dream", "intervention_scores", &scores);
}

/// E2: Build compact resume packet from current session state
fn build_resume_packet() {
    let state = common::read_session_state();

    // Top 5 files by recency
    let mut files: Vec<(&String, &common::FileReadEntry)> = state.files_read.iter().collect();
    files.sort_by(|a, b| b.1.turn.cmp(&a.1.turn));
    let high_salience: Vec<String> = files.iter().take(5).map(|(k, _)| k.to_string()).collect();

    let packet = ResumePacket {
        high_salience_files: high_salience,
        last_verified_state: if state.last_build_turn > 0 {
            format!("Last build at turn {}", state.last_build_turn)
        } else {
            "No verification yet".to_string()
        },
        current_issue: state.goal_stack.blocked_on.clone(),
        dead_ends: state.dead_ends.iter().take(3).cloned().collect(),
        probable_next_actions: Vec::new(), // populated by markov in future
    };

    let _ = common::storage::write_json("resume_packets", "current", &packet);
}

/// E4: Cluster repeated errors from event log
fn cluster_errors() {
    let events = common::storage::read_last_events(100);
    let mut clusters: BTreeMap<String, ErrorCluster> = BTreeMap::new();

    for raw in &events {
        let entry: serde_json::Value = match serde_json::from_slice(raw) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if entry.get("type").and_then(|v| v.as_str()) != Some("error") { continue; }

        let detail = entry.get("detail").and_then(|v| v.as_str()).unwrap_or("");
        let turn = entry.get("turn").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

        // Extract file from error detail (heuristic: first path-like token)
        let file = detail.split_whitespace()
            .find(|w| w.contains('/') || w.contains('\\') || w.contains('.'))
            .unwrap_or("unknown")
            .to_string();

        let stem = detail.chars().take(40).collect::<String>();
        let key = format!("{}:{}", file, &stem);

        let cluster = clusters.entry(key).or_insert_with(|| ErrorCluster {
            file: file.clone(),
            error_stem: stem,
            count: 0,
            first_turn: turn,
            last_turn: turn,
        });
        cluster.count += 1;
        cluster.last_turn = turn;
    }

    let significant: Vec<ErrorCluster> = clusters.into_values()
        .filter(|c| c.count >= 2)
        .collect();

    if !significant.is_empty() {
        let _ = common::storage::write_json("dream", "error_clusters", &significant);
    }
}

/// E3: Update working set rankings by recency-frequency-outcome
fn update_working_set() {
    let state = common::read_session_state();
    let mut rankings: Vec<RankedItem> = Vec::new();

    for (path, entry) in &state.files_read {
        let recency = if state.turn > 0 {
            1.0 - ((state.turn - entry.turn) as f64 / state.turn as f64)
        } else { 1.0 };
        let frequency = 1.0; // Simplified — would need per-file access count
        let outcome = if state.files_edited.contains(path) { 1.5 } else { 1.0 };
        let score = recency * frequency * outcome;

        rankings.push(RankedItem {
            path: path.clone(),
            score,
            last_turn: entry.turn,
            frequency: 1,
            led_to_progress: state.files_edited.contains(path),
        });
    }

    rankings.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    rankings.truncate(20);

    let _ = common::storage::write_json("dream", "working_set", &rankings);
}

/// E6: Build dead-end memory from session state
fn build_dead_end_memory() {
    let state = common::read_session_state();
    if state.dead_ends.is_empty() && state.failed_commands.is_empty() { return; }

    let mut memory: Vec<String> = Vec::new();
    memory.extend(state.dead_ends.iter().cloned());
    for (cmd, count) in &state.failed_commands {
        if *count >= 2 {
            memory.push(format!("cmd:{} (failed {} times)", cmd, count));
        }
    }
    memory.truncate(20);

    let _ = common::storage::write_json("dream", "dead_ends", &memory);
}

/// E1: General event consolidation (housekeeping)
fn consolidate_events() {
    // Future: aggregate event counts, prune old events, build summaries
    // For now, just verify events table health
    let count = common::storage::read_last_events(1).len();
    common::log("dream", &format!("Event store health check: {} events accessible", count));
}

/// Read the current resume packet (for MCP or post-compaction injection)
pub fn get_resume_packet() -> Option<ResumePacket> {
    common::storage::read_json("resume_packets", "current")
}

/// Read intervention effectiveness scores (for injection budget utility adjustment)
pub fn get_intervention_scores() -> InterventionScores {
    common::storage::read_json("dream", "intervention_scores").unwrap_or_default()
}

// ─── Semantic embedding support (Phase 9) ────────────────────────────────────
//
// Architecture is ready for candle integration. When the `semantic` feature is
// enabled (future), these functions use a MiniLM embedding model for:
//   - Semantic error clustering (vs string prefix matching)
//   - File relevance ranking (vs RFO heuristic)
//   - Dead-end similarity detection
//
// Without the feature, everything falls back to string-distance heuristics.
// The candle deps (candle-core, candle-nn, candle-transformers, tokenizers, hf-hub)
// are added to Cargo.toml behind `[features] semantic = [...]` when ready.

/// Compute string similarity (Jaccard on words). Semantic upgrade path: replace
/// with embedding cosine similarity when candle feature is available.
pub fn text_similarity(a: &str, b: &str) -> f64 {
    let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();
    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}

/// Cluster strings by similarity. Returns groups of indices.
/// Semantic upgrade path: replace threshold comparison with embedding distance.
pub fn cluster_by_similarity(items: &[String], threshold: f64) -> Vec<Vec<usize>> {
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut assigned: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for i in 0..items.len() {
        if assigned.contains(&i) { continue; }
        let mut cluster = vec![i];
        assigned.insert(i);
        for j in (i + 1)..items.len() {
            if assigned.contains(&j) { continue; }
            if text_similarity(&items[i], &items[j]) > threshold {
                cluster.push(j);
                assigned.insert(j);
            }
        }
        clusters.push(cluster);
    }
    clusters
}
