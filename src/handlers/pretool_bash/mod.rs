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
// Uses LazyLock for one-time regex compilation (via engines::reflex::compiled).
// All patterns are defined in config.rs. Fails open (exits 0) on any error.
// ──────────────────────────────────────────────────────────────────────────────

mod build_check;
mod dedup;
mod just;
mod safety;
mod truncation;

use crate::common;
use crate::engines::reflex::compiled::PATTERNS;

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
        && let Some((name, health)) = crate::engines::harbor::proc_mgmt::get_process_on_port(port)
        && health != "healthy"
    {
        safety::record_deny_savings();
        common::log(
            "pretool-bash",
            &format!("DENY health-gate: {} port {} is {}", name, port, health),
        );
        common::deny(
            "PreToolUse",
            &format!(
                "Service '{}' (port {}) is {}. Use: {} proc wait --name {}",
                name,
                port,
                health,
                crate::constants::NAME,
                name
            ),
        );
        return;
    }

    // 0. cd+just transform: "cd /path && just recipe" → "just recipe"
    //    The cd is unnecessary — just walks up to find the Justfile, and recipes
    //    have working-directory annotations for subdirectory context.
    if let Some(ref re) = PATTERNS.cd_just_re
        && let Some(caps) = re.captures(cmd)
    {
        let Some(recipe_match) = caps.get(2) else {
            return;
        };
        let recipe_part = recipe_match.as_str().trim();
        let new_cmd = format!("just {}", recipe_part);
        common::log(
            "pretool-bash",
            &format!("TRANSFORM cd+just → {}", common::truncate(&new_cmd, 80)),
        );
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

    // ─── Gatekeeper: collect all safety signals, decide once ──────────────────
    {
        use crate::engines::reflex::{gatekeeper, sentinel, tripwire};
        use crate::engines::signal::{Signal, SignalCategory, Verdict};
        use crate::engines::signal_bus::SignalBus;

        let mut bus = SignalBus::new();

        // Sentinel: safety + hallucination + destructive patterns
        for sig in sentinel::check_command(cmd) {
            bus.push(sig);
        }

        // Tripwire: expansion risks
        for sig in tripwire::check_expansion_risk(cmd) {
            bus.push(sig);
        }

        // Control character detection
        if let Some(desc) = common::detect_suspicious_chars(cmd) {
            bus.push(Signal::with_verdict(
                SignalCategory::Safety,
                1.0,
                format!("Suspicious characters: {}", desc),
                "control-chars",
                Verdict::Deny(format!(
                    "BLOCKED: Command contains suspicious characters ({}). Remove them and retry.",
                    desc
                )),
            ));
        }

        // Zero-trace check (AI attribution in commands)
        // Still uses legacy check since zero-trace patterns are in safety_pairs
        // and sentinel already picks them up

        if !bus.is_empty() {
            let verdict = gatekeeper::evaluate(bus.signals());
            match verdict {
                Verdict::Deny(msg) => {
                    safety::record_deny_savings();
                    common::log_structured(
                        "pretool-bash",
                        common::LogLevel::Deny,
                        "gatekeeper",
                        &common::truncate(cmd, 60),
                    );
                    common::add_session_note(
                        "deny",
                        &format!("[gatekeeper] {}", common::truncate(cmd, 60)),
                    );
                    common::deny("PreToolUse", &msg);
                    return;
                }
                Verdict::Advisory(msg) => {
                    common::log(
                        "gatekeeper",
                        &format!("ADVISORY: {}", common::truncate(&msg, 80)),
                    );
                    common::allow_with_advisory("PreToolUse", &msg);
                    return;
                }
                Verdict::Transform(val) => {
                    common::allow_with_update("PreToolUse", val);
                    return;
                }
                Verdict::Allow => {} // fall through to substitutions + rest of pipeline
            }
        }
    }

    // 5. Substitution patterns — TRANSFORM+TEACH or DENY
    match safety::check_substitutions(cmd) {
        safety::SubstitutionResult::Transform {
            new_cmd,
            source,
            target,
        } => {
            let updated = serde_json::json!({ "command": new_cmd });
            let advisory = format!(
                "Warden transformed `{}` → `{}` for this call. Use `{}` directly next time.",
                source, target, target
            );
            common::allow_with_transform("PreToolUse", updated, &advisory);
            return;
        }
        safety::SubstitutionResult::Deny => return,
        safety::SubstitutionResult::Pass => {}
    }

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
    if has_justfile && let Some(result) = just::try_just_transform(cmd) {
        match result {
            just::JustResult::Transform(new_cmd) => {
                common::log(
                    "pretool-bash",
                    &format!(
                        "TRANSFORM {} -> {}",
                        common::truncate(cmd, 60),
                        common::truncate(&new_cmd, 60)
                    ),
                );
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
                common::log(
                    "pretool-bash",
                    &format!("ADVISORY just: {}", common::truncate(&msg, 80)),
                );
                common::allow_with_advisory("PreToolUse", &msg);
                return;
            }
        }
    }

    // 6.5. Advisory patterns — ALLOW with systemMessage (non-blocking)
    if safety::check_advisories(cmd) {
        return;
    }

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
