// ─── analytics::recovery — CLI command knowledge base ────────────────────────
//
// Baked-in knowledge of common CLI tools, their flags, common mistakes,
// and alternatives. Used by PostToolUseFailure to suggest corrections.
//
// Two modes:
//   A. "Unknown argument" → suggest correct flag
//   B. "Command not found" → suggest install or alternative
// ──────────────────────────────────────────────────────────────────────────────

/// A known CLI tool with install commands and common flag mistakes
pub struct CliTool {
    pub name: &'static str,
    pub install: &'static [&'static str],
    pub common_mistakes: &'static [FlagFix],
    pub alternatives: &'static [&'static str],
}

/// A known flag mistake: wrong flag → correct flag + explanation
pub struct FlagFix {
    pub wrong: &'static str,
    pub correct: &'static str,
    pub hint: &'static str,
}

pub static CLI_KNOWLEDGE: &[CliTool] = &[
    CliTool {
        name: "eza",
        install: &["cargo install eza", "brew install eza", "scoop install eza"],
        common_mistakes: &[
            FlagFix {
                wrong: "--dirs-only",
                correct: "-D",
                hint: "Use -D or --only-dirs",
            },
            FlagFix {
                wrong: "--tree-level",
                correct: "--level",
                hint: "Use --level N",
            },
            FlagFix {
                wrong: "--all-files",
                correct: "-a",
                hint: "Use -a or --all",
            },
            FlagFix {
                wrong: "--directories",
                correct: "-D",
                hint: "Use -D or --only-dirs",
            },
        ],
        alternatives: &["ls", "exa"],
    },
    CliTool {
        name: "rg",
        install: &[
            "cargo install ripgrep",
            "brew install ripgrep",
            "scoop install ripgrep",
        ],
        common_mistakes: &[
            FlagFix {
                wrong: "--include",
                correct: "-g",
                hint: "Use -g '*.ext' for file glob",
            },
            FlagFix {
                wrong: "--recursive",
                correct: "",
                hint: "rg is recursive by default",
            },
            FlagFix {
                wrong: "-r",
                correct: "",
                hint: "rg is recursive by default; -r is for replace",
            },
        ],
        alternatives: &["grep"],
    },
    CliTool {
        name: "fd",
        install: &[
            "cargo install fd-find",
            "brew install fd",
            "scoop install fd",
        ],
        common_mistakes: &[
            FlagFix {
                wrong: "--name",
                correct: "",
                hint: "fd matches names by default: fd PATTERN",
            },
            FlagFix {
                wrong: "-iname",
                correct: "",
                hint: "fd is case-insensitive by default",
            },
            FlagFix {
                wrong: "--type file",
                correct: "--type f",
                hint: "Use -t f (short form)",
            },
        ],
        alternatives: &["find"],
    },
    CliTool {
        name: "bat",
        install: &["cargo install bat", "brew install bat", "scoop install bat"],
        common_mistakes: &[
            FlagFix {
                wrong: "--numbers",
                correct: "-n",
                hint: "Use -n or --number",
            },
            FlagFix {
                wrong: "--syntax",
                correct: "-l",
                hint: "Use -l LANG or --language LANG",
            },
        ],
        alternatives: &["cat"],
    },
    CliTool {
        name: "dust",
        install: &[
            "cargo install du-dust",
            "brew install dust",
            "scoop install dust",
        ],
        common_mistakes: &[FlagFix {
            wrong: "--human-readable",
            correct: "",
            hint: "dust is human-readable by default",
        }],
        alternatives: &["du"],
    },
    CliTool {
        name: "just",
        install: &[
            "cargo install just",
            "brew install just",
            "scoop install just",
        ],
        common_mistakes: &[FlagFix {
            wrong: "--file",
            correct: "-f",
            hint: "Use -f or --justfile",
        }],
        alternatives: &["make"],
    },
    CliTool {
        name: "xh",
        install: &["cargo install xh", "brew install xh", "scoop install xh"],
        common_mistakes: &[FlagFix {
            wrong: "--data",
            correct: "",
            hint: "xh sends data as request body directly",
        }],
        alternatives: &["curl", "httpie"],
    },
    CliTool {
        name: "ouch",
        install: &["cargo install ouch", "brew install ouch"],
        common_mistakes: &[
            FlagFix {
                wrong: "--extract",
                correct: "d",
                hint: "Use: ouch d archive.tar.gz (d = decompress)",
            },
            FlagFix {
                wrong: "--compress",
                correct: "c",
                hint: "Use: ouch c files... output.tar.gz",
            },
        ],
        alternatives: &["tar", "zip", "unzip"],
    },
    CliTool {
        name: "huniq",
        install: &["cargo install huniq"],
        common_mistakes: &[],
        alternatives: &["sort | uniq", "sort -u"],
    },
    CliTool {
        name: "procs",
        install: &[
            "cargo install procs",
            "brew install procs",
            "scoop install procs",
        ],
        common_mistakes: &[],
        alternatives: &["ps"],
    },
    CliTool {
        name: "sd",
        install: &["cargo install sd", "brew install sd"],
        common_mistakes: &[FlagFix {
            wrong: "",
            correct: "",
            hint: "sd is blocked on Windows (mangles newlines). Use Edit tool instead.",
        }],
        alternatives: &["sed"],
    },
    CliTool {
        name: "outline",
        install: &["cargo install code-outline"],
        common_mistakes: &[],
        alternatives: &["bat --style=header", "head -50"],
    },
    CliTool {
        name: "tokei",
        install: &[
            "cargo install tokei",
            "brew install tokei",
            "scoop install tokei",
        ],
        common_mistakes: &[],
        alternatives: &["cloc", "scc"],
    },
];

/// Check stderr for "command not found" and suggest install/alternative
pub fn check_not_found(stderr: &str) -> Option<String> {
    // Extract command name from common error patterns
    let cmd_name = extract_missing_command(stderr)?;

    for tool in CLI_KNOWLEDGE {
        if tool.name == cmd_name || tool.alternatives.contains(&cmd_name) {
            // It's a known tool
            if !tool.install.is_empty() {
                return Some(format!(
                    "'{}' is not installed. Install: {}\nAlternative: {}",
                    cmd_name,
                    tool.install[0],
                    if !tool.alternatives.is_empty() {
                        tool.alternatives[0]
                    } else {
                        "none"
                    },
                ));
            }
        }

        // Check if the missing command IS the alternative (user has the modern tool)
        if tool.alternatives.contains(&cmd_name) {
            return Some(format!(
                "'{}' not found. Use '{}' instead (already installed if Warden substitutions are active).",
                cmd_name, tool.name,
            ));
        }
    }

    None
}

/// Check stderr for "unknown argument" and suggest correct flag
pub fn check_bad_flag(cmd: &str, stderr: &str) -> Option<String> {
    // Extract the base command name
    let base_cmd = cmd.split_whitespace().next()?;

    for tool in CLI_KNOWLEDGE {
        if tool.name != base_cmd {
            continue;
        }

        // Check each known mistake
        for fix in tool.common_mistakes {
            if !fix.wrong.is_empty() && (cmd.contains(fix.wrong) || stderr.contains(fix.wrong)) {
                return if fix.correct.is_empty() {
                    Some(format!("{}: {}", tool.name, fix.hint))
                } else {
                    Some(format!(
                        "{}: '{}' → '{}'. {}",
                        tool.name, fix.wrong, fix.correct, fix.hint
                    ))
                };
            }
        }
    }

    None
}

/// Extract the missing command name from stderr
fn extract_missing_command(stderr: &str) -> Option<&str> {
    // Pattern: "command not found: foo"
    if let Some(idx) = stderr.find("command not found:") {
        let after = stderr[idx + 18..].trim();
        return after.split_whitespace().next();
    }
    // Pattern: "foo: command not found"
    if let Some(idx) = stderr.find(": command not found") {
        let before = &stderr[..idx];
        return before
            .split_whitespace()
            .last()
            .map(|s| s.trim_matches('\'').trim_matches('"'));
    }
    // Pattern: "'foo' is not recognized"
    if let Some(idx) = stderr.find("is not recognized") {
        let before = &stderr[..idx].trim();
        let word = before.split_whitespace().last().unwrap_or(before);
        let clean = word.trim_matches('\'').trim_matches('"');
        if !clean.is_empty() {
            return Some(clean);
        }
    }
    // Pattern: "not found in PATH"
    if stderr.contains("not found in PATH") || stderr.contains("No such file") {
        return stderr
            .split_whitespace()
            .next()
            .map(|s| s.trim_matches('\'').trim_matches('"'));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_command_not_found() {
        let result = check_not_found("bash: outline: command not found");
        assert!(result.is_some());
        assert!(result.unwrap().contains("code-outline"));
    }

    #[test]
    fn detect_bad_flag_eza() {
        let result = check_bad_flag("eza --dirs-only /path", "Unknown argument --dirs-only");
        assert!(result.is_some());
        let msg = result.unwrap();
        assert!(msg.contains("-D"), "should suggest -D, got: {}", msg);
    }

    #[test]
    fn detect_bad_flag_rg() {
        let result = check_bad_flag("rg --include '*.rs' pattern", "");
        assert!(result.is_some());
        assert!(result.unwrap().contains("-g"));
    }

    #[test]
    fn unknown_command_returns_none() {
        let result = check_not_found("some random error message");
        assert!(result.is_none());
    }
}
