// ─── pretool_bash::build_check — no-op build detection ───────────────────────

use crate::common;
use crate::config;

/// Check if a build/test command would be a no-op (no edits since last build).
/// Accepts pre-read state from dedup step to avoid double-read.
/// Returns true if advisory was emitted (caller should return).
pub fn check_noop_build(cmd: &str, state: &mut common::SessionState) -> bool {
    // Only check build/test commands
    if !is_build_cmd(cmd) {
        return false;
    }

    // First build ever — allow
    if state.last_build_turn == 0 {
        return false;
    }

    // No edits since last successful build
    if state.last_edit_turn <= state.last_build_turn {
        // Record token savings from noop build skip
        let saved = state.last_build_output_tokens.max(500);
        state.estimated_tokens_saved += saved;
        state.savings_build_skip += 1;
        common::write_session_state(state);

        common::log(
            "pretool-bash",
            &format!(
                "NOOP build: {} (saved ~{}tok)",
                common::truncate(cmd, 60),
                saved
            ),
        );
        common::allow_with_advisory(
            "PreToolUse",
            &format!(
                "No source files edited since last successful build (turn {}). This build will likely be a no-op.",
                state.last_build_turn
            ),
        );
        return true;
    }

    false
}

fn is_build_cmd(cmd: &str) -> bool {
    config::BUILD_CMDS.iter().any(|p| cmd.contains(p))
        || config::TEST_CMDS.iter().any(|p| cmd.contains(p))
}
