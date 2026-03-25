// ─── pretool_bash::dedup — pre-execution command dedup ────────────────────────
//
// Two modes:
//   - Read-only commands (git status, git diff, etc.): DENY if no edits since last run
//   - Other commands: ADVISORY if no edits since last run (Claude usually skips)

use crate::common;

/// Git read-only command prefixes — safe to hard-deny duplicates
const GIT_READONLY: &[&str] = &[
    "git status",
    "git diff",
    "git log",
    "git show",
    "git branch",
    "git remote",
    "git tag",
    "git ls-files",
    "git stash list",
    "git describe",
    "git rev-parse",
    "just status",
    "just log-compact",
    "just diff",
    "just diff-staged",
    "just diff-stat",
    "just show",
    "just branches",
    "just remotes",
    "just changed-files",
    "just last-commit",
];

/// Check if this command was already run with identical output and no edits since.
/// Returns (should_return, state) — if should_return is true, deny/advisory was emitted.
/// Returns the read state for reuse by build_check.
pub fn check_dedup(cmd: &str) -> (bool, common::SessionState) {
    let mut state = common::read_session_state();

    let mut normalized = String::with_capacity(cmd.len());
    for (i, word) in cmd.split_whitespace().enumerate() {
        if i > 0 {
            normalized.push(' ');
        }
        normalized.push_str(word);
    }

    if let Some(prev) = state.commands.get(&normalized) {
        // Only dedup if no edits happened after the previous run
        if state.last_edit_turn < prev.turn {
            let saved = prev.output_tokens.max(200);
            state.estimated_tokens_saved += saved;
            state.savings_dedup += 1;
            common::write_session_state(&state);

            // Read-only commands get hard DENY (output is guaranteed identical)
            if is_readonly_cmd(&normalized) {
                common::log(
                    "pretool-bash",
                    &format!(
                        "DENY dedup-readonly: {} (saved ~{}tok)",
                        common::truncate(cmd, 60),
                        saved
                    ),
                );
                common::deny(
                    "PreToolUse",
                    &format!(
                        "You ran this at turn {} with identical output. No files edited since. Output is still in your context.",
                        prev.turn
                    ),
                );
                return (true, state);
            }

            // Other commands get advisory (might have side effects)
            common::log(
                "pretool-bash",
                &format!(
                    "DEDUP pre-exec: {} (saved ~{}tok)",
                    common::truncate(cmd, 60),
                    saved
                ),
            );
            common::allow_with_advisory(
                "PreToolUse",
                &format!(
                    "You already ran this command at turn {} with identical output. No files edited since. Consider skipping.",
                    prev.turn
                ),
            );
            return (true, state);
        }
    }

    (false, state)
}

fn is_readonly_cmd(normalized: &str) -> bool {
    GIT_READONLY
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}
