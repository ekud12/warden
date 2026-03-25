// ─── posttool_session::edit — Write/Edit/MultiEdit state tracking ────────────

use crate::common;

/// Update session state for Write/Edit/MultiEdit operations
pub fn update_edit_state(file_path: &str) {
    let mut state = common::read_session_state();

    // Premature execution detection: editing before enough evidence gathered
    if state.turn <= 5 && state.files_read.len() < 3 && state.files_edited.is_empty() {
        common::log("intelligence", &format!(
            "Early editing: only {} files examined before first edit at turn {}",
            state.files_read.len(), state.turn
        ));
    }

    // Reset explore count — editing means committing to an approach
    state.explore_count = 0;
    state.reads_since_edit = 0;
    state.last_edit_turn = state.turn;
    state.last_edited_file = file_path.to_string();

    // Verification debt: increment edits since last build/test
    state.edits_since_verification += 1;

    // Infer subgoal from file being edited
    let module = file_path
        .replace('\\', "/")
        .rsplit('/')
        .nth(1)
        .unwrap_or("unknown")
        .to_string();
    state.goal_stack.subgoal = module;

    // Track edited file (dedup)
    let short_path = shorten_path(file_path);
    if !state.files_edited.contains(&short_path) {
        state.files_edited.push(short_path);
    }

    state.enforce_bounds();
    common::write_session_state(&state);
}

/// Shorten a path for display (keep just filename or last 2 components)
pub fn shorten_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.rsplit('/').take(2).collect();
    if parts.len() >= 2 {
        format!("{}/{}", parts[1], parts[0])
    } else {
        parts.first().unwrap_or(&path).to_string()
    }
}
