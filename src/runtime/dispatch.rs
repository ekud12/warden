// ─── dispatch — hook subcommand routing ──────────────────────────────────────
//
// Routes hook events (pretool-bash, session-start, etc.) to the correct handler.
// Includes stdin reading, daemon fast-path, CI mode, and panic isolation.
// ──────────────────────────────────────────────────────────────────────────────

#[allow(unused_imports)]
use crate::runtime;
use crate::{common, constants, engines, handlers}; // Used when daemon feature is active

const HOOK_SUBCMDS: &[&str] = &[
    "pretool-bash",
    "pretool-read",
    "pretool-write",
    "pretool-redirect",
    "permission-approve",
    "posttool-session",
    "posttool-mcp",
    "session-start",
    "session-end",
    "precompact-memory",
    "postcompact",
    "stop-check",
    "userprompt-context",
    "subagent-context",
    "subagent-stop",
    "postfailure-guide",
    "task-completed",
];

pub fn is_hook(subcmd: &str) -> bool {
    HOOK_SUBCMDS.contains(&subcmd)
}

pub fn run_hook(subcmd: &str, args: &[String]) {
    let _ = args;

    // Set per-project CWD
    if let Ok(cwd) = std::env::current_dir() {
        common::set_project_cwd(&cwd.to_string_lossy());
    }

    // CI mode: safety rules only, skip analytics
    if common::is_ci() {
        let raw = read_stdin();
        dispatch_hook_ci(subcmd, &raw);
        return;
    }

    // v2.4: Direct execution. The relay handles IPC to the server.
    // When called inside the server process, this runs with cached state.
    // When called as fallback (relay couldn't reach server), this runs cold.
    let raw = read_stdin();
    dispatch_hook(subcmd, &raw);
}

fn read_stdin() -> String {
    use std::io::Read;
    use std::time::Duration;
    // Timeout-protected stdin read. Spawns a reader thread and waits up to 5s.
    // Prevents cold-start hang: on first hook call after install/update, Claude Code
    // may not have written stdin yet (mutual wait on the stdin channel).
    // After 5s with no data, returns empty → hook exits 0 (fail-open).
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 1_048_576];
        let n = std::io::stdin().read(&mut buf).unwrap_or(0);
        let _ = tx.send(String::from_utf8_lossy(&buf[..n]).to_string());
    });
    rx.recv_timeout(Duration::from_secs(5)).unwrap_or_default()
}

/// CI mode dispatch: safety-critical handlers only, no analytics or session tracking.
fn dispatch_hook_ci(subcmd: &str, raw: &str) {
    match subcmd {
        "pretool-bash" => handlers::pretool_bash::run(raw),
        "pretool-write" => handlers::pretool_write::run(raw),
        "pretool-redirect" => handlers::pretool_redirect::run(raw),
        "postfailure-guide" => handlers::postfailure_guide::run(raw),
        _ => {} // Skip analytics, session tracking, context injection in CI
    }
}

/// Safety-critical handlers that deny dangerous operations. On panic, these fail CLOSED
/// (exit 1 = deny) to prevent a crash from silently disabling enforcement.
const SAFETY_CRITICAL: &[&str] = &[
    "pretool-bash",
    "pretool-write",
    "pretool-read",
    "pretool-redirect",
    "permission-approve",
];

fn dispatch_hook(subcmd: &str, raw: &str) {
    // Panic isolation: catch_unwind ensures a panicking handler never crashes the AI process.
    // Safety-critical handlers fail CLOSED on panic (exit 1 = deny).
    // Advisory handlers fail OPEN on panic (exit 0 = allow).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match subcmd {
        "pretool-bash" => handlers::pretool_bash::run(raw),
        "pretool-read" => handlers::pretool_read::run(raw),
        "pretool-write" => handlers::pretool_write::run(raw),
        "pretool-redirect" => handlers::pretool_redirect::run(raw),
        "permission-approve" => handlers::permission_approve::run(raw),
        "posttool-session" => engines::anchor::ledger::run(raw),
        "posttool-mcp" => handlers::posttool_mcp::run(raw),
        "session-start" => engines::anchor::session_start::run(raw),
        "session-end" => engines::anchor::session_end::run(raw),
        "precompact-memory" => engines::anchor::precompact::run(raw),
        "postcompact" => engines::anchor::postcompact::run(raw),
        "stop-check" => handlers::stop_check::run(raw),
        "userprompt-context" => handlers::userprompt_context::run(raw),
        "subagent-context" => handlers::subagent_context::run(raw),
        "subagent-stop" => handlers::subagent_stop::run(raw),
        "postfailure-guide" => handlers::postfailure_guide::run(raw),
        "task-completed" => handlers::task_completed::run(raw),
        _ => {}
    }));
    if result.is_err() {
        let is_safety = SAFETY_CRITICAL.contains(&subcmd);
        let mode = if is_safety {
            "closed (deny)"
        } else {
            "open (allow)"
        };
        eprintln!(
            "{}: handler '{}' panicked — failing {}",
            constants::NAME,
            subcmd,
            mode
        );
        // Flight recorder: log panics for post-mortem analysis
        common::storage::append_diagnostic(
            "panic",
            &format!("handler '{}' panicked — fail-{}", subcmd, mode),
        );
        if is_safety {
            std::process::exit(1);
        }
    }
}
