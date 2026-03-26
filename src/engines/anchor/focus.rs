// ─── Engine: Anchor — Focus score computation ─────────────────────────────────
//
// Composite 0-100 score measuring session focus. Penalizes directory spread,
// subsystem switches without milestones, and excessive exploration without edits.

use crate::common::SessionState;
use crate::engines::signal::{Signal, SignalCategory};

pub struct FocusReport {
    pub score: u32,
    pub advisory: Option<String>,
}

/// Compute focus score from session state
pub fn compute_focus(state: &SessionState) -> FocusReport {
    let dir_count = state.directories_touched.len();
    let switches = state.subsystem_switches;
    let reads_no_edit = state.reads_since_edit;

    // Penalties
    let dir_penalty = (dir_count.saturating_sub(3) * 10).min(40) as u32;
    let switch_penalty = (switches * 8).min(30);
    let explore_penalty = (reads_no_edit.saturating_sub(5) * 5).min(30);

    let score = 100u32.saturating_sub(dir_penalty + switch_penalty + explore_penalty);

    let advisory = if score < 40 && state.turn >= 8 {
        Some(format!(
            "Focus score {}/100. {} dirs touched, {} subsystem switches without milestone. Narrow scope.",
            score, dir_count, switches
        ))
    } else {
        None
    };

    FocusReport { score, advisory }
}

pub fn compute_focus_signal(state: &SessionState) -> Option<Signal> {
    let report = compute_focus(state);
    report.advisory.map(|msg| Signal::advisory(SignalCategory::Focus, 0.5, msg, "focus"))
}

// ─── Working set tracking ────────────────────────────────────────────────────

/// Tracks the top files the agent is actively working with.
/// Ranked by recency, outcome (edits > reads), and error association.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct WorkingSet {
    entries: Vec<WorkingSetEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkingSetEntry {
    pub path: String,
    pub score: f64,
    pub last_access_turn: usize,
    pub was_edited: bool,
    pub had_error: bool,
}

impl WorkingSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a file access. Updates existing entry or adds new one.
    pub fn record_access(&mut self, path: &str, turn: usize, edited: bool, errored: bool) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path) {
            entry.last_access_turn = turn;
            entry.was_edited |= edited;
            entry.had_error |= errored;
            entry.score = Self::compute_score(entry);
        } else {
            let mut entry = WorkingSetEntry {
                path: path.to_string(),
                score: 0.0,
                last_access_turn: turn,
                was_edited: edited,
                had_error: errored,
            };
            entry.score = Self::compute_score(&entry);
            self.entries.push(entry);
        }
        self.entries.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.entries.truncate(10);
    }

    fn compute_score(entry: &WorkingSetEntry) -> f64 {
        let mut score = 1.0;
        if entry.was_edited {
            score += 2.0;
        }
        if entry.had_error {
            score += 1.5;
        }
        score += entry.last_access_turn as f64 * 0.1;
        score
    }

    /// Return the top N file paths by score.
    pub fn top(&self, n: usize) -> Vec<&str> {
        self.entries.iter().take(n).map(|e| e.path.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edited_files_rank_higher() {
        let mut ws = WorkingSet::new();
        ws.record_access("read_only.rs", 1, false, false);
        ws.record_access("edited.rs", 1, true, false);
        let top = ws.top(2);
        assert_eq!(top[0], "edited.rs", "edited file should rank first");
    }

    #[test]
    fn error_files_rank_higher() {
        let mut ws = WorkingSet::new();
        ws.record_access("ok.rs", 1, false, false);
        ws.record_access("broken.rs", 1, false, true);
        let top = ws.top(2);
        assert_eq!(top[0], "broken.rs", "error file should rank first");
    }

    #[test]
    fn top_10_limit_enforced() {
        let mut ws = WorkingSet::new();
        for i in 0..15 {
            ws.record_access(&format!("file_{}.rs", i), i, false, false);
        }
        assert_eq!(ws.len(), 10, "should cap at 10 entries");
    }

    #[test]
    fn recent_access_updates_score() {
        let mut ws = WorkingSet::new();
        ws.record_access("file.rs", 1, false, false);
        let score1 = ws.entries[0].score;
        ws.record_access("file.rs", 10, false, false);
        let score2 = ws.entries[0].score;
        assert!(score2 > score1, "recent access should increase score");
    }

    #[test]
    fn empty_working_set() {
        let ws = WorkingSet::new();
        assert!(ws.is_empty());
        assert_eq!(ws.top(5).len(), 0);
    }
}
