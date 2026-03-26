// ─── analytics — runtime intelligence engine ─────────────────────────────────
//
// Non-engine analytics modules that remain here:
//   forecast   — Linear regression for token budget / compaction ETA
//   goal       — Session goal tracking and topic coherence
//   markov     — Action prediction via transition probabilities
//   quality    — Session quality prediction from early snapshots
//   recovery   — CLI command knowledge base (flag fixes, install suggestions)
//
// Migrated to engines:
//   anomaly → engines::dream::imprint
//   dna → engines::dream::dna
//   effectiveness → engines::dream::pruner
//   entropy → engines::reflex::entropy
//   error_prevention → engines::anchor::error_prevention
//   focus → engines::anchor::focus
//   loop_patterns → engines::reflex::loopbreaker
//   trust → engines::anchor::trust
//   verification → engines::anchor::debt
// ──────────────────────────────────────────────────────────────────────────────

pub mod forecast;
pub mod goal;
pub mod markov;
pub mod quality;
pub mod recovery;
