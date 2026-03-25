// ─── core::thresholds — compiled default threshold values ────────────────────

/// Maximum file size for Read governance (bytes). Files larger are denied.
pub const MAX_READ_SIZE: u64 = 50_000;

/// Maximum MCP output size (bytes, turn 0-15)
pub const MAX_MCP_OUTPUT: usize = 15_000;

/// Maximum string length before trimming in MCP output
pub const MAX_STRING_LEN: usize = 2_000;

/// Head retention when trimming strings
pub const STRING_KEEP_HEAD: usize = 1_000;

/// Tail retention when trimming strings
pub const STRING_KEEP_TAIL: usize = 500;

/// Maximum array length before trimming in MCP output
pub const MAX_ARRAY_LEN: usize = 30;

/// Head retention when trimming arrays
pub const ARRAY_KEEP_FIRST: usize = 20;

/// Tail retention when trimming arrays
pub const ARRAY_KEEP_LAST: usize = 5;

/// Warn when session exceeds this many turns (context pressure)
pub const MAX_SESSION_TURNS_WARN: u32 = 150;

/// Warn when more than this many files edited in one session
pub const MAX_FILES_EDITED_WARN: u32 = 40;

/// Default cost budget per session (USD, blended rate)
pub const COST_BUDGET_DEFAULT: f64 = 5.0;

/// Default blended token rate ($/1M tokens — roughly Opus input+output average)
pub const TOKEN_RATE_PER_MILLION: f64 = 9.0;

/// Progressive read advisory turn threshold (matches runtime default in rules/mod.rs)
pub const PROGRESSIVE_READ_ADVISORY_TURN: u32 = 50;

/// Progressive read deny turn threshold (matches runtime default in rules/mod.rs)
pub const PROGRESSIVE_READ_DENY_TURN: u32 = 80;

// ─── Injection budget thresholds ──────────────────────────────────────────────

/// Trust score above which session is "very healthy" — inject top 1 advisory only
pub const TRUST_BUDGET_HIGH: u32 = 85;
/// Trust score above which session is "normal" — inject top 3
pub const TRUST_BUDGET_NORMAL: u32 = 50;
/// Trust score above which session is "degraded" — inject top 5
pub const TRUST_BUDGET_DEGRADED: u32 = 25;
// Below TRUST_BUDGET_DEGRADED: uncapped (struggling)

// ─── Trust score weights ──────────────────────────────────────────────────────

pub const TRUST_WEIGHT_ERRORS: i32 = 5;
pub const TRUST_WEIGHT_VERIFICATION_DEBT: i32 = 3;
pub const TRUST_WEIGHT_SUBSYSTEM_SWITCHES: i32 = 2;
pub const TRUST_WEIGHT_DEAD_ENDS: i32 = 4;
pub const TRUST_WEIGHT_CHECKPOINT_GAP: i32 = 1;
pub const TRUST_WEIGHT_RECENT_DENIALS: i32 = 3;
pub const TRUST_MILESTONE_BONUS: i32 = 10;
pub const TRUST_BALANCE_BONUS: i32 = 5;

// ─── Dream state ──────────────────────────────────────────────────────────────

/// Effectiveness learning rate: how much to adjust per advisory→milestone observation
pub const DREAM_LEARNING_RATE: f64 = 0.1;

// ─── Scorecard weights ────────────────────────────────────────────────────────

pub const SCORECARD_BASELINE: i32 = 50;
pub const SCORECARD_SAFETY_BONUS: i32 = 20;
pub const SCORECARD_FP_PENALTY: i32 = 10;
pub const SCORECARD_EFFICIENCY_HIGH: i32 = 15;
pub const SCORECARD_EFFICIENCY_MED: i32 = 10;
pub const SCORECARD_MILESTONE_PER: i32 = 3;
pub const SCORECARD_LOOP_PENALTY: i32 = 5;
pub const SCORECARD_SILENCE_BONUS_HIGH: i32 = 10;
pub const SCORECARD_SILENCE_BONUS_MED: i32 = 5;
pub const SCORECARD_REPEAT_PENALTY: i32 = 5;
