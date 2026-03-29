#![allow(dead_code)] // Handler dispatch is indirect; serialized structs need fields for deserialization
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
mod benchmark;
mod cli;
mod common;
mod config;
mod constants;
mod engines;
mod handlers;
mod install;
mod rules;
mod runtime;
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

    // Persistent server mode (v2.4 unified architecture)
    // Spawned by relay on first hook call. Stays alive, handles all hooks via IPC.
    if subcmd == "__server" || subcmd == "daemon" {
        runtime::server::run();
        return;
    }

    if runtime::dispatch::is_hook(subcmd) {
        runtime::dispatch::run_hook(subcmd, &args);
    } else {
        cli::run(subcmd, &args);
    }
}
