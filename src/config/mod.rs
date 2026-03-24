// ─── config — static pattern tables and constants ─────────────────────────────
//
// All core values live under config::core/ — one file per category.
// See src/config/core/mod.rs for the complete directory listing.
//
// Legacy modules (commands, extensions, just) are kept for non-pattern config.
// Patterns are re-exported from core:: for backward compat (config::SAFETY etc.)

pub mod core;
pub mod restrictions;

// Re-export core values at config:: level for backward compatibility
pub use core::safety::{SAFETY, DESTRUCTIVE, GIT_SAFETY};
pub use core::hallucination::{HALLUCINATION, HALLUCINATION_ADVISORY};
pub use core::substitutions::SUBSTITUTIONS;
pub use core::advisories::ADVISORIES;
pub use core::zero_trace::{ZERO_TRACE_CMD, ZERO_TRACE_CONTENT, ZERO_TRACE_WRITE, ZERO_TRACE_PATH_EXCLUDE};
pub use core::sensitive_paths::{SENSITIVE_PATHS_DENY, SENSITIVE_PATHS_WARN};
pub use core::injection::INJECTION_PATTERNS;
pub use core::error_hints::ERROR_HINTS;
pub use core::auto_allow::AUTO_ALLOW;
pub use core::thresholds::*;

// Command classification
pub use core::commands::*;

// Configurable detection patterns
pub use core::patterns::*;

// Extensions and Just config (now in core/)
pub use core::extensions::*;
pub use core::just::*;
