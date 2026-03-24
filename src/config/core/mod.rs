// ─── config::core — ALL compiled-in rule values in one place ─────────────────
//
// Every deny pattern, advisory, threshold, and constant that ships baked into
// the binary lives under this module. To see ALL core rules at a glance:
//
//   src/config/core/
//   ├── mod.rs              ← this file (re-exports)
//   ├── safety.rs           ← rm -rf, sudo, git mutating ops
//   ├── hallucination.rs    ← reverse shells, credential piping
//   ├── substitutions.rs    ← grep→rg, find→fd, curl→xh
//   ├── advisories.rs       ← docker CLI→MCP, symbol rg→aidex
//   ├── zero_trace.rs       ← AI attribution blocking
//   ├── sensitive_paths.rs  ← .ssh, .gnupg, system dirs
//   ├── injection.rs        ← prompt injection detection
//   ├── error_hints.rs      ← PostToolUseFailure recovery hints
//   ├── auto_allow.rs       ← safe read-only commands
//   └── thresholds.rs       ← MAX_READ_SIZE, MAX_MCP_OUTPUT, etc.
//
// The TOML files (rules/core.toml, rules/community.toml) can EXTEND or
// REPLACE these at runtime. These compiled values are the fallback floor.
// ──────────────────────────────────────────────────────────────────────────────

pub mod advisories;
pub mod auto_allow;
pub mod commands;
pub mod error_hints;
pub mod extensions;
pub mod hallucination;
pub mod injection;
pub mod just;
pub mod patterns;
pub mod safety;
pub mod sensitive_paths;
pub mod substitutions;
pub mod thresholds;
pub mod zero_trace;
