// ─── engines — Warden's 4-engine architecture ───────────────────────────────
//
// Reflex  — act now (safety, blocking, substitution)
// Anchor  — stay grounded (session state, drift, verification)
// Dream   — learn quietly (patterns, conventions, repair knowledge)
// Harbor  — connect (assistant adapters, MCP, CLI, tool integrations)
// ──────────────────────────────────────────────────────────────────────────────

pub mod signal;
pub mod signal_bus;
pub mod reflex;
pub mod anchor;
pub mod dream;
pub mod harbor;
