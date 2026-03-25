// ─── adaptation — intra-session phase detection and threshold adaptation ─────
//
// Classifies the session into phases (Warmup, Productive, Exploring, Struggling,
// Late) based on TurnSnapshot patterns, then adjusts ~8 runtime parameters per
// phase. Every transition is logged with reasoning to session-notes.jsonl.
//
// Design principles:
//   - Single module, single entry point: adapt()
//   - Phase presets are declarative (phase_params lookup table)
//   - Hysteresis prevents flapping (candidate must sustain 2 turns)
//   - All adaptations are bounded (hard min/max per parameter)
//   - Self-reflective: every transition logged with before/after/reason
//   - Late is one-way (never exits — context pressure only increases)
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::handlers::userprompt_context;
use crate::rules;
use serde::{Deserialize, Serialize};

/// Minimum turns a candidate phase must sustain before committing
const HYSTERESIS_TURNS: u32 = 2;
/// Maximum stored phase transitions
const MAX_TRANSITIONS: usize = 10;

// ─── Types ──────────────────────────────────────────────────────────────────

/// Session phases detected from TurnSnapshot patterns
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Default, PartialEq)]
pub enum SessionPhase {
    #[default]
    Warmup,
    Productive,
    Exploring,
    Struggling,
    Late,
}

impl std::fmt::Display for SessionPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Warmup => write!(f, "Warmup"),
            Self::Productive => write!(f, "Productive"),
            Self::Exploring => write!(f, "Exploring"),
            Self::Struggling => write!(f, "Struggling"),
            Self::Late => write!(f, "Late"),
        }
    }
}

/// Adapted runtime parameters — override static rules::RULES values per phase
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AdaptedParams {
    pub advisory_cooldown: u32,
    pub truncation_max_lines: usize,
    pub mcp_output_limit: usize,
    pub explore_budget: u32,
    pub context_chars_max: usize,
    pub rules_reinject_interval: u32,
    pub read_dedup_window: u32,
    pub drift_threshold: u32,
}

impl Default for AdaptedParams {
    fn default() -> Self {
        phase_params(&SessionPhase::Warmup)
    }
}

/// Persistent adaptation state stored in SessionState
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct AdaptiveState {
    pub phase: SessionPhase,
    pub phase_entered_turn: u32,
    pub candidate_phase: Option<SessionPhase>,
    pub candidate_since_turn: u32,
    pub params: AdaptedParams,
    pub transitions: Vec<PhaseTransition>,
}

/// Logged phase transition
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PhaseTransition {
    pub turn: u32,
    pub from: String,
    pub to: String,
    pub reason: String,
}

// ─── Phase presets ──────────────────────────────────────────────────────────

fn phase_params(phase: &SessionPhase) -> AdaptedParams {
    match phase {
        SessionPhase::Warmup => AdaptedParams {
            advisory_cooldown: 5,
            truncation_max_lines: 80,
            mcp_output_limit: 15_000,
            explore_budget: 10,
            context_chars_max: 1200,
            rules_reinject_interval: 30,
            read_dedup_window: 10,
            drift_threshold: 3,
        },
        SessionPhase::Productive => AdaptedParams {
            advisory_cooldown: 7,
            truncation_max_lines: 90,
            mcp_output_limit: 18_000,
            explore_budget: 8,
            context_chars_max: 1000,
            rules_reinject_interval: 40,
            read_dedup_window: 15,
            drift_threshold: 4,
        },
        SessionPhase::Exploring => AdaptedParams {
            advisory_cooldown: 4,
            truncation_max_lines: 70,
            mcp_output_limit: 12_000,
            explore_budget: 5,
            context_chars_max: 1200,
            rules_reinject_interval: 25,
            read_dedup_window: 8,
            drift_threshold: 3,
        },
        SessionPhase::Struggling => AdaptedParams {
            advisory_cooldown: 3,
            truncation_max_lines: 60,
            mcp_output_limit: 10_000,
            explore_budget: 8,
            context_chars_max: 1500,
            rules_reinject_interval: 15,
            read_dedup_window: 10,
            drift_threshold: 2,
        },
        SessionPhase::Late => AdaptedParams {
            advisory_cooldown: 3,
            truncation_max_lines: 40,
            mcp_output_limit: 7_000,
            explore_budget: 6,
            context_chars_max: 800,
            rules_reinject_interval: 20,
            read_dedup_window: 20,
            drift_threshold: 2,
        },
    }
}

// ─── Classification ─────────────────────────────────────────────────────────

/// Pure function: classify session phase from recent snapshots.
fn classify(
    snapshots: &[common::TurnSnapshot],
    turn: u32,
    errors_unresolved: u32,
    last_edit_turn: u32,
    advisory_turn: u32,
) -> SessionPhase {
    // Late is highest priority and one-way
    if turn >= advisory_turn {
        return SessionPhase::Late;
    }

    // Warmup: early turns with no edits
    if turn <= 5 && last_edit_turn == 0 {
        return SessionPhase::Warmup;
    }

    // Need at least 3 snapshots for pattern detection
    if snapshots.len() < 3 {
        return if last_edit_turn > 0 {
            SessionPhase::Productive
        } else {
            SessionPhase::Warmup
        };
    }

    let recent = if snapshots.len() > 5 {
        &snapshots[snapshots.len() - 5..]
    } else {
        snapshots
    };

    // Struggling: errors rising + no milestones
    if errors_unresolved >= 3 {
        let slope = userprompt_context::error_slope(snapshots, 10);
        let has_recent_milestone = recent.iter().any(|s| s.milestones_hit);
        if slope > 0.3 && !has_recent_milestone {
            return SessionPhase::Struggling;
        }
    }

    // Productive: recent edits or milestones
    let recent3 = if snapshots.len() > 3 {
        &snapshots[snapshots.len() - 3..]
    } else {
        snapshots
    };
    let has_recent_edits = recent3.iter().any(|s| s.edits_this_turn);
    let has_recent_milestone = recent.iter().any(|s| s.milestones_hit);
    if has_recent_edits || has_recent_milestone {
        return SessionPhase::Productive;
    }

    // Exploring: explore growing, no edits, no milestones
    let explore_growing = recent3
        .windows(2)
        .all(|w| w[1].explore_count >= w[0].explore_count);
    let no_edits = !recent3.iter().any(|s| s.edits_this_turn);
    if explore_growing && no_edits {
        return SessionPhase::Exploring;
    }

    // Default: stay Productive if we've ever edited, else Warmup
    if last_edit_turn > 0 {
        SessionPhase::Productive
    } else {
        SessionPhase::Warmup
    }
}

// ─── Entry point ────────────────────────────────────────────────────────────

/// Called once per turn from userprompt_context, after snapshot recording.
/// Returns optional context string to inject on phase change.
pub fn adapt(state: &mut common::SessionState) -> Option<String> {
    let advisory_turn = rules::RULES.progressive_read_advisory_turn;
    let candidate = classify(
        &state.turn_snapshots,
        state.turn,
        state.errors_unresolved,
        state.last_edit_turn,
        advisory_turn,
    );

    // Late overrides everything immediately (no hysteresis)
    if candidate == SessionPhase::Late && state.adaptive.phase != SessionPhase::Late {
        return commit_transition(
            state,
            SessionPhase::Late,
            "Turn threshold reached — context pressure mode",
        );
    }

    // Same as current phase — clear candidate
    if candidate == state.adaptive.phase {
        state.adaptive.candidate_phase = None;
        return None;
    }

    // New candidate — start hysteresis
    if state.adaptive.candidate_phase.as_ref() != Some(&candidate) {
        state.adaptive.candidate_phase = Some(candidate);
        state.adaptive.candidate_since_turn = state.turn;
        return None;
    }

    // Existing candidate — check if sustained long enough
    if state
        .turn
        .saturating_sub(state.adaptive.candidate_since_turn)
        >= HYSTERESIS_TURNS
    {
        let reason = transition_reason(&state.adaptive.phase, &candidate, state);
        let new_phase = candidate;
        state.adaptive.candidate_phase = None;
        return commit_transition(state, new_phase, &reason);
    }

    None
}

/// Commit a phase transition: update params, log, write session note.
fn commit_transition(
    state: &mut common::SessionState,
    new_phase: SessionPhase,
    reason: &str,
) -> Option<String> {
    let old_phase = state.adaptive.phase;
    let turn = state.turn;

    // Record transition
    let transition = PhaseTransition {
        turn,
        from: old_phase.to_string(),
        to: new_phase.to_string(),
        reason: reason.to_string(),
    };
    state.adaptive.transitions.push(transition);

    // Enforce bounds
    if state.adaptive.transitions.len() > MAX_TRANSITIONS {
        state
            .adaptive
            .transitions
            .drain(..state.adaptive.transitions.len() - MAX_TRANSITIONS);
    }

    // Apply new phase params
    state.adaptive.phase = new_phase;
    state.adaptive.phase_entered_turn = turn;
    state.adaptive.params = phase_params(&new_phase);

    // Log to session notes
    let data = serde_json::json!({
        "from": old_phase.to_string(),
        "to": new_phase.to_string(),
        "turn": turn,
        "reason": reason,
        "params": {
            "advisory_cooldown": state.adaptive.params.advisory_cooldown,
            "truncation_max_lines": state.adaptive.params.truncation_max_lines,
            "mcp_output_limit": state.adaptive.params.mcp_output_limit,
            "explore_budget": state.adaptive.params.explore_budget,
        }
    });
    let detail = format!("{}→{} at turn {}", old_phase, new_phase, turn);
    common::add_session_note_ext("adaptation", &detail, Some(&data));
    common::log_structured(
        "adaptation",
        common::LogLevel::Info,
        "phase-change",
        &detail,
    );

    // Context injection for Claude
    Some(format!(
        "Session phase: {} ({}). Adjusting parameters.",
        new_phase, reason
    ))
}

/// Generate a human-readable reason for the transition.
fn transition_reason(
    from: &SessionPhase,
    to: &SessionPhase,
    state: &common::SessionState,
) -> String {
    match to {
        SessionPhase::Productive => {
            if state.last_edit_turn >= state.turn.saturating_sub(3) {
                "Edits in recent turns".to_string()
            } else {
                "Milestone reached".to_string()
            }
        }
        SessionPhase::Struggling => {
            format!(
                "{} unresolved errors, error slope rising, no recent milestones",
                state.errors_unresolved
            )
        }
        SessionPhase::Exploring => {
            format!(
                "Explore count growing ({}) without edits for {} turns",
                state.explore_count,
                state.turn.saturating_sub(state.last_edit_turn)
            )
        }
        SessionPhase::Late => "Turn threshold reached — context pressure mode".to_string(),
        SessionPhase::Warmup => format!("Reverted from {} — early session", from),
    }
}
