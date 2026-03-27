// ─── Engine: Anchor — Ledger ─────────────────────────────────────────────────
//
// Runs after Bash, Write, Edit, MultiEdit, and Read tool calls. Tracks session
// activity by writing structured JSONL entries to session-notes.jsonl AND
// updating session-state.json for smart features:
//
// For Bash (exit_code != 0 → errors, == 0 → milestones):
//   - TypeScript errors (TS\d{4}), lint errors, dependency failures
//   - Test failures, build failures, permission errors, missing tools
//   - Knip findings (unused/unresolved)
//   - Build/test/tsc success, commits, deploys, health checks
//   - Output dedup: hash command outputs, detect identical runs
//
// For Write/Edit/MultiEdit:
//   - Tracks edited code files (dedup by checking last 300 bytes of session file)
//   - Resets explore_count (editing = committing to approach)
//   - Tracks files_edited in session-state
//
// For Read:
//   - Records file read with content hash in session-state.files_read
//   - Increments explore_count
//
// This data feeds into session_start (context loading), precompact_memory
// (compaction summaries), pretool_read (read dedup), and userprompt_context
// (exploration budget).
// ──────────────────────────────────────────────────────────────────────────────

mod bash;
mod compress;
mod edit;
mod read;
mod syntax;

use crate::common;
use crate::config;
use crate::engines::anchor::budget as token_budget;
use crate::rules;
use regex::Regex;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

static CODE_EXTS: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(config::CODE_EXTS_REGEX).ok());

/// PostToolUse handler for Bash|Write|Edit|MultiEdit|Read
pub fn run(raw: &str) {
    let input = common::parse_input_or_return!(raw);

    let tool = match input.tool_name.as_deref() {
        Some(t) => t,
        None => return,
    };

    let ti = input.tool_input.as_ref();

    // ── Cost tracking: categorize token usage ──
    {
        let output_size = input
            .tool_output
            .as_ref()
            .map(|v| v.to_string().len() as u64 / 4) // ~1 token per 4 chars
            .unwrap_or(0);
        let input_size = ti.map(|v| v.to_string().len() as u64 / 4).unwrap_or(0);
        let _was_denied = false; // PostToolUse means it ran
        let _was_dedup = false;
        let mut cost_state = common::read_session_state();
        // Track in estimated tokens
        cost_state.estimated_tokens_in += input_size;
        cost_state.estimated_tokens_out += output_size;
        common::write_session_state(&cost_state);
    }

    // ── DOOM-LOOP DETECTION: identical tool calls ──
    check_doom_loop(tool, ti);

    // ── Action tracking for entropy + markov ──
    {
        let action_type = match tool {
            "Bash" => {
                let exit_code = input
                    .tool_output
                    .as_ref()
                    .and_then(|v| {
                        v.get("exitCode")
                            .or_else(|| v.get("exit_code"))
                            .or_else(|| v.get("returncode"))
                    })
                    .and_then(|v| v.as_i64());
                if exit_code == Some(0) {
                    "bash_ok"
                } else {
                    "bash_fail"
                }
            }
            "Read" => "read",
            "Write" | "Edit" | "MultiEdit" => "edit",
            _ => "other",
        };
        let mut state = common::read_session_state();
        // Record action
        state.action_history.push(action_type.to_string());
        if state.action_history.len() > 20 {
            state
                .action_history
                .drain(..state.action_history.len() - 20);
        }
        // Record Markov transition
        if state.action_history.len() >= 2 {
            let prev = state.action_history[state.action_history.len() - 2].clone();
            crate::analytics::markov::record_transition(
                &mut state.action_transitions,
                &prev,
                action_type,
            );
        }
        // Record working set directories
        let file_path = ti
            .and_then(|v| v.get("file_path").or_else(|| v.get("path")))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !file_path.is_empty() {
            let dir = file_path.replace('\\', "/");
            if let Some(d) = dir.rsplit('/').nth(1) {
                let d = d.to_string();
                // Initial working set (first 5 dirs, frozen)
                if state.initial_working_set.len() < 5 && !state.initial_working_set.contains(&d) {
                    state.initial_working_set.push(d.clone());
                }
                // Track touch on initial set (for context switch decay)
                if state.initial_working_set.contains(&d) {
                    state.last_initial_set_touch_turn = state.turn;
                }
                // Rolling working set (last 10 dirs, updates continuously)
                if !state.rolling_working_set.contains(&d) {
                    state.rolling_working_set.push(d);
                    if state.rolling_working_set.len() > 10 {
                        state.rolling_working_set.remove(0);
                    }
                }
            }
        }
        common::write_session_state(&state);
    }

    // ── BASH: Error/Milestone Detection + Session State ──
    if tool == "Bash" {
        let cmd = ti
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        let to = input.tool_output.as_ref();
        let stdout = to
            .and_then(|v| v.get("stdout"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let stderr = to
            .and_then(|v| v.get("stderr"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let exit_code = to
            .and_then(|v| {
                v.get("exitCode")
                    .or_else(|| v.get("exit_code"))
                    .or_else(|| v.get("returncode"))
            })
            .and_then(|v| v.as_i64());

        let output = format!("{}\n{}", stdout, stderr);

        // Single-pass: errors/milestones + state tracking (1 read + 1 write)
        bash::process(cmd, &output, exit_code);

        // Output compression (supplementary summary for long outputs)
        if let Some(summary) = compress::summarize(&output) {
            common::additional_context(&summary);
        }

        // Large output offloading to scratch file
        let offload_threshold = rules::RULES.offload_threshold;
        if offload_threshold > 0 && output.len() > offload_threshold {
            if let Some((preview, path)) = common::scratch::offload(&output, "bash") {
                common::log(
                    "offload",
                    &format!("Bash output ({} bytes) → {}", output.len(), path.display()),
                );
                common::additional_context(&preview);
            }
            // Cleanup old scratch files periodically
            common::scratch::cleanup_old();
        }

        // Response sanitization: scan output for prompt injection
        scan_for_injection_with_threshold("bash-output", &output);

        // Token budget tracking
        token_budget::track(ti, input.tool_output.as_ref());

        return;
    }

    // ── READ: File Read Tracking ──
    if tool == "Read" {
        let file_path = ti
            .and_then(|v| v.get("file_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if !file_path.is_empty() {
            read::update_read_state(file_path);
        }

        // Scan Read output for injection
        if let Some(output) = &input.tool_output
            && let Some(text) = output
                .get("content")
                .or_else(|| output.get("output"))
                .and_then(|v| v.as_str())
        {
            scan_for_injection_with_threshold("read-output", text);
        }

        return;
    }

    // ── WRITE/EDIT/MULTIEDIT: Edit Tracking ──
    if tool == "Write" || tool == "Edit" || tool == "MultiEdit" {
        let file_path = ti
            .and_then(|v| v.get("file_path").or_else(|| v.get("path")))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if !file_path.is_empty() {
            // Check if code file extension
            if CODE_EXTS.as_ref().is_some_and(|re| re.is_match(file_path)) {
                // Dedup: check last 300 bytes of session file for same path
                let session_path = common::project_dir().join("session-notes.jsonl");
                let tail = common::read_tail(&session_path, 300);
                if !tail.contains(file_path) {
                    common::add_session_note("edit", file_path);
                    common::log("posttool-session", &format!("EDIT {}", file_path));
                } else {
                    common::log(
                        "posttool-session",
                        &format!("EDIT (dedup skip) {}", file_path),
                    );
                }
            }

            // Session state: reset explore_count, track edited file
            edit::update_edit_state(file_path);

            // Co-change suggestion (rate-limited: only if advisory ready)
            {
                let mut state = common::read_session_state();
                if state.advisory_ready("cochange")
                    && let Some(hint) = crate::handlers::git_guardian::suggest_cochanges(file_path)
                {
                    common::additional_context(&hint);
                    common::log(
                        "posttool-session",
                        &format!("co-change: {}", common::truncate(&hint, 80)),
                    );
                }
                common::write_session_state(&state);
            }

            // Active typos check on edited code files (rate-limited, 5-turn cooldown)
            if CODE_EXTS.as_ref().is_some_and(|re| re.is_match(file_path)) {
                let mut state = common::read_session_state();
                if state.advisory_ready("typos")
                    && let Some(findings) = run_typos(file_path)
                {
                    common::log(
                        "posttool-session",
                        &format!("typos: {}", common::truncate(&findings, 80)),
                    );
                    common::additional_context(&findings);
                }
                common::write_session_state(&state);
            }

            // JSON syntax validation (runs for .json regardless of CODE_EXTS)
            syntax::check_syntax(file_path);
        }

        // Token budget tracking for edits
        token_budget::track(ti, input.tool_output.as_ref());
    }
}

/// Run typos on a file, return formatted findings or None.
/// Fails open if typos is not installed.
fn run_typos(file_path: &str) -> Option<String> {
    use crate::common::subprocess;

    let result = subprocess::run("typos", &["--brief", "--no-default-config", file_path])?;

    // Exit code 2 = typos found, 0 = clean, 1 = error
    if result.exit_code != 2 {
        return None;
    }

    if result.stdout.trim().is_empty() {
        return None;
    }

    let total = result.stdout.lines().count();
    let lines: Vec<&str> = result.stdout.lines().take(5).collect();
    let truncated = if total > 5 { " (showing first 5)" } else { "" };

    Some(format!(
        "Typos detected{}:\n{}",
        truncated,
        lines.join("\n")
    ))
}

/// Scan text for injection with per-category session threshold (max 5 per category)
fn scan_for_injection_with_threshold(source: &str, text: &str) {
    let matches = common::sanitize::scan_for_injection(text);
    if matches.is_empty() {
        return;
    }

    let mut state = common::read_session_state();
    let mut should_warn = false;

    for m in &matches {
        let count = state
            .injection_warn_counts
            .entry(m.category.clone())
            .or_insert(0);
        if *count < 5 {
            *count += 1;
            should_warn = true;
        }
    }

    common::write_session_state(&state);

    if should_warn {
        let warning = common::sanitize::build_warning(&matches);
        common::log("injection-detect", &format!("{}: {}", source, warning));
        common::additional_context(&warning);
    }
}

/// Doom-loop detection: track tool call fingerprints and warn on repeats.
/// Fingerprint = hash(tool_name + serialized args). Fires advisory when
/// the same exact call is made N times (configurable via rules.toml).
fn check_doom_loop(tool: &str, tool_input: Option<&serde_json::Value>) {
    let threshold = rules::RULES.doom_loop_threshold;
    if threshold == 0 {
        return; // disabled
    }

    // Build fingerprint from tool name + args
    let mut hasher = DefaultHasher::new();
    tool.hash(&mut hasher);
    if let Some(input) = tool_input {
        // Serialize args deterministically for hashing
        let args_str = input.to_string();
        args_str.hash(&mut hasher);
    }
    let fingerprint = hasher.finish();

    let mut state = common::read_session_state();
    let count = state.tool_fingerprints.entry(fingerprint).or_insert(0);
    *count = count.saturating_add(1);
    let current = *count;

    if current >= threshold {
        // Reset counter so warning doesn't fire every subsequent call
        state.tool_fingerprints.remove(&fingerprint);
        state.enforce_bounds();
        common::write_session_state(&state);

        // Build a human-readable summary of what's looping
        let args_preview = tool_input
            .map(|v| {
                let s = v.to_string();
                if s.len() > 120 {
                    format!("{}...", &s[..120])
                } else {
                    s
                }
            })
            .unwrap_or_default();

        let warning = format!(
            "Doom-loop detected: `{}` called {} times with identical arguments. \
             This suggests a repeated failing action. Try a different approach, \
             check error output carefully, or ask the user for guidance.\nArgs: {}",
            tool, current, args_preview
        );
        common::log(
            "doom-loop",
            &format!(
                "{} x{}: {}",
                tool,
                current,
                common::truncate(&args_preview, 80)
            ),
        );
        common::additional_context(&warning);
    } else {
        state.enforce_bounds();
        common::write_session_state(&state);
    }
}
