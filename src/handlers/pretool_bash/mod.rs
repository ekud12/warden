// ─── pretool_bash — PreToolUse handler for Bash commands ──────────────────────
//
// The most complex hook handler. Processes every Bash tool call through a
// multi-step pipeline before execution:
//
//   0. cd+just:          TRANSFORM "cd /path && just recipe" → "just recipe"
//   1. Just-passthrough: commands starting with "just " skip to truncation only
//   2. Safety check:     DENY destructive/dangerous patterns (rm -rf, sudo, etc.)
//   2.5. Hallucination:  DENY agent-specific dangerous patterns (reverse shells, etc.)
//   2.75. Hall. advisory: ALLOW with advisory for suspicious-but-maybe-legit patterns
//   3. Destructive check: DENY ops needing explicit approval (knip --fix, sg -r)
//   4. Zero-trace:       DENY AI attribution in echo/printf/tee commands
//   5. Substitutions:    DENY banned CLIs with redirect messages (grep→rg, etc.)
//   6. Just-first:       TRANSFORM raw commands to just recipes when Justfile exists
//   6.5. Advisories:     ALLOW with systemMessage for MCP-preferred alternatives
//   7. Truncation:       WRAP verbose commands with truncate-filter pipe
//
// Uses LazyLock for one-time regex compilation. All patterns are defined
// in config.rs. Fails open (exits 0) on any error.
// ──────────────────────────────────────────────────────────────────────────────

mod build_check;
mod dedup;
mod just;
mod safety;
mod truncation;

use crate::common;
use crate::config;
use crate::rules;
use regex::Regex;
use std::sync::LazyLock;

/// Compiled regex collections — built once, reused across calls
pub(crate) struct CompiledPatterns {
    pub safety: Vec<(Regex, String)>,
    pub hallucination: Vec<(Regex, String)>,
    pub hallucination_advisory: Vec<(Regex, String)>,
    pub destructive: Vec<(Regex, String)>,
    pub substitutions: Vec<(Regex, String)>,
    pub advisories: Vec<(Regex, String)>,
    pub cd_just_re: Option<Regex>,
    pub zero_trace_cmd: Option<Regex>,
    pub zero_trace_path_exclude: Option<Regex>,
    pub zero_trace_write: Option<Regex>,
    pub verbose: Vec<Regex>,
    pub short: Vec<Regex>,
    pub just_verbose_re: Option<Regex>,
    pub just_short_re: Option<Regex>,
    pub auto_allow: Vec<Regex>,
    pub port_re: Option<Regex>,
}

pub(crate) static PATTERNS: LazyLock<CompiledPatterns> = LazyLock::new(|| {
    let r = &*rules::RULES;

    let compile_merged_pairs = |pairs: &[(String, String)]| -> Vec<(Regex, String)> {
        pairs
            .iter()
            .filter_map(|(pat, msg)| Regex::new(pat).ok().map(|re| (re, msg.clone())))
            .collect()
    };

    let compile_merged_list = |pats: &[String]| -> Vec<Regex> {
        pats.iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect()
    };

    let just_verbose_joined = r.just_verbose
        .iter()
        .map(|s| regex::escape(s))
        .collect::<Vec<_>>()
        .join("|");
    let just_short_joined = r.just_short
        .iter()
        .map(|s| regex::escape(s))
        .collect::<Vec<_>>()
        .join("|");

    // Also append legacy JSON overrides (backward compat with overrides.json)
    let overrides = &*crate::handlers::config_override::OVERRIDES;
    let mut safety = compile_merged_pairs(&r.safety_pairs);
    let mut hallucination = compile_merged_pairs(&r.hallucination_pairs);
    let mut hallucination_advisory = compile_merged_pairs(&r.hallucination_advisory_pairs);
    let destructive = compile_merged_pairs(&r.destructive_pairs);
    let mut substitutions = compile_merged_pairs(&r.substitutions_pairs);
    let mut advisories = compile_merged_pairs(&r.advisories_pairs);
    let mut auto_allow = compile_merged_list(&r.auto_allow_patterns);

    // Legacy JSON overrides (append on top of TOML-merged rules)
    for (pat, msg) in &overrides.safety {
        if let Ok(re) = Regex::new(pat) { let m: String = msg.clone(); safety.push((re, m)); }
    }
    for (pat, msg) in &overrides.hallucination {
        if let Ok(re) = Regex::new(pat) { let m: String = msg.clone(); hallucination.push((re, m)); }
    }
    for (pat, msg) in &overrides.hallucination_advisory {
        if let Ok(re) = Regex::new(pat) { let m: String = msg.clone(); hallucination_advisory.push((re, m)); }
    }
    for (pat, msg) in &overrides.substitutions {
        if let Ok(re) = Regex::new(pat) { let m: String = msg.clone(); substitutions.push((re, m)); }
    }
    for (pat, msg) in &overrides.advisories {
        if let Ok(re) = Regex::new(pat) { let m: String = msg.clone(); advisories.push((re, m)); }
    }
    for pat in &overrides.auto_allow {
        if let Ok(re) = Regex::new(pat) { auto_allow.push(re); }
    }

    CompiledPatterns {
        safety,
        hallucination,
        hallucination_advisory,
        destructive,
        substitutions,
        advisories,
        cd_just_re: Regex::new(r#"^\s*cd\s+["']?([^"'&;]+?)["']?\s*&&\s*just\s+(.+)$"#).ok(),
        zero_trace_cmd: if r.zero_trace_cmd.is_empty() { None } else { Regex::new(&r.zero_trace_cmd).ok() },
        zero_trace_path_exclude: if r.zero_trace_path_exclude.is_empty() { None } else { Regex::new(&r.zero_trace_path_exclude).ok() },
        zero_trace_write: if r.zero_trace_write.is_empty() { None } else { Regex::new(&r.zero_trace_write).ok() },
        verbose: config::VERBOSE_PATTERNS.iter().filter_map(|p| Regex::new(p).ok()).collect(),
        short: config::SHORT_COMMANDS.iter().filter_map(|p| Regex::new(p).ok()).collect(),
        just_verbose_re: Regex::new(&format!(r"(?i)^\s*just\s+({})\b", just_verbose_joined)).ok(),
        just_short_re: Regex::new(&format!(r"(?i)^\s*just\s+({})\b", just_short_joined)).ok(),
        auto_allow,
        port_re: Regex::new(r"(?:localhost|127\.0\.0\.1):(\d+)").ok(),
    }
});

/// PreToolUse handler for Bash — safety, just-first, substitutions, zero-trace, truncation
pub fn run(raw: &str) {
    let input = common::parse_input_or_return!(raw);

    let cmd = match input
        .tool_input
        .as_ref()
        .and_then(|v| v.get("command"))
        .and_then(|v| v.as_str())
    {
        Some(c) if !c.trim().is_empty() => c.trim(),
        _ => return, // Empty command — passthrough
    };

    // -1. Health gate: deny HTTP calls to unhealthy managed processes
    if let Some(port) = extract_localhost_port(cmd)
        && let Some((name, health)) = crate::handlers::proc_mgmt::get_process_on_port(port)
            && health != "healthy" {
                safety::record_deny_savings();
                common::log("pretool-bash", &format!("DENY health-gate: {} port {} is {}", name, port, health));
                common::deny(
                    "PreToolUse",
                    &format!(
                        "Service '{}' (port {}) is {}. Use: {} proc wait --name {}",
                        name, port, health, crate::constants::NAME, name
                    ),
                );
                return;
            }

    // 0. cd+just transform: "cd /path && just recipe" → "just recipe"
    //    The cd is unnecessary — just walks up to find the Justfile, and recipes
    //    have working-directory annotations for subdirectory context.
    if let Some(ref re) = PATTERNS.cd_just_re
        && let Some(caps) = re.captures(cmd) {
            let Some(recipe_match) = caps.get(2) else { return; };
            let recipe_part = recipe_match.as_str().trim();
            let new_cmd = format!("just {}", recipe_part);
            common::log("pretool-bash", &format!("TRANSFORM cd+just → {}", common::truncate(&new_cmd, 80)));
            let updated = serde_json::json!({ "command": new_cmd });
            common::allow_with_update("PreToolUse", updated);
            return;
        }

    // 1. Commands starting with "just " — skip to truncation check only
    //    (only relevant if Justfile exists; without it, just commands would fail anyway)
    if cmd.starts_with("just ") || cmd.starts_with("just\t") {
        truncation::handle_truncation(cmd);
        return;
    }

    // Cache Justfile presence for just-first transforms
    let has_justfile = just::justfile_exists();

    // 2. Safety patterns — DENY
    if safety::check_safety(cmd) { return; }

    // 2.5. Hallucination hardening — DENY
    if safety::check_hallucination(cmd) { return; }

    // 2.75. Hallucination advisories — suspicious but possibly legitimate
    if safety::check_hallucination_advisory(cmd) { return; }

    // 2.8. Control character detection — DENY commands with embedded control chars
    if let Some(desc) = common::detect_suspicious_chars(cmd) {
        safety::record_deny_savings();
        common::log_structured("pretool-bash", common::LogLevel::Deny, "control-chars", &desc);
        common::deny("PreToolUse", &format!("BLOCKED: Command contains suspicious characters ({}). Remove them and retry.", desc));
        return;
    }

    // 3. Destructive patterns — DENY
    if safety::check_destructive(cmd) { return; }

    // 4. Zero-trace patterns (AI attribution in echo/printf/tee)
    if safety::check_zero_trace(cmd) { return; }

    // 5. Substitution patterns — DENY
    if safety::check_substitutions(cmd) { return; }

    // 5.5. Pre-execution command dedup (after all safety checks)
    let (deduped, mut state) = dedup::check_dedup(cmd);
    if deduped {
        return;
    }

    // 5.75. No-op build detection (reuses state from dedup)
    if build_check::check_noop_build(cmd, &mut state) {
        return;
    }

    // 6. Just-first transform — only when Justfile exists in project
    if has_justfile
        && let Some(result) = just::try_just_transform(cmd) {
            match result {
                just::JustResult::Transform(new_cmd) => {
                    common::log("pretool-bash", &format!("TRANSFORM {} -> {}", common::truncate(cmd, 60), common::truncate(&new_cmd, 60)));
                    let updated = serde_json::json!({ "command": new_cmd });
                    common::allow_with_update("PreToolUse", updated);
                    return;
                }
                just::JustResult::Deny(msg) => {
                    safety::record_deny_savings();
                    common::log("pretool-bash", &format!("DENY just: {}", msg));
                    common::deny("PreToolUse", &msg);
                    return;
                }
                just::JustResult::Advisory(msg) => {
                    common::log("pretool-bash", &format!("ADVISORY just: {}", common::truncate(&msg, 80)));
                    common::allow_with_advisory("PreToolUse", &msg);
                    return;
                }
            }
        }

    // 6.5. Advisory patterns — ALLOW with systemMessage (non-blocking)
    if safety::check_advisories(cmd) { return; }

    // 7. Truncation check
    truncation::handle_truncation(cmd);
}

/// Extract localhost port from HTTP tool commands (xh, curl, wget targeting localhost)
fn extract_localhost_port(cmd: &str) -> Option<u16> {
    // Only check HTTP-like commands
    if !cmd.contains("xh ") && !cmd.contains("curl ") && !cmd.contains("wget ") {
        return None;
    }

    // Match localhost:PORT or 127.0.0.1:PORT
    let re = PATTERNS.port_re.as_ref()?;
    let caps = re.captures(cmd)?;
    caps.get(1)?.as_str().parse().ok()
}
