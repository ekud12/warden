// ─── core::patterns — configurable detection patterns ─────────────────────────
//
// Patterns used by handlers for build/error/milestone detection, truncation
// filtering, protected branches, and other heuristics. All defined here as
// compiled defaults, overrideable via the TOML merge chain.

/// Protected branch names — working on these triggers a warning.
/// Override in personal.toml: [protected_branches] patterns = ["develop", "trunk"]
pub const PROTECTED_BRANCHES: &[&str] = &["main", "master"];

/// Build success milestone patterns (matched against Bash stdout)
pub const MILESTONE_PATTERNS: &[&str] = &[
    r"(?i)build\s+(succeeded|successful|passed|complete)",
    r"(?i)tests?\s+(passed|succeeded|ok|complete)",
    r"(?i)tsc.*found\s+0\s+error",
    r"(?i)lint\s*(passed|clean|ok)",
    r"(?i)cargo\s+(build|check|test).*Finished",
    r"(?i)npm\s+run\s+build.*done",
    r"(?i)0\s+error",
    r"Compiling.*Finished",
    r"(?i)deploy(ed|ment)\s+(success|complete)",
    r"(?i)health\s+check\s+(pass|ok|200)",
];

/// Build/test error patterns (matched against Bash stdout+stderr)
pub const ERROR_PATTERNS: &[&str] = &[
    r"TS\d{4}:",                                   // TypeScript errors
    r"(?i)error\[E\d+\]",                          // Rust compiler errors
    r"(?i)(FAIL|FAILED|failing)\b",                // Test failures
    r"(?i)build\s+failed",                         // Build failures
    r"(?i)(ERR!|npm\s+ERR)",                       // npm errors
    r"(?i)permission\s+denied",                    // Permission errors
    r"(?i)command\s+not\s+found",                  // Missing tools
    r"(?i)(SyntaxError|TypeError|ReferenceError)", // JS/TS runtime errors
    r"(?i)panic(ked)?(\s+at)?",                    // Rust panics
    r"(?i)segmentation\s+fault",                   // Segfaults
    r"(?i)out\s+of\s+memory",                      // OOM
];

/// Build command detection patterns (determines last_build_turn)
pub const BUILD_COMMANDS: &[&str] = &[
    r"^\s*cargo\s+(build|check|test|clippy)\b",
    r"^\s*npm\s+(run\s+)?(build|test|lint|check)\b",
    r"^\s*pnpm\s+(run\s+)?(build|test|lint)\b",
    r"^\s*yarn\s+(run\s+)?(build|test|lint)\b",
    r"^\s*bun\s+(run\s+)?(build|test)\b",
    r"^\s*dotnet\s+(build|test|publish)\b",
    r"^\s*make\s+(build|test|check|lint)\b",
    r"^\s*just\s+(build|test|check|lint)\b",
    r"^\s*pytest\b",
    r"^\s*tsc\b",
    r"^\s*eslint\b",
    r"^\s*ruff\s+check\b",
];

/// Truncation filter: failure line patterns
pub const TRUNCATE_FAIL: &str = r"(?i)FAIL|FAILED|ERROR|✗|✘|failing|panicked|assertion|×|BROKEN";

/// Truncation filter: test summary patterns
pub const TRUNCATE_SUMMARY: &str =
    r"(?i)Tests?:?\s*\d|test result:|passed|failed|ok\s*\(|Ran \d|TOTAL";

/// Truncation filter: build-important line patterns
pub const TRUNCATE_BUILD_IMPORTANT: &str =
    r"(?i)error|warning|warn\[|ERR!|FATAL|failed|succeeded|Build|Finished|Compiling.*error";

/// Truncation filter: install error patterns
pub const TRUNCATE_INSTALL_ERROR: &str =
    r"(?i)error|ERR!|WARN|warning|deprecated|vulnerability|audit";

/// Truncation filter: default important line patterns
pub const TRUNCATE_DEFAULT_IMPORTANT: &str =
    r"(?i)error|warn(ing)?|fail(ed|ure)?|ERR!|FATAL|exception|assert|passed|succeeded|skipped";

/// Knip finding patterns (unused exports/imports)
pub const KNIP_PATTERNS: &[&str] = &[
    r"(?i)unused\s+(export|file|dependency|type)",
    r"(?i)unresolved\s+(import|module)",
];

// ── PostToolUse:Bash session tracking patterns ──────────────────────────────
// Used by posttool_session/bash.rs (compiled once via LazyLock)

pub const BASH_TS_ERROR: &str = r"TS\d{4}";
pub const BASH_LINT_COUNT: &str = r"(\d+) errors?";
pub const BASH_DEP_FAILURE: &str = r"npm ERR!|ERESOLVE|peer dep";
pub const BASH_TEST_FAIL: &str = r"(?i)FAIL|failed|✗|✘|\d+ failing";
pub const BASH_TEST_COUNT: &str = r"(\d+) (?:failing|failed)";
pub const BASH_BUILD_FAIL: &str = r"(?i)error\b.*\bbuild\b|\bbuild\b.*\berror\b|Build FAILED";
pub const BASH_PERM_ERROR: &str = r"EACCES|Permission denied|EPERM";
pub const BASH_MISSING_TOOL: &str = r"command not found|is not recognized|not found in PATH";
pub const BASH_MISSING_TOOL_NAME: &str = r#"['"]?(\S+)['"]?:?\s*(?:command )?not found"#;
pub const BASH_KNIP_CHECK: &str = r"(?i)unused|unresolved";
pub const BASH_KNIP_COUNT: &str = r"(\d+) (?:unused|unresolved)";
pub const BASH_BUILD_SUCCESS: &str = r"(?i)Build succeeded|Finished.*profile|Finished.*target|Successfully compiled|webpack compiled|build completed successfully";
pub const BASH_TEST_PASS: &str =
    r"(?i)test result:\s*ok|✓|✔|\d+ passing|\d+ passed|Tests:\s*\d+ passed|all \d+ tests? passed";
pub const BASH_KNIP_DIRTY: &str = r"(?i)unused|unresolved|error";
pub const BASH_NO_CIRCULAR: &str = r"(?i)no circular";
pub const BASH_GIT_COMMIT: &str = r"git\s+commit";
pub const BASH_COMMIT_MSG: &str = r#"-m\s+["']([^"']+)"#;
pub const BASH_ERROR_LOC: &str =
    r"(?m)^.*?(\S+\.\w+)[:\(](\d+)[,:\)].*?(?:error|Error)\S*:?\s*(.{0,60})";
pub const BASH_TRUNCATION_MARKER: &str =
    r"--- (?:TRUNCATED|TEST OUTPUT|BUILD OUTPUT|INSTALL OUTPUT) \((?:filtered: )?(\d+) lines";
