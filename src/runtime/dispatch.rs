// ─── dispatch — hook subcommand routing ──────────────────────────────────────
//
// Routes hook events (pretool-bash, session-start, etc.) to the correct handler.
// Includes stdin reading, daemon fast-path, CI mode, and panic isolation.
// ──────────────────────────────────────────────────────────────────────────────

use crate::{common, constants, engines, handlers, runtime};
use std::process;

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
    let _ = args; // args available for future use; currently stdin-driven

    // Set per-project CWD
    if let Ok(cwd) = std::env::current_dir() {
        common::set_project_cwd(&cwd.to_string_lossy());
    }

    // CI mode: safety rules only, skip daemon, skip analytics
    if common::is_ci() {
        let raw = read_stdin();
        dispatch_hook_ci(subcmd, &raw);
        return;
    }

    // IPC fast-path: try daemon first unless WARDEN_NO_DAEMON is set
    if std::env::var("WARDEN_NO_DAEMON").is_err() {
        let raw = read_stdin();
        if let Some(resp) = runtime::ipc::try_daemon(subcmd, &raw) {
            if resp.exit_code == runtime::ipc::EXIT_RESTART {
                // Daemon detected rebuild — fall through to direct execution
                dispatch_hook(subcmd, &raw);
            } else {
                if !resp.stdout.is_empty() {
                    print!("{}", resp.stdout);
                }
                process::exit(resp.exit_code);
            }
        } else {
            // Daemon not available — direct execution (fast path).
            // Spawn daemon in background for *next* call (fire-and-forget).
            // This avoids the old 3x150ms retry loop that added 450ms latency.
            std::thread::spawn(runtime::ipc::spawn_daemon);
            dispatch_hook(subcmd, &raw);
        }
    } else {
        let raw = read_stdin();
        dispatch_hook(subcmd, &raw);
    }
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

fn dispatch_hook(subcmd: &str, raw: &str) {
    // Panic isolation: catch_unwind ensures a panicking handler never blocks the AI.
    // Fail open: on panic, log error and exit 0.
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
        eprintln!(
            "{}: handler '{}' panicked — failing open",
            constants::NAME,
            subcmd
        );
        // Flight recorder: log panics for post-mortem analysis
        common::storage::append_diagnostic("panic", &format!("handler '{}' panicked", subcmd));
    }
}
