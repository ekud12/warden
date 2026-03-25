// ─── analytics — runtime intelligence engine ─────────────────────────────────
//
// All analytics features run automatically during the session.
// Updated every turn via userprompt-context, every tool call via posttool-session.
// Users opt-out per feature via config.toml [telemetry] section.
//
// Modules:
//   anomaly    — Welford's online mean/variance, z-score flagging
//   forecast   — Linear regression for token budget / compaction ETA
//   dna        — Per-project statistical fingerprint
//   cost       — Token cost categorization and tracking
//   effectiveness — Per-rule fire count + quality delta scoring
//   quality    — Session quality prediction from early snapshots
//   recovery   — CLI command knowledge base (flag fixes, install suggestions)
// ──────────────────────────────────────────────────────────────────────────────

pub mod anomaly;
pub mod dna;
pub mod effectiveness;
pub mod entropy;
pub mod error_prevention;
pub mod focus;
pub mod forecast;
pub mod goal;
pub mod loop_patterns;
pub mod markov;
pub mod negative_memory;
pub mod quality;
pub mod recovery;
pub mod trust;
pub mod verification;
