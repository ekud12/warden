// ─── Engine: Anchor — Postcompact ────────────────────────────────────────────
//
// Resets volatile session state fields that become invalid after compaction:
//   - recent_denial_turns — drift warnings based on pre-compaction denials
//   - last_context_hash — context dedup blocks new injections
//   - advisory_cooldowns — advisories stay suppressed when they should re-fire
//   - tool_fingerprints — doom-loop detection carries over invalid fingerprints
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;

pub fn run(raw: &str) {
    let _input = common::parse_input(raw);
    let mut state = common::read_session_state();

    state.recent_denial_turns.clear();
    state.last_context_hash = 0;
    state.advisory_cooldowns.clear();
    state.tool_fingerprints.clear();
    state.last_compaction_turn = state.turn;

    common::write_session_state(&state);
    common::add_session_note(
        "compaction",
        &format!("Context compacted at turn {}", state.turn),
    );
    common::log(
        "postcompact",
        &format!("State reset at turn {}", state.turn),
    );
}
