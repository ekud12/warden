// ─── common — shared types, I/O helpers, and output writers ───────────────────
//
// Re-exports from sub-modules so all `common::` paths remain valid.
// ──────────────────────────────────────────────────────────────────────────────

pub mod events;
mod io;
mod output;
pub mod sanitize;
pub mod scratch;
mod session;
pub mod shell_parse;
pub mod storage;
pub mod subprocess;
pub mod types;
pub mod util;

// ─── types ───────────────────────────────────────────────────────────────────
pub use types::HookInput;

// ─── output ──────────────────────────────────────────────────────────────────
pub use output::{
    additional_context, allow, allow_with_advisory, allow_with_transform, allow_with_update,
    deny, deny_with_id,
    permission_approve, start_capture, stop_block, take_capture, updated_mcp_output,
};

// ─── io ──────────────────────────────────────────────────────────────────────
pub use io::{
    LogLevel, add_session_note, add_session_note_ext, assistant_rules_dir, hooks_dir, is_ci, log,
    log_structured, parse_input, project_dir, read_tail, set_project_cwd,
};

/// Parse input or return early from handler (used by 10+ handlers)
macro_rules! parse_input_or_return {
    ($raw:expr) => {
        match $crate::common::parse_input($raw) {
            Some(input) => input,
            None => return,
        }
    };
}
pub(crate) use parse_input_or_return;

/// Get the project CWD (re-exported for rules module access)
pub fn io_get_project_cwd() -> String {
    io::get_project_cwd()
}

// ─── session ─────────────────────────────────────────────────────────────────
pub use session::{
    CommandEntry, FileReadEntry, SessionState, TurnSnapshot, read_session_state,
    write_session_state,
};

/// Enable all daemon-mode optimizations (session cache + log buffering)
pub fn enable_daemon_mode() {
    session::enable_daemon_mode();
    io::enable_log_buffering();
}

/// Flush all daemon-mode buffers (session state + logs)
pub fn flush_daemon_buffers() {
    session::flush_session_cache();
    io::flush_log_buffer();
}

// ─── util ────────────────────────────────────────────────────────────────────
pub use util::{
    content_hash, detect_suspicious_chars, file_mtime, normalize_path, now_iso, string_hash,
    strip_ansi, truncate,
};
