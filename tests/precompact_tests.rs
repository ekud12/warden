// ─── precompact handler tests — verify no rule leakage into output ────────────
//
// The precompact_memory handler should emit session summaries, NOT tool-enforcement
// rules. Rules persist via Claude's rules/ directory and must not be duplicated
// into additionalContext (wastes tokens, causes drift).
// ──────────────────────────────────────────────────────────────────────────────

use std::process::Command;

fn run_warden(subcmd: &str, input: &str) -> String {
    let exe = env!("CARGO_BIN_EXE_warden");
    let output = Command::new(exe)
        .arg(subcmd)
        .env("WARDEN_NO_DAEMON", "1")
        .env("WARDEN_TEST", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
            child.wait_with_output()
        })
        .expect("failed to run warden");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn precompact_input() -> String {
    r#"{"hook":"PreCompact","tool_name":"","tool_input":{}}"#.to_string()
}

// ─── Test: precompact output must not contain tool-enforcement rules ─────────

#[test]
fn test_precompact_no_rules_in_output() {
    let out = run_warden("precompact-memory", &precompact_input());

    // Key phrases from tool-enforcement rules that must NOT appear
    let forbidden_phrases = [
        "NEVER use the Grep tool",
        "ALWAYS use aidex_query",
        "NEVER use `rg` for symbol lookups",
        "NEVER guess or hallucinate library APIs",
        "NEVER read a code file >50KB",
        "ALWAYS use context7",
        "NEVER use Read to understand file structure",
        "Tool Enforcement Rules",
        "Hook-enforced",
        "Tool choice quick reference",
        "NEVER use `tar`",
        "NEVER use `du`",
        "NEVER use `sort | uniq`",
    ];

    for phrase in &forbidden_phrases {
        assert!(
            !out.contains(phrase),
            "Precompact output must NOT contain tool-enforcement rule: '{}'.\nGot output:\n{}",
            phrase,
            &out[..out.len().min(500)]
        );
    }
}

// ─── Test: precompact output is valid or empty (fail-open) ──────────────────

#[test]
fn test_precompact_output_structure() {
    let out = run_warden("precompact-memory", &precompact_input());

    // Output should either be empty (no session data) or contain session context
    if !out.is_empty() {
        // If there is output, it should be session-related, not rules
        // The output format uses additionalContext which is JSON
        let no_rule_keywords = !out.contains("NEVER") && !out.contains("ALWAYS")
            && !out.contains("BLOCKED");
        assert!(
            no_rule_keywords,
            "Precompact output should contain session data, not enforcement rules.\nGot: {}",
            &out[..out.len().min(500)]
        );
    }
}

// ─── Test: precompact with empty input still works (fail-open) ──────────────

#[test]
fn test_precompact_empty_input_passthrough() {
    let out = run_warden("precompact-memory", "");
    // Should not crash, should not contain rules
    assert!(
        !out.contains("NEVER use the Grep tool"),
        "Empty-input precompact must not leak rules"
    );
}
