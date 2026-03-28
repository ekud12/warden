// ─── pretool_bash::safety — safety, destructive, zero-trace, and substitution checks ──
//
// Phase 1 optimization: RegexSet for single-pass boolean matching.
// Instead of iterating 149 regexes sequentially, one RegexSet.is_match() call
// tests all patterns simultaneously via a single DFA pass.

use regex::RegexSet;
use std::sync::LazyLock;

use crate::common;
use crate::engines::reflex::compiled::PATTERNS;
use crate::rules;

// ── Variable Expansion / Indirect Execution Detection ──────────────────────
// Catches bypass vectors where dangerous operations are hidden behind
// variable expansion ($VAR -rf), subshells ($(cmd)), backticks (`cmd`),
// eval, or xargs piping to dangerous commands.

static EXPANSION_RISK: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r"\$\{?\w+\}?\s+-(r|f|rf|fr)\b",             // $VAR -rf
        r"`[^`]*`\s+-(r|f|rf|fr)\b",                 // `cmd` -rf
        r"\$\([^)]*\)\s+-(r|f|rf|fr)\b",             // $(cmd) -rf
        r"\beval\s+",                                // eval anything
        r"\bsource\s+/dev/",                         // source from device
        r#"\bsh\s+-c\s+["']?\$"#,                    // sh -c "$VAR"
        r#"\bbash\s+-c\s+["']?\$"#,                  // bash -c "$VAR"
        r"\bxargs\s+.*\b(rm|chmod|chown|dd|mkfs)\b", // xargs + dangerous cmd
    ])
    .unwrap()
});

/// Check if command uses variable expansion or indirect execution in dangerous positions
pub fn check_expansion_risk(cmd: &str) -> bool {
    if EXPANSION_RISK.is_match(cmd) {
        record_deny_savings();
        common::log_structured(
            "pretool-bash",
            common::LogLevel::Deny,
            "expansion-risk",
            &common::truncate(cmd, 60),
        );
        common::add_session_note(
            "deny",
            &format!("[expansion-risk] {}", common::truncate(cmd, 60)),
        );
        common::deny(
            "PreToolUse",
            "BLOCKED: Command uses variable expansion or indirect execution in a potentially dangerous context. \
             Use literal commands instead of variables/subshells for destructive operations.",
        );
        return true;
    }
    false
}

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
        && !state.rules_fired.iter().any(|r| r == id)
    {
        state.rules_fired.push(id.to_string());
        if state.rules_fired.len() > 50 {
            state.rules_fired.drain(..state.rules_fired.len() - 50);
        }
    }
    common::write_session_state(&state);
}

/// Check safety patterns — single RegexSet pass, then exclusion check on matches
pub fn check_safety(cmd: &str) -> bool {
    let matches: Vec<usize> = PATTERNS.safety_set.matches(cmd).into_iter().collect();
    for idx in matches {
        if !is_safety_excluded(cmd) {
            // Shadow mode: log but don't block
            if PATTERNS.safety_shadow.get(idx).copied().unwrap_or(false) {
                common::log(
                    "pretool-bash",
                    &format!(
                        "SHADOW safety: would deny: {}",
                        &PATTERNS.safety_messages[idx]
                    ),
                );
                continue;
            }
            record_deny_savings();
            common::log_structured(
                "pretool-bash",
                common::LogLevel::Deny,
                "safety",
                &common::truncate(cmd, 60),
            );
            common::add_session_note(
                "deny",
                &format!(
                    "[{}] {}",
                    &PATTERNS.safety_ids[idx],
                    common::truncate(cmd, 60)
                ),
            );
            common::deny("PreToolUse", &PATTERNS.safety_messages[idx]);
            return true;
        }
    }
    false
}

/// Check hallucination patterns — single RegexSet pass
pub fn check_hallucination(cmd: &str) -> bool {
    if let Some(idx) = PATTERNS.hallucination_set.matches(cmd).into_iter().next() {
        if PATTERNS
            .hallucination_shadow
            .get(idx)
            .copied()
            .unwrap_or(false)
        {
            common::log(
                "pretool-bash",
                &format!(
                    "SHADOW hallucination: would deny: {}",
                    &PATTERNS.hallucination_messages[idx]
                ),
            );
            return false;
        }
        record_deny_savings();
        common::log_structured(
            "pretool-bash",
            common::LogLevel::Deny,
            "hallucination",
            &common::truncate(cmd, 60),
        );
        common::add_session_note(
            "deny",
            &format!("[hallucination] {}", common::truncate(cmd, 60)),
        );
        common::deny("PreToolUse", &PATTERNS.hallucination_messages[idx]);
        return true;
    }
    false
}

/// Check hallucination advisory patterns — single RegexSet pass
pub fn check_hallucination_advisory(cmd: &str) -> bool {
    if let Some(idx) = PATTERNS
        .hallucination_advisory_set
        .matches(cmd)
        .into_iter()
        .next()
    {
        common::log_structured(
            "pretool-bash",
            common::LogLevel::Advisory,
            "hallucination",
            &common::truncate(cmd, 60),
        );
        common::add_session_note(
            "advisory",
            &format!("[hallucination-advisory] {}", common::truncate(cmd, 60)),
        );
        common::allow_with_advisory("PreToolUse", &PATTERNS.hallucination_advisory_messages[idx]);
        return true;
    }
    false
}

/// Check destructive patterns — RegexSet pass, then exclusion + restriction check
pub fn check_destructive(cmd: &str) -> bool {
    let matches: Vec<usize> = PATTERNS.destructive_set.matches(cmd).into_iter().collect();
    for idx in matches {
        if !is_destructive_excluded(cmd) {
            if PATTERNS
                .destructive_shadow
                .get(idx)
                .copied()
                .unwrap_or(false)
            {
                common::log(
                    "pretool-bash",
                    &format!(
                        "SHADOW destructive: would deny: {}",
                        &PATTERNS.destructive_messages[idx]
                    ),
                );
                continue;
            }
            let restriction_id = format!("destructive.{}", idx);
            if is_disabled(&restriction_id) {
                continue;
            }
            record_deny_savings_with_rule(Some(&restriction_id));
            common::log_structured(
                "pretool-bash",
                common::LogLevel::Deny,
                "destructive",
                &common::truncate(cmd, 60),
            );
            common::add_session_note(
                "deny",
                &format!("[{}] {}", restriction_id, common::truncate(cmd, 60)),
            );
            common::deny_with_id(
                "PreToolUse",
                &PATTERNS.destructive_messages[idx],
                &restriction_id,
            );
            return true;
        }
    }
    false
}

/// Check zero-trace patterns in echo/printf/tee — returns true if command was denied
pub fn check_zero_trace(cmd: &str) -> bool {
    if let (Some(zt_cmd), Some(zt_write)) = (&PATTERNS.zero_trace_cmd, &PATTERNS.zero_trace_write)
        && zt_cmd.is_match(cmd)
        && zt_write.is_match(cmd)
    {
        let is_path_context = PATTERNS
            .zero_trace_path_exclude
            .as_ref()
            .is_some_and(|re| re.is_match(cmd));
        if !is_path_context {
            record_deny_savings();
            common::log_structured("pretool-bash", common::LogLevel::Deny, "zero-trace", cmd);
            common::add_session_note(
                "deny",
                &format!("[zero-trace] {}", common::truncate(cmd, 60)),
            );
            common::deny(
                "PreToolUse",
                "BLOCKED: Do not include AI/Claude/Copilot/LLM attribution in echo/printf/tee commands. Remove the attribution text and retry.",
            );
            return true;
        }
    }
    false
}

/// Result of substitution check
pub enum SubstitutionResult {
    /// No substitution matched
    Pass,
    /// Rewrite command + teach the agent what happened
    Transform {
        new_cmd: String,
        source: String,
        target: String,
    },
    /// Block with message (incompatible output or dangerous)
    Deny,
}

/// Check substitution patterns — transforms first, then denials.
pub fn check_substitutions(cmd: &str) -> SubstitutionResult {
    let base_cmd = cmd.split_whitespace().next().unwrap_or("");

    // Transform-eligible: silently rewrite command
    for (re, source, target) in &PATTERNS.transforms {
        if re.is_match(cmd) {
            let restriction_id = format!("substitution.{}", source);
            if is_disabled(&restriction_id) {
                continue;
            }
            if !crate::install::detect::substitution_target_available(source) {
                continue;
            }
            let new_cmd = cmd.replacen(source, target, 1);
            common::log(
                "pretool-bash",
                &format!(
                    "TRANSFORM {} -> {}",
                    common::truncate(cmd, 40),
                    common::truncate(&new_cmd, 40)
                ),
            );
            return SubstitutionResult::Transform {
                new_cmd,
                source: source.to_string(),
                target: target.to_string(),
            };
        }
    }

    // Denial substitutions: block + suggest
    for (re, msg) in &PATTERNS.substitutions {
        if re.is_match(cmd) {
            let restriction_id = format!("substitution.{}", base_cmd);
            if is_disabled(&restriction_id) {
                continue;
            }
            if !crate::install::detect::substitution_target_available(base_cmd) {
                continue;
            }
            record_deny_savings_with_rule(Some(&restriction_id));
            common::log_structured(
                "pretool-bash",
                common::LogLevel::Deny,
                "substitution",
                &common::truncate(cmd, 40),
            );
            common::add_session_note(
                "deny",
                &format!("[{}] {}", restriction_id, common::truncate(cmd, 60)),
            );
            common::deny_with_id("PreToolUse", msg, &restriction_id);
            return SubstitutionResult::Deny;
        }
    }
    SubstitutionResult::Pass
}

/// Check advisory patterns — single RegexSet pass
pub fn check_advisories(cmd: &str) -> bool {
    if let Some(idx) = PATTERNS.advisories_set.matches(cmd).into_iter().next() {
        common::log_structured(
            "pretool-bash",
            common::LogLevel::Advisory,
            "advisory",
            &common::truncate(cmd, 60),
        );
        common::add_session_note(
            "advisory",
            &format!("[advisory] {}", common::truncate(cmd, 60)),
        );
        common::allow_with_advisory("PreToolUse", &PATTERNS.advisories_messages[idx]);
        return true;
    }
    false
}

/// Safety pattern exclusions (replaces regex lookaheads which the regex crate doesn't support).
/// Returns true if the command should NOT be denied despite matching a safety pattern.
fn is_safety_excluded(cmd: &str) -> bool {
    if cmd.contains("clean")
        && (cmd.contains("--dry-run") || cmd.contains(" -n ") || cmd.ends_with(" -n"))
    {
        return true;
    }
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
