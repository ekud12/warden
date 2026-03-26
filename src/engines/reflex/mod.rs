// ─── Reflex Engine — "Act Now" ───────────────────────────────────────────────
//
// Immediate, deterministic, zero-latency decisions.
// SLA: <50ms per hook call.
//
// Modules:
//   Sentinel    — unsafe action detection (safety + hallucination patterns)
//   Loopbreaker — repetition / doom-loop detection (dedup + entropy + spirals)
//   Tripwire    — high-risk pattern detection (injection, expansion bypass)
//   Gatekeeper  — central decision trait (all signals → single Verdict)
// ──────────────────────────────────────────────────────────────────────────────

pub mod sentinel;
pub mod loopbreaker;
pub mod entropy;
pub mod tripwire;
pub mod gatekeeper;
