// ─── posttool_session::bash — Bash command state tracking ────────────────────
//
// Performance: all regexes compiled once via LazyLock (reused across daemon
// requests). Session state read once + written once per call.

use crate::common;
use crate::config;
use regex::Regex;
use std::sync::LazyLock;

// ── Compiled regex patterns (initialized once, reused across daemon requests) ──

struct SessionRegexes {
    // Error patterns
    ts_error: Regex,
    lint_count: Regex,
    dep_failure: Regex,
    test_fail: Regex,
    test_count: Regex,
    build_fail: Regex,
    perm_error: Regex,
    missing_tool: Regex,
    missing_tool_name: Regex,
    knip_check: Regex,
    knip_count: Regex,
    // Milestone patterns
    build_success: Regex,
    test_pass: Regex,
    knip_dirty: Regex,
    no_circular: Regex,
    git_commit: Regex,
    commit_msg: Regex,
}

static RE: LazyLock<SessionRegexes> = LazyLock::new(|| SessionRegexes {
    ts_error: Regex::new(config::BASH_TS_ERROR).expect("regex: ts_error"),
    lint_count: Regex::new(config::BASH_LINT_COUNT).expect("regex: lint_count"),
    dep_failure: Regex::new(config::BASH_DEP_FAILURE).expect("regex: dep_failure"),
    test_fail: Regex::new(config::BASH_TEST_FAIL).expect("regex: test_fail"),
    test_count: Regex::new(config::BASH_TEST_COUNT).expect("regex: test_count"),
    build_fail: Regex::new(config::BASH_BUILD_FAIL).expect("regex: build_fail"),
    perm_error: Regex::new(config::BASH_PERM_ERROR).expect("regex: perm_error"),
    missing_tool: Regex::new(config::BASH_MISSING_TOOL).expect("regex: missing_tool"),
    missing_tool_name: Regex::new(config::BASH_MISSING_TOOL_NAME).expect("regex: missing_tool_name"),
    knip_check: Regex::new(config::BASH_KNIP_CHECK).expect("regex: knip_check"),
    knip_count: Regex::new(config::BASH_KNIP_COUNT).expect("regex: knip_count"),
    build_success: Regex::new(config::BASH_BUILD_SUCCESS).expect("regex: build_success"),
    test_pass: Regex::new(config::BASH_TEST_PASS).expect("regex: test_pass"),
    knip_dirty: Regex::new(config::BASH_KNIP_DIRTY).expect("regex: knip_dirty"),
    no_circular: Regex::new(config::BASH_NO_CIRCULAR).expect("regex: no_circular"),
    git_commit: Regex::new(config::BASH_GIT_COMMIT).expect("regex: git_commit"),
    commit_msg: Regex::new(config::BASH_COMMIT_MSG).expect("regex: commit_msg"),
});

/// Process a Bash PostToolUse event: errors/milestones + state tracking.
/// Single read + single write to session-state.json.
pub fn process(cmd: &str, output: &str, exit_code: Option<i64>) {
    let re = &*RE;
    let mut state = common::read_session_state();

    // Error/milestone detection
    if let Some(code) = exit_code {
        if code != 0 {
            detect_errors(cmd, output, &mut state, re);
        } else {
            detect_milestones(cmd, output, &mut state, re);
        }
    }

    // Truncation savings: detect truncation markers in output and record savings
    // (truncate-filter is a subprocess that can't update daemon cache directly)
    detect_truncation_savings(output, &mut state);

    // Explore count
    if is_explore_command(cmd) {
        state.explore_count += 1;
    }

    // Output dedup: hash first 4KB of output, check against stored hash
    if !cmd.is_empty() {
        let normalized = normalize_cmd(cmd);
        let sample = if output.len() > 4096 {
            &output[..4096]
        } else {
            output
        };
        let hash = common::string_hash(sample);

        if let Some(prev) = state.commands.get(&normalized)
            && prev.hash == hash {
                common::additional_context(&format!(
                    "Output identical to turn {}. No new errors/changes.",
                    prev.turn
                ));
                common::log(
                    "posttool-session",
                    &format!("DEDUP cmd: {}", common::truncate(cmd, 40)),
                );
            }

        let output_tokens = (output.len() as u64) / 4; // ~1 token per 4 chars

        state.commands.insert(
            normalized,
            common::CommandEntry {
                hash,
                turn: state.turn,
                output_tokens,
            },
        );
    }

    state.enforce_bounds();
    common::write_session_state(&state);
}

fn detect_errors(
    cmd: &str,
    output: &str,
    state: &mut common::SessionState,
    re: &SessionRegexes,
) {
    state.errors_unresolved += 1;

    // TypeScript errors
    let matches: Vec<&str> = re.ts_error.find_iter(output).map(|m| m.as_str()).collect();
    if !matches.is_empty() {
        let mut unique: Vec<&str> = Vec::new();
        for m in &matches {
            if !unique.contains(m) {
                unique.push(m);
            }
        }
        let detail = format!("{} | {}", unique.join(", "), common::truncate(cmd, 60));
        common::add_session_note("ts-error", &detail);
        common::log("posttool-session", &format!("ERROR ts {}", detail));
    }

    // Lint errors
    if cmd_matches(cmd, config::LINT_CMDS)
        && let Some(cap) = re.lint_count.captures(output) {
            let n = cap.get(1).map(|m| m.as_str()).unwrap_or("?");
            let detail = format!("{} errors | {}", n, common::truncate(cmd, 60));
            common::add_session_note("lint-error", &detail);
            common::log("posttool-session", &format!("ERROR lint {}", detail));
        }

    // Dependency failures
    if re.dep_failure.is_match(output) {
        common::add_session_note("dep-error", &common::truncate(cmd, 80));
        common::log(
            "posttool-session",
            &format!("ERROR dep {}", common::truncate(cmd, 60)),
        );
    }

    // Test failures
    if cmd_matches(cmd, config::TEST_CMDS) && re.test_fail.is_match(output) {
        let n = re
            .test_count
            .captures(output)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("?");
        let detail = format!("{} failures | {}", n, common::truncate(cmd, 60));
        common::add_session_note("test-fail", &detail);
        common::log("posttool-session", &format!("ERROR test {}", detail));
    }

    // Build failures — extract file:line errors for context enrichment
    if re.build_fail.is_match(output) {
        common::add_session_note("build-fail", &common::truncate(cmd, 80));
        common::log(
            "posttool-session",
            &format!("ERROR build {}", common::truncate(cmd, 60)),
        );
        // Extract specific error locations so Claude can jump to them without
        // parsing hundreds of lines of build output
        let error_locations = extract_error_locations(output);
        if !error_locations.is_empty() {
            common::additional_context(&format!(
                "Build errors: {}",
                error_locations.join(", ")
            ));
        }
    }

    // Permission errors
    if re.perm_error.is_match(output) {
        common::add_session_note("permission", &common::truncate(cmd, 80));
        common::log(
            "posttool-session",
            &format!("ERROR permission {}", common::truncate(cmd, 60)),
        );
    }

    // Missing tool
    if re.missing_tool.is_match(output) {
        let tool_name = re
            .missing_tool_name
            .captures(output)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("unknown");
        common::add_session_note("missing-tool", tool_name);
        common::log(
            "posttool-session",
            &format!("ERROR missing-tool {}", tool_name),
        );
    }

    // Knip findings
    if cmd_matches(cmd, config::KNIP_CMDS) && re.knip_check.is_match(output) {
        let issues: Vec<&str> = re
            .knip_count
            .find_iter(output)
            .map(|m| m.as_str())
            .collect();
        let detail = if issues.is_empty() {
            "issues found".to_string()
        } else {
            issues.join(", ")
        };
        common::add_session_note("knip-finding", &detail);
        common::log("posttool-session", &format!("ERROR knip {}", detail));
    }
}

fn detect_milestones(
    cmd: &str,
    output: &str,
    state: &mut common::SessionState,
    re: &SessionRegexes,
) {
    // Build success
    if cmd_matches(cmd, config::BUILD_CMDS) && re.build_success.is_match(output) {
        let detail = format!("Build OK: {}", common::truncate(cmd, 60));
        common::add_session_note("milestone", &detail);
        common::log("posttool-session", &format!("MILESTONE {}", detail));
        state.errors_unresolved = 0;
        state.last_milestone = detail;
        state.last_build_turn = state.turn;
        state.last_build_output_tokens = (output.len() as u64) / 4;
        return;
    }

    // Tests pass
    if cmd_matches(cmd, config::TEST_CMDS) && re.test_pass.is_match(output) {
        let detail = format!("Tests OK: {}", common::truncate(cmd, 60));
        common::add_session_note("milestone", &detail);
        common::log("posttool-session", &format!("MILESTONE {}", detail));
        state.errors_unresolved = 0;
        state.last_milestone = detail;
        state.last_build_turn = state.turn;
        state.last_build_output_tokens = (output.len() as u64) / 4;
        return;
    }

    // TSC clean
    if cmd_matches(cmd, config::TSC_CMDS) && output.trim().is_empty() {
        common::add_session_note("milestone", "TypeScript clean");
        common::log("posttool-session", "MILESTONE TypeScript clean");
        state.errors_unresolved = 0;
        state.last_milestone = "TypeScript clean".to_string();
        return;
    }

    // Knip clean
    if cmd_matches(cmd, config::KNIP_CMDS) && !re.knip_dirty.is_match(output) {
        common::add_session_note("milestone", "knip clean");
        common::log("posttool-session", "MILESTONE knip clean");
        state.errors_unresolved = 0;
        state.last_milestone = "knip clean".to_string();
        return;
    }

    // Madge no circular
    if cmd_matches(cmd, config::CIRCULAR_CMDS) && re.no_circular.is_match(output) {
        common::add_session_note("milestone", "No circular deps");
        common::log("posttool-session", "MILESTONE No circular deps");
        state.errors_unresolved = 0;
        state.last_milestone = "No circular deps".to_string();
        return;
    }

    // Git commit
    if re.git_commit.is_match(cmd) {
        let msg = re
            .commit_msg
            .captures(cmd)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("?");
        let detail = format!("commit: {}", common::truncate(msg, 60));
        common::add_session_note("commit", &common::truncate(msg, 80));
        common::log(
            "posttool-session",
            &format!("MILESTONE {}", detail),
        );
        state.errors_unresolved = 0;
        state.last_milestone = detail;
        return;
    }

    // Deploy
    if cmd_matches(cmd, config::DEPLOY_CMDS) {
        let detail = format!("Deploy: {}", common::truncate(cmd, 60));
        common::add_session_note("milestone", &detail);
        common::log("posttool-session", &format!("MILESTONE {}", detail));
        state.errors_unresolved = 0;
        state.last_milestone = detail;
        return;
    }

    // Health check
    if cmd_matches(cmd, config::HEALTH_CMDS) {
        common::add_session_note("milestone", "Health check passed");
        common::log("posttool-session", "MILESTONE Health check passed");
        state.errors_unresolved = 0;
        state.last_milestone = "Health check passed".to_string();
    }
}

fn is_explore_command(cmd: &str) -> bool {
    cmd.contains("rg ")
        || cmd.contains("fd ")
        || cmd.contains("just rg")
        || cmd.contains("just fd")
        || cmd.contains("just outline")
}

fn normalize_cmd(cmd: &str) -> String {
    let mut result = String::with_capacity(cmd.len());
    for (i, word) in cmd.split_whitespace().enumerate() {
        if i > 0 { result.push(' '); }
        result.push_str(word);
    }
    result
}

fn cmd_matches(cmd: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| cmd.contains(p))
}

/// Extract file:line error locations from build output for context enrichment.
/// Returns compact strings like "src/main.rs:42: missing semicolon"
fn extract_error_locations(output: &str) -> Vec<String> {
    // Common error location formats:
    //   file.rs:42:5: error[E0308]: ...
    //   src/file.ts(42,5): error TS2304: ...
    //   file.cs(42,5): error CS1002: ...
    //   ERROR in src/file.ts:42:5
    static ERROR_LOC: LazyLock<Option<Regex>> = LazyLock::new(|| {
        Regex::new(config::BASH_ERROR_LOC).ok()
    });

    let re = match ERROR_LOC.as_ref() {
        Some(r) => r,
        None => return Vec::new(),
    };

    let mut locations: Vec<String> = Vec::new();
    let mut seen_files = std::collections::HashSet::new();

    for cap in re.captures_iter(output) {
        let file = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let line = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let msg = cap.get(3).map(|m| m.as_str()).unwrap_or("").trim();

        // Dedup by file (one entry per file is enough for navigation)
        let key = format!("{}:{}", file, line);
        if seen_files.contains(&key) {
            continue;
        }
        seen_files.insert(key);

        locations.push(format!("{}:{}: {}", file, line, common::truncate(msg, 50)));

        if locations.len() >= 5 {
            break;
        }
    }

    locations
}

/// Detect truncation markers in output and record savings in session state.
/// The truncate-filter subprocess can't update the daemon's session cache directly,
/// so we detect its markers here (posttool runs inside the daemon).
///
/// Marker formats (from truncate_filter.rs):
///   "--- TRUNCATED (500 lines, showing head+tail+3 important) ---"
///   "--- TEST OUTPUT (filtered: 200 lines → 15 kept, failures + summary) ---"
///   "--- BUILD OUTPUT (filtered: 300 lines → 20 kept, errors + warnings) ---"
///   "--- INSTALL OUTPUT (filtered: 150 lines → 8 kept) ---"
static TRUNCATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(config::BASH_TRUNCATION_MARKER).unwrap()
});

fn detect_truncation_savings(output: &str, state: &mut common::SessionState) {
    if let Some(caps) = TRUNCATION_RE.captures(output)
        && let Some(total_str) = caps.get(1)
            && let Ok(total_lines) = total_str.as_str().parse::<u64>() {
                let visible_lines = output.lines().count() as u64;
                let dropped = total_lines.saturating_sub(visible_lines);
                if dropped > 0 {
                    let saved = dropped * 10;
                    state.estimated_tokens_saved += saved;
                    state.savings_truncation += 1;
                    common::log(
                        "posttool-session",
                        &format!("TRUNCATION saved ~{} tokens ({} lines dropped)", saved, dropped),
                    );
                }
            }
}
