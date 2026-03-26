// ─── Sentinel — Unsafe action detection ──────────────────────────────────────
//
// Matches against safety + hallucination + destructive + zero-trace patterns.
// Source: pretool_bash/safety.rs, pretool_bash/hallucination.rs
//
// Future: consolidate all pattern-matching safety checks into this module.
// Currently delegates to existing handlers via pretool_bash pipeline.
// ──────────────────────────────────────────────────────────────────────────────
