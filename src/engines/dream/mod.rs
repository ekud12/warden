// ─── Dream Engine — "Learn Quietly" ──────────────────────────────────────────
//
// Extracts reusable patterns and compresses experience during daemon idle time.
// Runs asynchronously — never blocks hook calls.
//
// Modules:
//   Imprint — error clustering + baseline consolidation (E1, E4, E6)
//   Trace   — successful sequence + repair pattern learning (E7, E8)
//   Lore    — convention learning + cross-project knowledge (E9)
//   Pruner  — artifact scoring, decay, and cleanup (E5, E10)
//   Replay  — resume packet generation + working set ranking (E2, E3)
// ──────────────────────────────────────────────────────────────────────────────

pub mod imprint;
pub mod dna;
pub mod trace;
pub mod lore;
pub mod pruner;
pub mod replay;

use crate::common;
use crate::engines::signal::Budget;
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
    /// E7: Mine successful action sequences from events
    LearnSequences,
    /// E8: Learn repair patterns from error → fix sequences
    LearnRepairPatterns,
    /// E9: Learn project conventions from recurring patterns
    LearnConventions,
    /// E10: Score and prune dream artifacts by usefulness
    ScoreArtifacts,
}

impl DreamTask {
    pub fn budget(&self) -> Budget {
        match self {
            Self::ConsolidateEvents => Budget { max_events: 500, max_artifacts: 50, max_output_chars: 10_000, max_ms: 200 },
            Self::BuildResumePacket => Budget { max_events: 200, max_artifacts: 20, max_output_chars: 5_000, max_ms: 100 },
            Self::ClusterErrors => Budget { max_events: 500, max_artifacts: 100, max_output_chars: 15_000, max_ms: 300 },
            Self::UpdateWorkingSetRanking => Budget { max_events: 200, max_artifacts: 20, max_output_chars: 5_000, max_ms: 100 },
            Self::LearnEffectiveness => Budget { max_events: 500, max_artifacts: 50, max_output_chars: 10_000, max_ms: 200 },
            Self::BuildDeadEndMemory => Budget { max_events: 200, max_artifacts: 20, max_output_chars: 5_000, max_ms: 100 },
            Self::LearnSequences => Budget { max_events: 1000, max_artifacts: 100, max_output_chars: 20_000, max_ms: 500 },
            Self::LearnRepairPatterns => Budget { max_events: 1000, max_artifacts: 50, max_output_chars: 15_000, max_ms: 500 },
            Self::LearnConventions => Budget { max_events: 500, max_artifacts: 50, max_output_chars: 10_000, max_ms: 300 },
            Self::ScoreArtifacts => Budget { max_events: 200, max_artifacts: 200, max_output_chars: 10_000, max_ms: 200 },
        }
    }
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
    // ── V2 fields (serde(default) ensures backward compat) ──
    /// Top playbook candidate if available
    #[serde(default)]
    pub top_playbook: String,
    /// High-confidence convention hints
    #[serde(default)]
    pub convention_hints: Vec<String>,
    /// Open verification debt (edits since last build)
    #[serde(default)]
    pub verification_debt: u32,
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

// ─── Dream V2 — Typed Procedural Knowledge ──────────────────────────────────

/// A learned procedure that leads to successful outcomes
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DreamPlaybook {
    pub id: String,
    pub name: String,
    pub trigger_signals: Vec<String>,
    pub recommended_steps: Vec<String>,
    pub evidence_count: u32,
    pub success_rate: f64,
    pub last_seen_turn: u32,
    pub source_sessions: u32,
}

/// A learned mapping from error signature to successful remediation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepairPattern {
    pub error_signature: String,
    pub affected_files: Vec<String>,
    pub commands_that_helped: Vec<String>,
    pub verification_step: String,
    pub success_count: u32,
    pub last_seen_turn: u32,
}

/// A learned project convention (e.g., preferred build command, common entry files)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectConvention {
    pub kind: String,
    pub observation: String,
    pub confidence: f64,
    pub evidence_count: u32,
    pub last_updated_turn: u32,
}

/// A successful action sequence extracted from session events
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SuccessfulSequence {
    pub actions: Vec<String>,
    pub led_to_milestone: bool,
    pub occurrences: u32,
    pub last_seen_turn: u32,
}

const TASK_ORDER: &[DreamTask] = &[
    DreamTask::LearnEffectiveness, // highest value — feeds back into injection budget
    DreamTask::BuildResumePacket,  // second — ready for session resume
    DreamTask::LearnSequences,     // mine successful sequences
    DreamTask::ClusterErrors,      // compress noisy error history
    DreamTask::LearnRepairPatterns, // error → fix mappings
    DreamTask::LearnConventions,   // project conventions
    DreamTask::UpdateWorkingSetRanking,
    DreamTask::BuildDeadEndMemory,
    DreamTask::ScoreArtifacts,     // prune weak artifacts
    DreamTask::ConsolidateEvents,  // lowest priority — housekeeping
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
    let budget = batch.kind.budget();
    let start = std::time::Instant::now();

    match batch.kind {
        DreamTask::LearnEffectiveness => pruner::learn_effectiveness(),
        DreamTask::BuildResumePacket => replay::build_resume_packet(),
        DreamTask::ClusterErrors => imprint::cluster_errors(),
        DreamTask::UpdateWorkingSetRanking => replay::update_working_set(),
        DreamTask::BuildDeadEndMemory => imprint::build_dead_end_memory(),
        DreamTask::ConsolidateEvents => imprint::consolidate_events(),
        DreamTask::LearnSequences => trace::learn_sequences(),
        DreamTask::LearnRepairPatterns => trace::learn_repair_patterns(),
        DreamTask::LearnConventions => lore::learn_conventions(),
        DreamTask::ScoreArtifacts => pruner::score_artifacts(),
    }

    let elapsed = start.elapsed().as_millis() as u64;
    if elapsed > budget.max_ms {
        common::log("dream", &format!(
            "{:?} exceeded budget: {}ms > {}ms limit",
            batch.kind, elapsed, budget.max_ms
        ));
    }
}

// ─── Dream V2 — Public Accessors ────────────────────────────────────────────

/// Get learned playbooks (for suggest_action)
pub fn get_sequences() -> BTreeMap<String, SuccessfulSequence> {
    common::storage::read_json("dream", "sequences").unwrap_or_default()
}

/// Get learned repair patterns
pub fn get_repair_patterns() -> Vec<RepairPattern> {
    common::storage::read_json("dream", "repair_patterns").unwrap_or_default()
}

/// Get learned project conventions
pub fn get_conventions() -> Vec<ProjectConvention> {
    common::storage::read_json("dream", "conventions").unwrap_or_default()
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
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Cluster strings by similarity. Returns groups of indices.
/// Semantic upgrade path: replace threshold comparison with embedding distance.
pub fn cluster_by_similarity(items: &[String], threshold: f64) -> Vec<Vec<usize>> {
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut assigned: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for i in 0..items.len() {
        if assigned.contains(&i) {
            continue;
        }
        let mut cluster = vec![i];
        assigned.insert(i);
        for j in (i + 1)..items.len() {
            if assigned.contains(&j) {
                continue;
            }
            if text_similarity(&items[i], &items[j]) > threshold {
                cluster.push(j);
                assigned.insert(j);
            }
        }
        clusters.push(cluster);
    }
    clusters
}
