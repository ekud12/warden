// ─── engines — Warden's 4-engine architecture ───────────────────────────────
//
// Reflex  — act now (safety, blocking, substitution)
// Anchor  — stay grounded (session state, drift, verification)
// Dream   — learn quietly (patterns, conventions, repair knowledge)
// Harbor  — connect (assistant adapters, MCP, CLI, tool integrations)
// ──────────────────────────────────────────────────────────────────────────────

pub mod anchor;
pub mod dream;
pub mod harbor;
pub mod reflex;
pub mod signal;
pub mod signal_bus;

use signal::Signal;

/// Context passed to each engine's `process()` method.
pub struct EngineContext<'a> {
    /// The command or action being evaluated.
    pub cmd: &'a str,
    /// Tool output (for PostToolUse events).
    pub output: &'a str,
    /// Hook event type (e.g., "PreToolUse", "PostToolUse", "UserPromptSubmit").
    pub event: &'a str,
    /// Maximum time budget in milliseconds.
    pub budget_ms: u32,
}

/// Trait implemented by each of warden's four engines.
/// Each engine produces Signal values from the given context.
pub trait Engine: Send + Sync {
    /// Engine name for logging and tracing.
    fn name(&self) -> &'static str;

    /// Process the context and return signals.
    /// Signals compete for the injection budget and feed the Gatekeeper.
    fn process(&self, ctx: &EngineContext) -> Vec<Signal>;
}
