// ─── core::filters — default command output filter rules ──────────────────────
//
// Data-driven filter rules for the smart_filter engine. Each rule specifies
// how to compress output from a specific command type.
//
// Users can override/extend these in personal.toml or project rules.toml:
//   [[command_filters]]
//   match = "terraform plan"
//   strategy = "keep_matching"
//   keep_patterns = ["Plan:", "to add", "to change", "Error:"]
//   max_lines = 30

use crate::rules::schema::CommandFilter;

/// Compiled default filter rules
pub fn defaults() -> Vec<CommandFilter> {
    vec![
        CommandFilter {
            cmd_match: "cargo test|cargo nextest".into(),
            strategy: "strip_matching".into(),
            keep_patterns: vec![
                "FAILED".into(), "failures:".into(), "test result:".into(),
                "panicked".into(), r"error\[".into(), "---- ".into(),
            ],
            strip_patterns: vec![
                r"\.\.\. ok$".into(), r"\.\.\. ignored$".into(),
            ],
            keep_first: 3, keep_last: 3,
            summary_template: "cargo test ({kept} of {total} lines, failures + summary)".into(),
            max_lines: 40,
        },
        CommandFilter {
            cmd_match: "cargo build|cargo check|cargo clippy".into(),
            strategy: "strip_matching".into(),
            keep_patterns: vec![
                r"error\[".into(), "error:".into(), "warning:".into(),
                "Finished".into(), "-->".into(), " |".into(),
                "could not compile".into(),
            ],
            strip_patterns: vec![
                "^Compiling ".into(), "^Downloading ".into(), "^Downloaded ".into(),
            ],
            keep_first: 2, keep_last: 3,
            summary_template: "cargo build ({kept} of {total} lines, errors + warnings)".into(),
            max_lines: 50,
        },
        CommandFilter {
            cmd_match: "git diff".into(),
            strategy: "keep_matching".into(),
            keep_patterns: vec![
                "^diff --git".into(), "^@@".into(), r"^\+\+\+".into(),
                r"^---".into(), r"^\+".into(), r"^-".into(),
            ],
            strip_patterns: vec![],
            keep_first: 0, keep_last: 0,
            summary_template: "git diff ({kept} of {total} lines)".into(),
            max_lines: 60,
        },
        CommandFilter {
            cmd_match: "git log".into(),
            strategy: "keep_matching".into(),
            keep_patterns: vec![
                "^commit ".into(), "^Author:".into(), "^Date:".into(), "^Merge:".into(),
            ],
            strip_patterns: vec![],
            keep_first: 0, keep_last: 0,
            summary_template: "git log ({kept} of {total} lines, headers + subjects)".into(),
            max_lines: 40,
        },
        CommandFilter {
            cmd_match: "npm install|npm ci|pnpm install|yarn install|bun install".into(),
            strategy: "strip_matching".into(),
            keep_patterns: vec![
                "WARN".into(), "ERR!".into(), "added".into(), "removed".into(),
                "audit".into(), "vulnerabilit".into(), "up to date".into(),
            ],
            strip_patterns: vec![
                "npm http".into(), "fetch ".into(), "GET ".into(),
                "reify".into(), "idealTree".into(), "timing".into(),
            ],
            keep_first: 1, keep_last: 3,
            summary_template: "install ({kept} of {total} lines)".into(),
            max_lines: 30,
        },
        CommandFilter {
            cmd_match: "pytest|vitest|jest|npm test|pnpm test|go test|dotnet test".into(),
            strategy: "strip_matching".into(),
            keep_patterns: vec![
                "(?i)fail".into(), "(?i)error".into(), "assert".into(),
                "expected".into(), "actual".into(), "(?i)suite".into(),
                "(?i)result".into(), "(?i)total".into(),
            ],
            strip_patterns: vec![
                r"(?i)pass".into(), r"✓".into(), r"✔".into(),
            ],
            keep_first: 3, keep_last: 3,
            summary_template: "tests ({kept} of {total} lines, failures + summary)".into(),
            max_lines: 40,
        },
        CommandFilter {
            cmd_match: "eslint|biome|ruff|pylint|mypy|clippy".into(),
            strategy: "keep_matching".into(),
            keep_patterns: vec![
                "error".into(), "warning".into(), "warn".into(),
                r"^\S+\.\w+".into(), // file paths
            ],
            strip_patterns: vec![],
            keep_first: 2, keep_last: 3,
            summary_template: "lint ({kept} of {total} lines)".into(),
            max_lines: 50,
        },
        CommandFilter {
            cmd_match: r"^(ls|eza|tree|fd|find)\s".into(),
            strategy: "head_tail".into(),
            keep_patterns: vec![],
            strip_patterns: vec![],
            keep_first: 30, keep_last: 5,
            summary_template: "listing ({kept} of {total} entries)".into(),
            max_lines: 40,
        },
    ]
}
