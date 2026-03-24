// ─── pretool_bash::safety — safety, destructive, zero-trace, and substitution checks ──

use super::PATTERNS;
use crate::common;
use crate::rules;

/// Check if a restriction is disabled via config
fn is_disabled(id: &str) -> bool {
    rules::RULES.disabled_restrictions.contains(id)
}

/// Record token savings from a deny/redirect intervention (~200 tokens per denial)
pub fn record_deny_savings() {
    record_deny_savings_with_rule(None);
}

/// Record deny savings and optionally track the rule ID for effectiveness analysis
pub fn record_deny_savings_with_rule(rule_id: Option<&str>) {
    let mut state = common::read_session_state();
    state.estimated_tokens_saved += 200;
    state.savings_deny += 1;
    state.record_denial();
    if let Some(id) = rule_id
        && !state.rules_fired.iter().any(|r| r == id) {
            state.rules_fired.push(id.to_string());
            if state.rules_fired.len() > 50 {
                state.rules_fired.drain(..state.rules_fired.len() - 50);
            }
        }
    common::write_session_state(&state);
}

/// Check safety patterns — returns true if command was denied
pub fn check_safety(cmd: &str) -> bool {
    for (re, msg) in &PATTERNS.safety {
        if re.is_match(cmd) && !is_safety_excluded(cmd) {
            record_deny_savings();
            common::log_structured("pretool-bash", common::LogLevel::Deny, "safety", &common::truncate(cmd, 60));
            common::deny("PreToolUse", msg);
            return true;
        }
    }
    false
}

/// Check hallucination patterns — returns true if command was denied
pub fn check_hallucination(cmd: &str) -> bool {
    for (re, msg) in &PATTERNS.hallucination {
        if re.is_match(cmd) {
            record_deny_savings();
            common::log_structured("pretool-bash", common::LogLevel::Deny, "hallucination", &common::truncate(cmd, 60));
            common::deny("PreToolUse", msg);
            return true;
        }
    }
    false
}

/// Check hallucination advisory patterns — returns true if advisory was emitted
pub fn check_hallucination_advisory(cmd: &str) -> bool {
    for (re, msg) in &PATTERNS.hallucination_advisory {
        if re.is_match(cmd) {
            common::log_structured("pretool-bash", common::LogLevel::Advisory, "hallucination", &common::truncate(cmd, 60));
            common::allow_with_advisory("PreToolUse", msg);
            return true;
        }
    }
    false
}

/// Check destructive patterns — returns true if command was denied
pub fn check_destructive(cmd: &str) -> bool {
    for (i, (re, msg)) in PATTERNS.destructive.iter().enumerate() {
        if re.is_match(cmd) && !is_destructive_excluded(cmd) {
            let restriction_id = format!("destructive.{}", i);
            if is_disabled(&restriction_id) { continue; }
            record_deny_savings_with_rule(Some(&restriction_id));
            common::log_structured("pretool-bash", common::LogLevel::Deny, "destructive", &common::truncate(cmd, 60));
            common::deny_with_id("PreToolUse", msg, &restriction_id);
            return true;
        }
    }
    false
}

/// Check zero-trace patterns in echo/printf/tee — returns true if command was denied
pub fn check_zero_trace(cmd: &str) -> bool {
    if let (Some(zt_cmd), Some(zt_write)) =
        (&PATTERNS.zero_trace_cmd, &PATTERNS.zero_trace_write)
        && zt_cmd.is_match(cmd) && zt_write.is_match(cmd) {
            let is_path_context = PATTERNS
                .zero_trace_path_exclude
                .as_ref()
                .is_some_and(|re| re.is_match(cmd));
            if !is_path_context {
                record_deny_savings();
                common::log_structured("pretool-bash", common::LogLevel::Deny, "zero-trace", cmd);
                common::deny("PreToolUse", "BLOCKED: Do not include AI/Claude/Copilot/LLM attribution in echo/printf/tee commands. Remove the attribution text and retry.");
                return true;
            }
        }
    false
}

/// Check substitution patterns — returns true if command was denied
pub fn check_substitutions(cmd: &str) -> bool {
    let base_cmd = cmd.split_whitespace().next().unwrap_or("");

    for (re, msg) in &PATTERNS.substitutions {
        if re.is_match(cmd) {
            let restriction_id = format!("substitution.{}", base_cmd);
            if is_disabled(&restriction_id) {
                continue; // User disabled this restriction
            }
            if !crate::install::detect::substitution_target_available(base_cmd) {
                continue;
            }
            record_deny_savings_with_rule(Some(&restriction_id));
            common::log_structured("pretool-bash", common::LogLevel::Deny, "substitution", &common::truncate(cmd, 40));
            common::deny_with_id("PreToolUse", msg, &restriction_id);
            return true;
        }
    }
    false
}

/// Check advisory patterns — returns true if advisory was emitted
pub fn check_advisories(cmd: &str) -> bool {
    for (re, msg) in &PATTERNS.advisories {
        if re.is_match(cmd) {
            common::log_structured("pretool-bash", common::LogLevel::Advisory, "advisory", &common::truncate(cmd, 60));
            common::allow_with_advisory("PreToolUse", msg);
            return true;
        }
    }
    false
}

/// Safety pattern exclusions (replaces regex lookaheads which the regex crate doesn't support).
/// Returns true if the command should NOT be denied despite matching a safety pattern.
fn is_safety_excluded(cmd: &str) -> bool {
    // git clean with dry-run is safe (read-only preview)
    if cmd.contains("clean") && (cmd.contains("--dry-run") || cmd.contains(" -n ") || cmd.ends_with(" -n")) {
        return true;
    }
    // git stash list/show are read-only
    if cmd.contains("stash") && (cmd.contains("list") || cmd.contains("show")) {
        return true;
    }
    false
}

/// Destructive pattern exclusions (replaces regex lookaheads).
fn is_destructive_excluded(cmd: &str) -> bool {
    if cmd.contains("sg") && cmd.contains("-r") && cmd.contains("--dry-run") {
        return true;
    }
    false
}
