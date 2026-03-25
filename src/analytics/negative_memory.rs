// ─── analytics::negative_memory — dead-end tracking ─────────────────────────
//
// Records files and commands that were explored but yielded no progress.
// Warns when the agent revisits a dead end without new evidence.

use crate::common::SessionState;

const MAX_DEAD_ENDS: usize = 20;

/// Record a dead end (file or command that yielded no progress)
pub fn record_dead_end(state: &mut SessionState, description: &str) {
    // Deduplicate by prefix (same file or command)
    let prefix = description.split(':').next().unwrap_or(description);
    state.dead_ends.retain(|d| !d.starts_with(prefix));

    if state.dead_ends.len() >= MAX_DEAD_ENDS {
        state.dead_ends.remove(0);
    }
    state.dead_ends.push(description.to_string());
}

/// Check if a file path matches a known dead end
pub fn check_revisit(state: &SessionState, file_path: &str) -> Option<String> {
    for dead_end in &state.dead_ends {
        if dead_end.starts_with(file_path) {
            let reason = dead_end.split(':').nth(1).unwrap_or("no useful signal");
            return Some(format!(
                "Previously explored {} and found: {}. Choose a different approach.",
                file_path,
                reason.trim()
            ));
        }
    }
    None
}

/// Record a failed command (for command outcome memory)
pub fn record_command_failure(state: &mut SessionState, cmd_prefix: &str) {
    let count = state
        .failed_commands
        .entry(cmd_prefix.to_string())
        .or_insert(0);
    *count += 1;

    // Bound the map
    if state.failed_commands.len() > 20
        && let Some(oldest) = state.failed_commands.keys().next().cloned()
    {
        state.failed_commands.remove(&oldest);
    }
}

/// Clear a command from failure memory on success
pub fn record_command_success(state: &mut SessionState, cmd_prefix: &str) {
    state.failed_commands.remove(cmd_prefix);
}

/// Check if a command has failed repeatedly
pub fn check_repeated_failure(state: &SessionState, cmd_prefix: &str) -> Option<String> {
    if let Some(&count) = state.failed_commands.get(cmd_prefix)
        && count >= 2
    {
        return Some(format!(
            "This command pattern failed {} times with no relevant changes since. Try a different approach.",
            count
        ));
    }
    None
}
