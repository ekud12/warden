// ─── Anchor Engine — "Stay Grounded" ─────────────────────────────────────────
//
// Maintains compact session state and prevents drift.
// SLA: <100ms per hook call.
//
// Modules:
//   Compass — drift detection + phase adaptation (5 session phases)
//   Focus   — salience / importance ranking + explore budget
//   Ledger  — session state tracking (turn-by-turn events)
//   Debt    — verification debt tracking (edits since last build/test)
//   Trust   — composite trust score (gates injection budget)
// ──────────────────────────────────────────────────────────────────────────────

pub mod budget;
pub mod compass;
pub mod debt;
pub mod error_prevention;
pub mod focus;
pub mod git_summary;
pub mod ledger;
pub mod postcompact;
pub mod precompact;
pub mod session_end;
pub mod session_start;
pub mod trust;
