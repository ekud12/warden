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

/// Progressive read advisory turn threshold
pub const PROGRESSIVE_READ_ADVISORY_TURN: u32 = 25;

/// Progressive read deny turn threshold
pub const PROGRESSIVE_READ_DENY_TURN: u32 = 40;
