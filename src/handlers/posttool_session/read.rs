// ─── posttool_session::read — Read tool state tracking ───────────────────────

use crate::common;

/// Update session state for Read operations
pub fn update_read_state(file_path: &str) {
    let mut state = common::read_session_state();
    state.explore_count += 1;

    // Record file read with content hash and mtime
    match common::content_hash(std::path::Path::new(file_path)) {
        Some(hash) => {
            let size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
            let mtime = common::file_mtime(std::path::Path::new(file_path)).unwrap_or(0);
            state.files_read.insert(
                file_path.to_string(),
                common::FileReadEntry {
                    hash,
                    turn: state.turn,
                    size,
                    mtime,
                },
            );
            common::log("posttool-session", &format!("READ tracked: {} (turn {})", common::truncate(file_path, 60), state.turn));
        }
        None => {
            common::log("posttool-session", &format!("READ hash-fail: {}", common::truncate(file_path, 60)));
        }
    }

    state.enforce_bounds();
    common::write_session_state(&state);
}
