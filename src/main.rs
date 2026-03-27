#![allow(dead_code)] // Application binary: many functions are called indirectly via handler dispatch
// ─── warden — AI Coding Session Guardian ─────────────────────────────────────
//
// Single-binary runtime intelligence layer for AI coding assistants.
// Supports Claude Code and Gemini CLI through adapter pattern.
// All hook handlers dispatched as subcommands.
//
// Architecture (4-engine model):
//   - Reflex  — act now (safety, blocking, substitution)
//   - Anchor  — stay grounded (session state, drift, verification)
//   - Dream   — learn quietly (patterns, conventions, repair knowledge)
//   - Harbor  — connect (assistant adapters, MCP, CLI, tool integrations)
//
// Design: every handler exits 0 on error to never block the AI assistant.
// ──────────────────────────────────────────────────────────────────────────────

mod analytics;
mod assistant;
mod cli;
mod common;
mod config;
mod constants;
mod engines;
mod handlers;
mod install;
mod runtime;
mod rules;
mod benchmark;
mod scorecard;

fn main() {
    // Capture backtraces on panic for post-mortem debugging
    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let msg = format!("PANIC: {}\n{}", info, bt);
        eprintln!("{}", msg);
        // Write to panic.log in project log dir if available
        let project_dir = common::project_dir();
        let log_path = project_dir.join("panic.log");
        let _ = std::fs::write(log_path, &msg);
    }));

    let args: Vec<String> = std::env::args().collect();
    let subcmd = args.get(1).map(|s| s.as_str()).unwrap_or("");

    // Daemon server mode — handle before dispatch routing
    if subcmd == "daemon" {
        let mtime: u64 = args
            .get(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(runtime::ipc::get_binary_mtime);
        runtime::daemon::run_server(mtime);
        return;
    }

    if runtime::dispatch::is_hook(subcmd) {
        runtime::dispatch::run_hook(subcmd, &args);
    } else {
        cli::run(subcmd, &args);
    }
}
