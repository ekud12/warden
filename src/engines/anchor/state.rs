// ─── Engine: Anchor — SessionState (Single Source of Truth) ─────────────────
//
// Central session state view owned by Anchor. Consolidates runtime signals
// from Compass (phase/adaptation), Focus (salience), Trust (composite score),
// Debt (verification debt), and Ledger (turn tracking) into one coherent
// snapshot.
//
// Design:
//   - AnchorSessionState is the canonical read view for Reflex (via &ref).
//   - Dream receives updates through async channels (future work).
//   - Constructed from common::SessionState via From impl (no data duplication).
//   - Does NOT replace common::SessionState yet — that's a separate migration.
//     This type layers a semantic view on top of the raw persistence struct.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::engines::anchor::compass::{AdaptiveState, SessionPhase};
use crate::engines::anchor::focus;
use crate::engines::anchor::trust;
use serde::{Deserialize, Serialize};

// ─── Core type ──────────────────────────────────────────────────────────────

/// Central session state — owned exclusively by Anchor.
/// All state mutations go through Anchor methods.
/// Reflex gets &AnchorSessionState (read-only).
/// Dream gets async update channel (future).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorSessionState {
    // ── Identity ──
    pub phase: SessionPhase,
    pub turn_count: u32,
    pub project_type: String,

    // ── Health signals ──
    pub trust_score: u32,
    pub focus_score: u32,
    pub verification_debt: u32,
    pub reads_since_edit: u32,
    pub errors_unresolved: u32,
    pub turns_since_checkpoint: u32,

    // ── Working context ──
    pub working_set: Vec<String>,
    pub files_edited: Vec<String>,
    pub goals: GoalSnapshot,
    pub last_tool: Option<String>,
    pub last_edit_turn: u32,

    // ── Progress tracking ──
    pub milestones: Vec<String>,
    pub dead_ends: Vec<String>,
    pub error_count: u32,

    // ── Token economy ──
    pub estimated_tokens_in: u64,
    pub estimated_tokens_out: u64,
    pub estimated_tokens_saved: u64,

    // ── Adaptation state (delegated to Compass) ──
    pub adaptive: AdaptiveState,
}

/// Snapshot of structured goal state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoalSnapshot {
    pub primary: String,
    pub subgoal: String,
    pub blocked_on: String,
}

// ─── Defaults ───────────────────────────────────────────────────────────────

impl Default for AnchorSessionState {
    fn default() -> Self {
        Self {
            phase: SessionPhase::default(),
            turn_count: 0,
            project_type: String::new(),
            trust_score: 100,
            focus_score: 100,
            verification_debt: 0,
            reads_since_edit: 0,
            errors_unresolved: 0,
            turns_since_checkpoint: 0,
            working_set: Vec::new(),
            files_edited: Vec::new(),
            goals: GoalSnapshot::default(),
            last_tool: None,
            last_edit_turn: 0,
            milestones: Vec::new(),
            dead_ends: Vec::new(),
            error_count: 0,
            estimated_tokens_in: 0,
            estimated_tokens_out: 0,
            estimated_tokens_saved: 0,
            adaptive: AdaptiveState::default(),
        }
    }
}

// ─── Health assessment ──────────────────────────────────────────────────────

impl AnchorSessionState {
    /// Session is healthy: trust above threshold, few unresolved errors
    pub fn is_healthy(&self) -> bool {
        self.trust_score > 50 && self.errors_unresolved < 3
    }

    /// Session is drifting: high verification debt, many reads without edits,
    /// or focus score critically low
    pub fn is_drifting(&self) -> bool {
        self.verification_debt >= 4
            || self.reads_since_edit >= 7
            || self.focus_score < 30
    }

    /// Session needs intervention: struggling phase or very low trust
    pub fn needs_intervention(&self) -> bool {
        self.phase == SessionPhase::Struggling || self.trust_score < 25
    }

    /// Session is under context pressure (Late phase or high token usage)
    pub fn is_under_pressure(&self) -> bool {
        self.phase == SessionPhase::Late
            || (self.estimated_tokens_in + self.estimated_tokens_out) > 500_000
    }

    /// Total estimated token usage
    pub fn total_tokens(&self) -> u64 {
        self.estimated_tokens_in + self.estimated_tokens_out
    }

    /// Token savings percentage (0-100)
    pub fn savings_pct(&self) -> u64 {
        let total = self.total_tokens() + self.estimated_tokens_saved;
        if total > 0 {
            (self.estimated_tokens_saved * 100) / total
        } else {
            0
        }
    }
}

// ─── Conversion from legacy common::SessionState ────────────────────────────

impl From<&common::SessionState> for AnchorSessionState {
    fn from(s: &common::SessionState) -> Self {
        // Compute live trust and focus scores from the source state
        let trust_score = trust::compute_trust(s);
        let focus_report = focus::compute_focus(s);

        // Extract last action from action history as last_tool proxy
        let last_tool = s.action_history.last().cloned();

        // Collect milestones: last_milestone + any from decisions containing "milestone"
        let mut milestones = Vec::new();
        if !s.last_milestone.is_empty() {
            milestones.push(s.last_milestone.clone());
        }

        Self {
            phase: s.adaptive.phase,
            turn_count: s.turn,
            project_type: s.project_type.clone(),
            trust_score,
            focus_score: focus_report.score,
            verification_debt: s.edits_since_verification,
            reads_since_edit: s.reads_since_edit,
            errors_unresolved: s.errors_unresolved,
            turns_since_checkpoint: s.turns_since_checkpoint,
            working_set: s.rolling_working_set.clone(),
            files_edited: s.files_edited.clone(),
            goals: GoalSnapshot {
                primary: s.goal_stack.primary.clone(),
                subgoal: s.goal_stack.subgoal.clone(),
                blocked_on: s.goal_stack.blocked_on.clone(),
            },
            last_tool,
            last_edit_turn: s.last_edit_turn,
            milestones,
            dead_ends: s.dead_ends.clone(),
            error_count: s.errors_unresolved, // current unresolved as proxy
            estimated_tokens_in: s.estimated_tokens_in,
            estimated_tokens_out: s.estimated_tokens_out,
            estimated_tokens_saved: s.estimated_tokens_saved,
            adaptive: s.adaptive.clone(),
        }
    }
}

impl From<common::SessionState> for AnchorSessionState {
    fn from(s: common::SessionState) -> Self {
        AnchorSessionState::from(&s)
    }
}

// ─── Snapshot for read-only consumers ───────────────────────────────────────

impl AnchorSessionState {
    /// Create from the current persisted session state (convenience).
    /// Reads common::SessionState from disk/cache, converts to Anchor view.
    pub fn current() -> Self {
        let raw = common::read_session_state();
        AnchorSessionState::from(&raw)
    }

    /// Compact summary string for logging/injection
    pub fn summary(&self) -> String {
        format!(
            "turn={} phase={} trust={} focus={} debt={} errors={} tokens=~{}K",
            self.turn_count,
            self.phase,
            self.trust_score,
            self.focus_score,
            self.verification_debt,
            self.errors_unresolved,
            self.total_tokens() / 1000,
        )
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_healthy() {
        let state = AnchorSessionState::default();
        assert!(state.is_healthy());
        assert!(!state.is_drifting());
        assert!(!state.needs_intervention());
        assert!(!state.is_under_pressure());
    }

    #[test]
    fn unhealthy_on_low_trust() {
        let state = AnchorSessionState {
            trust_score: 30,
            ..Default::default()
        };
        assert!(!state.is_healthy());
    }

    #[test]
    fn unhealthy_on_many_errors() {
        let state = AnchorSessionState {
            errors_unresolved: 5,
            ..Default::default()
        };
        assert!(!state.is_healthy());
    }

    #[test]
    fn drifting_on_high_debt() {
        let state = AnchorSessionState {
            verification_debt: 6,
            ..Default::default()
        };
        assert!(state.is_drifting());
    }

    #[test]
    fn drifting_on_excessive_reads() {
        let state = AnchorSessionState {
            reads_since_edit: 10,
            ..Default::default()
        };
        assert!(state.is_drifting());
    }

    #[test]
    fn drifting_on_low_focus() {
        let state = AnchorSessionState {
            focus_score: 20,
            ..Default::default()
        };
        assert!(state.is_drifting());
    }

    #[test]
    fn needs_intervention_when_struggling() {
        let state = AnchorSessionState {
            phase: SessionPhase::Struggling,
            ..Default::default()
        };
        assert!(state.needs_intervention());
    }

    #[test]
    fn needs_intervention_on_very_low_trust() {
        let state = AnchorSessionState {
            trust_score: 15,
            ..Default::default()
        };
        assert!(state.needs_intervention());
    }

    #[test]
    fn under_pressure_when_late() {
        let state = AnchorSessionState {
            phase: SessionPhase::Late,
            ..Default::default()
        };
        assert!(state.is_under_pressure());
    }

    #[test]
    fn under_pressure_on_high_tokens() {
        let state = AnchorSessionState {
            estimated_tokens_in: 300_000,
            estimated_tokens_out: 250_000,
            ..Default::default()
        };
        assert!(state.is_under_pressure());
    }

    #[test]
    fn total_tokens_calculation() {
        let state = AnchorSessionState {
            estimated_tokens_in: 100,
            estimated_tokens_out: 200,
            ..Default::default()
        };
        assert_eq!(state.total_tokens(), 300);
    }

    #[test]
    fn savings_pct_calculation() {
        let state = AnchorSessionState {
            estimated_tokens_in: 400,
            estimated_tokens_out: 400,
            estimated_tokens_saved: 200,
            ..Default::default()
        };
        assert_eq!(state.savings_pct(), 20); // 200 / (800 + 200) = 20%
    }

    #[test]
    fn savings_pct_zero_tokens() {
        let state = AnchorSessionState::default();
        assert_eq!(state.savings_pct(), 0);
    }

    #[test]
    fn from_common_session_state() {
        let mut raw = common::SessionState::default();
        raw.turn = 10;
        raw.errors_unresolved = 2;
        raw.edits_since_verification = 3;
        raw.reads_since_edit = 1;
        raw.last_milestone = "tests passing".to_string();
        raw.goal_stack.primary = "implement feature X".to_string();
        raw.rolling_working_set = vec!["src".to_string(), "tests".to_string()];
        raw.files_edited = vec!["src/main.rs".to_string()];
        raw.estimated_tokens_in = 5000;
        raw.estimated_tokens_out = 3000;
        raw.estimated_tokens_saved = 1000;
        raw.project_type = "rust".to_string();
        raw.dead_ends = vec!["tried approach A".to_string()];
        raw.action_history = vec!["edit".to_string(), "bash_ok".to_string()];

        let anchor = AnchorSessionState::from(&raw);

        assert_eq!(anchor.turn_count, 10);
        assert_eq!(anchor.errors_unresolved, 2);
        assert_eq!(anchor.verification_debt, 3);
        assert_eq!(anchor.reads_since_edit, 1);
        assert_eq!(anchor.milestones, vec!["tests passing"]);
        assert_eq!(anchor.goals.primary, "implement feature X");
        assert_eq!(anchor.working_set, vec!["src", "tests"]);
        assert_eq!(anchor.files_edited, vec!["src/main.rs"]);
        assert_eq!(anchor.estimated_tokens_in, 5000);
        assert_eq!(anchor.estimated_tokens_out, 3000);
        assert_eq!(anchor.estimated_tokens_saved, 1000);
        assert_eq!(anchor.project_type, "rust");
        assert_eq!(anchor.dead_ends, vec!["tried approach A"]);
        assert_eq!(anchor.last_tool, Some("bash_ok".to_string()));
        assert!(anchor.is_healthy());
    }

    #[test]
    fn summary_format() {
        let state = AnchorSessionState {
            turn_count: 15,
            phase: SessionPhase::Productive,
            trust_score: 85,
            focus_score: 70,
            verification_debt: 2,
            errors_unresolved: 1,
            estimated_tokens_in: 50_000,
            estimated_tokens_out: 30_000,
            ..Default::default()
        };
        let s = state.summary();
        assert!(s.contains("turn=15"));
        assert!(s.contains("phase=Productive"));
        assert!(s.contains("trust=85"));
        assert!(s.contains("focus=70"));
        assert!(s.contains("debt=2"));
        assert!(s.contains("errors=1"));
        assert!(s.contains("tokens=~80K"));
    }

    #[test]
    fn serialization_roundtrip() {
        let state = AnchorSessionState {
            turn_count: 42,
            phase: SessionPhase::Exploring,
            trust_score: 65,
            focus_score: 55,
            verification_debt: 3,
            reads_since_edit: 4,
            errors_unresolved: 1,
            working_set: vec!["src".to_string()],
            goals: GoalSnapshot {
                primary: "build feature".to_string(),
                subgoal: "write tests".to_string(),
                blocked_on: String::new(),
            },
            ..Default::default()
        };

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: AnchorSessionState =
            serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.turn_count, 42);
        assert_eq!(restored.phase, SessionPhase::Exploring);
        assert_eq!(restored.trust_score, 65);
        assert_eq!(restored.focus_score, 55);
        assert_eq!(restored.verification_debt, 3);
        assert_eq!(restored.reads_since_edit, 4);
        assert_eq!(restored.errors_unresolved, 1);
        assert_eq!(restored.working_set, vec!["src"]);
        assert_eq!(restored.goals.primary, "build feature");
        assert_eq!(restored.goals.subgoal, "write tests");
    }
}
