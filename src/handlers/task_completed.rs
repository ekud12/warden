// ─── task_completed — direct milestone signal on task completion ─────────────
//
// Handles TaskCompleted events to reset error counters and record milestones.
// Solves stop-check false-blocks where errors stay "unresolved" because bash
// output pattern-matching missed the resolution signal.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;

pub fn run(raw: &str) {
    let v: serde_json::Value = serde_json::from_str(raw).unwrap_or_default();
    let description = v.get("description")
        .or(v.get("task_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("task completed");

    let detail = common::truncate(description, 100);

    let mut state = common::read_session_state();
    state.last_milestone = detail.clone();
    state.explore_count = 0;
    state.errors_unresolved = 0;
    common::write_session_state(&state);

    common::add_session_note("milestone", &detail);
    common::log("task-completed", &detail);
}
