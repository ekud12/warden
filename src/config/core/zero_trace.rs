// ─── core::zero_trace — AI attribution blocking patterns ─────────────────────
//
// OFF by default. Most users want AI attribution (Co-Authored-By, etc.)
// Power users enable via personal.toml: [zero_trace] content_pattern = "..."
// When disabled (empty string), the regex won't match anything.

/// Content patterns: blocks AI attribution in file writes
/// Empty by default — enable in personal.toml
pub const ZERO_TRACE_CONTENT: &str = "";

/// Command patterns: blocks AI attribution in echo/printf/tee
/// Empty by default — enable in personal.toml
pub const ZERO_TRACE_CMD: &str = "";

/// Write operation patterns (echo, printf, tee, >>)
pub const ZERO_TRACE_WRITE: &str = r"(echo|printf|tee|>>)";

/// Path exclusions: don't trigger zero-trace in .claude/ directories
pub const ZERO_TRACE_PATH_EXCLUDE: &str = r"(?i)[/\\.]claude[/\\]";
