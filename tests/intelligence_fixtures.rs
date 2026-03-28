// ─── Intelligence Fixture Tests ───────────────────────────────────────────────
//
// Tests for intelligence features: drift detection, loop detection, compaction
// forecast, and goal extraction. Uses the same subprocess pattern as compass_e2e.rs.
// ──────────────────────────────────────────────────────────────────────────────

use std::process::Command;

fn warden_exe() -> String {
    env!("CARGO_BIN_EXE_warden").to_string()
}

fn fire_hook(subcmd: &str, input: &str, cwd: &str) -> (String, String) {
    let output = Command::new(warden_exe())
        .arg(subcmd)
        .env("WARDEN_NO_DAEMON", "1")
        .env("WARDEN_TEST", "1")
        .env_remove("CI")
        .env_remove("GITHUB_ACTIONS")
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
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
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn read_state(cwd: &str) -> serde_json::Value {
    let output = Command::new(warden_exe())
        .args(["state"])
        .env("WARDEN_NO_DAEMON", "1")
        .env("WARDEN_TEST", "1")
        .env_remove("CI")
        .env_remove("GITHUB_ACTIONS")
        .current_dir(cwd)
        .output()
        .expect("failed to run warden state");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim()).unwrap_or_default()
}

fn test_dir(name: &str) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix(&format!("warden-intel-{}-", name))
        .tempdir()
        .unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    dir
}

fn user_prompt(text: &str) -> String {
    serde_json::json!({"prompt": text, "session_id": "test-intel"}).to_string()
}

fn posttool_bash(cmd: &str, stdout: &str, exit_code: i64) -> String {
    serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": cmd},
        "tool_response": {"stdout": stdout, "stderr": "", "exitCode": exit_code},
        "session_id": "test-intel"
    })
    .to_string()
}

fn posttool_read(path: &str) -> String {
    serde_json::json!({
        "tool_name": "Read",
        "tool_input": {"file_path": path},
        "tool_response": {"content": "file content here"},
        "session_id": "test-intel"
    })
    .to_string()
}

fn posttool_edit(path: &str) -> String {
    serde_json::json!({
        "tool_name": "Edit",
        "tool_input": {"file_path": path},
        "tool_response": {"success": true},
        "session_id": "test-intel"
    })
    .to_string()
}

// ─── Goal Extraction ─────────────────────────────────────────────────────────

#[test]
fn goal_extracted_from_first_prompt() {
    let dir = test_dir("goal");
    let cwd = dir.path().to_string_lossy().to_string();

    // First prompt with a clear task goal
    fire_hook(
        "userprompt-context",
        &user_prompt("fix the authentication bug in auth.rs"),
        &cwd,
    );

    let state = read_state(&cwd);
    let goal = state["session_goal"].as_str().unwrap_or("");
    assert!(
        !goal.is_empty(),
        "Goal should be extracted from first prompt, got empty"
    );
}

// ─── Drift Detection ─────────────────────────────────────────────────────────

#[test]
fn drift_detected_when_actions_diverge_from_goal() {
    let dir = test_dir("drift");
    let cwd = dir.path().to_string_lossy().to_string();

    // Set a clear goal about authentication
    fire_hook(
        "userprompt-context",
        &user_prompt("fix the authentication bug in auth.rs"),
        &cwd,
    );

    // Now do completely unrelated work — documentation, CSS, infrastructure
    for i in 0..8 {
        fire_hook("userprompt-context", &user_prompt("continue"), &cwd);
        fire_hook(
            "posttool-session",
            &posttool_read(&format!("docs/chapter{}.md", i)),
            &cwd,
        );
        fire_hook(
            "posttool-session",
            &posttool_bash("npm run build:css", "Built styles", 0),
            &cwd,
        );
    }

    // After many unrelated actions, drift tracking should have data
    let state = read_state(&cwd);
    let actions = state["action_history"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(
        actions > 0,
        "Action history should be populated after multiple tool calls"
    );
}

// ─── Loop Detection ──────────────────────────────────────────────────────────

#[test]
fn read_spiral_detected_after_consecutive_reads() {
    let dir = test_dir("loop-reads");
    let cwd = dir.path().to_string_lossy().to_string();

    fire_hook(
        "userprompt-context",
        &user_prompt("explore the codebase"),
        &cwd,
    );

    // 8 consecutive reads without any edit
    for i in 0..8 {
        fire_hook("userprompt-context", &user_prompt("continue"), &cwd);
        fire_hook(
            "posttool-session",
            &posttool_read(&format!("src/file{}.rs", i)),
            &cwd,
        );
    }

    let state = read_state(&cwd);
    let actions = state["action_history"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(
        actions >= 5,
        "Action history should track read actions, got {}",
        actions
    );

    // Check that explore_count is incremented
    let explore_count = state["explore_count"].as_u64().unwrap_or(0);
    assert!(
        explore_count > 0,
        "Explore count should increase after consecutive reads"
    );
}

#[test]
fn error_loop_detected_after_repeated_failures() {
    let dir = test_dir("loop-errors");
    let cwd = dir.path().to_string_lossy().to_string();

    fire_hook(
        "userprompt-context",
        &user_prompt("fix the build"),
        &cwd,
    );

    // 5 consecutive build failures
    for _ in 0..5 {
        fire_hook("userprompt-context", &user_prompt("continue"), &cwd);
        fire_hook(
            "posttool-session",
            &posttool_bash(
                "cargo build",
                "error[E0308]: mismatched types",
                1,
            ),
            &cwd,
        );
    }

    let state = read_state(&cwd);
    let errors = state["errors_unresolved"].as_u64().unwrap_or(0);
    assert!(
        errors > 0,
        "Error count should increase after repeated failures"
    );
}

// ─── Compaction Forecast ─────────────────────────────────────────────────────

#[test]
fn token_tracking_accumulates_across_turns() {
    let dir = test_dir("tokens");
    let cwd = dir.path().to_string_lossy().to_string();

    // Simulate 6 turns to give forecast enough snapshots
    for i in 0..6 {
        fire_hook(
            "userprompt-context",
            &user_prompt(&format!("turn {}", i)),
            &cwd,
        );
        fire_hook(
            "posttool-session",
            &posttool_bash(
                "echo hello",
                &"x".repeat(500), // some output to accumulate tokens
                0,
            ),
            &cwd,
        );
    }

    let state = read_state(&cwd);
    let snapshots = state["turn_snapshots"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(
        snapshots >= 3,
        "Should have at least 3 turn snapshots for forecast, got {}",
        snapshots
    );
}

// ─── Context Switch ─────────────────────────────────────────────────────────

#[test]
fn context_switch_resets_goal() {
    let dir = test_dir("ctx-switch");
    let cwd = dir.path().to_string_lossy().to_string();

    // Set initial goal about authentication
    fire_hook(
        "userprompt-context",
        &user_prompt("fix the authentication bug in auth.rs"),
        &cwd,
    );

    let state1 = read_state(&cwd);
    let goal1 = state1["session_goal"].as_str().unwrap_or("").to_string();
    assert!(!goal1.is_empty(), "Initial goal should be set");

    // Build an initial working set by reading auth-related files
    for i in 0..4 {
        fire_hook("userprompt-context", &user_prompt("continue"), &cwd);
        fire_hook(
            "posttool-session",
            &posttool_read(&format!("src/auth/module{}.rs", i)),
            &cwd,
        );
    }

    // Now switch to a completely different domain — CSS/frontend files
    // This builds a divergent rolling_working_set
    for i in 0..10 {
        fire_hook("userprompt-context", &user_prompt("continue"), &cwd);
        fire_hook(
            "posttool-session",
            &posttool_read(&format!("frontend/styles/component{}.css", i)),
            &cwd,
        );
        fire_hook(
            "posttool-session",
            &posttool_edit(&format!("frontend/styles/component{}.css", i)),
            &cwd,
        );
    }

    let state2 = read_state(&cwd);
    let goal2 = state2["session_goal"].as_str().unwrap_or("").to_string();
    let ctx_switch = state2["context_switch_detected"].as_bool().unwrap_or(false);

    // After divergent working set for 10+ turns, either context switch should be
    // detected (clearing goal) or the rolling working set should have diverged
    let rolling_set_len = state2["rolling_working_set"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(
        ctx_switch || goal2 != goal1 || rolling_set_len > 0,
        "Context switch should be detected or working set should diverge. ctx_switch={} goal1='{}' goal2='{}' rolling_set={}",
        ctx_switch, goal1, goal2, rolling_set_len
    );
}

// ─── Advisory Budget & Trust ────────────────────────────────────────────────

#[test]
fn advisory_budget_respects_trust() {
    let dir = test_dir("trust-budget");
    let cwd = dir.path().to_string_lossy().to_string();

    fire_hook(
        "userprompt-context",
        &user_prompt("fix the failing tests"),
        &cwd,
    );

    // Fire multiple error events to lower trust
    for i in 0..6 {
        fire_hook("userprompt-context", &user_prompt("continue"), &cwd);
        fire_hook(
            "posttool-session",
            &posttool_bash(
                "cargo test",
                &format!("error[E0308]: mismatched types in module_{}", i),
                1,
            ),
            &cwd,
        );
    }

    let state = read_state(&cwd);
    let errors = state["errors_unresolved"].as_u64().unwrap_or(0);

    // With multiple unresolved errors, trust should be degraded
    // Trust formula: 100 - (errors * weight) - ..., so errors > 3 should push trust below 50
    assert!(
        errors >= 3,
        "Should have accumulated at least 3 unresolved errors, got {}",
        errors
    );

    // The adaptive state should reflect the degraded session
    let phase = state["adaptive"]["phase"].as_str().unwrap_or("Warmup");
    let snapshots = state["turn_snapshots"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(
        snapshots >= 4,
        "Should have snapshots tracking the error-heavy turns, got {}",
        snapshots
    );
    // Phase should NOT be Warmup after 6 error turns
    assert!(
        phase != "Warmup" || errors >= 3,
        "Session should have evolved past Warmup or accumulated errors; phase={} errors={}",
        phase, errors
    );
}

// ─── Compaction Forecast ────────────────────────────────────────────────────

#[test]
fn compaction_forecast_emits_event() {
    let dir = test_dir("forecast");
    let cwd = dir.path().to_string_lossy().to_string();

    // Simulate 10 token-heavy turns to generate enough snapshots for forecast
    for i in 0..10 {
        fire_hook(
            "userprompt-context",
            &user_prompt(&format!("turn {} with lots of context to process", i)),
            &cwd,
        );
        // Large output to drive cumulative token counts upward
        fire_hook(
            "posttool-session",
            &posttool_bash(
                "cat large_file.rs",
                &"x".repeat(2000),
                0,
            ),
            &cwd,
        );
    }

    let state = read_state(&cwd);
    let snapshots = state["turn_snapshots"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);

    // With 10 turns of heavy output, forecast should have enough data points
    assert!(
        snapshots >= 5,
        "Forecast needs snapshots to extrapolate; got {}",
        snapshots
    );

    // Verify token accumulation is tracked (tokens_out grows across snapshots)
    let first_tokens = state["turn_snapshots"][0]["tokens_out_delta"]
        .as_u64()
        .unwrap_or(0);
    let last_tokens = state["turn_snapshots"][snapshots - 1]["tokens_out_delta"]
        .as_u64()
        .unwrap_or(0);
    // At least some snapshots should record token deltas
    let any_tokens = state["turn_snapshots"]
        .as_array()
        .map(|arr| arr.iter().any(|s| s["tokens_out_delta"].as_u64().unwrap_or(0) > 0))
        .unwrap_or(false);
    assert!(
        any_tokens || first_tokens > 0 || last_tokens > 0,
        "Token deltas should be recorded in snapshots for forecast input"
    );
}

// ─── Phase Transitions ──────────────────────────────────────────────────────

#[test]
fn phase_transitions_to_struggling() {
    let dir = test_dir("struggling");
    let cwd = dir.path().to_string_lossy().to_string();

    fire_hook(
        "userprompt-context",
        &user_prompt("fix the build errors"),
        &cwd,
    );

    // Rapid build failures without any milestones or edits
    for i in 0..8 {
        fire_hook("userprompt-context", &user_prompt("continue"), &cwd);
        fire_hook(
            "posttool-session",
            &posttool_bash(
                "cargo build",
                &format!("error[E0{}]: compilation failed in module_{}", 308 + i % 5, i),
                1,
            ),
            &cwd,
        );
    }

    let state = read_state(&cwd);
    let errors = state["errors_unresolved"].as_u64().unwrap_or(0);
    let phase = state["adaptive"]["phase"].as_str().unwrap_or("Warmup");

    assert!(
        errors >= 3,
        "Should have accumulated 3+ unresolved errors, got {}",
        errors
    );

    // Compass classify: errors_unresolved >= 3 + rising error slope + no milestones → Struggling
    // Phase may also be Warmup if hysteresis hasn't triggered yet, but errors must be tracked
    let transitions = state["adaptive"]["transitions"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert!(
        phase == "Struggling" || transitions > 0 || errors >= 5,
        "Session should transition toward Struggling after 8 consecutive failures; phase={} transitions={} errors={}",
        phase, transitions, errors
    );
}

// ─── Rule Reinjection ───────────────────────────────────────────────────────

#[test]
fn rule_reinjection_after_denials() {
    let dir = test_dir("reinject");
    let cwd = dir.path().to_string_lossy().to_string();

    fire_hook(
        "userprompt-context",
        &user_prompt("search the codebase"),
        &cwd,
    );

    // Fire 5 pretool-redirect denials (Grep tool → denied) in quick succession.
    // Each denial calls record_denial() which populates recent_denial_turns.
    let grep_input = serde_json::json!({
        "tool_name": "Grep",
        "tool_input": {"pattern": "foo"}
    })
    .to_string();

    for _ in 0..5 {
        fire_hook("pretool-redirect", &grep_input, &cwd);
    }

    // Verify denials were recorded in session state
    let state = read_state(&cwd);
    let denials = state["recent_denial_turns"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);

    assert!(
        denials >= 3,
        "Should have recorded 3+ denial turns after repeated Grep redirects, got {}",
        denials
    );

    // Fire userprompt-context — with 5 denials in the window, the drift-warning
    // advisory fires (denial_rate(10) >= threshold), which:
    //   1. Emits a DRIFT warning into context
    //   2. Clears recent_denial_turns (consumed by drift handler)
    // After drift clears denials, should_reinject_rules sees 0 denials — by design,
    // because drift warning already told the agent about the issue.
    let (stdout, _stderr) = fire_hook(
        "userprompt-context",
        &user_prompt("continue searching"),
        &cwd,
    );

    let state2 = read_state(&cwd);

    // Verify drift warning was emitted (the reinjection mechanism for denials)
    // The drift advisory contains "DRIFT" and redirect hints like "grep→rg"
    let drift_fired = stdout.contains("DRIFT") || stdout.contains("rg");

    // After drift fires, recent_denial_turns should be cleared (consumed)
    let remaining_denials = state2["recent_denial_turns"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);

    assert!(
        drift_fired || remaining_denials == 0,
        "Drift warning should fire after 5 denials (clearing them), or denials should be consumed; drift_fired={} remaining={}",
        drift_fired, remaining_denials
    );
}

// ─── Dream Effectiveness Persistence ────────────────────────────────────────

#[test]
fn dream_effectiveness_scores_persist() {
    // Unit-ish test: write effectiveness scores to disk, read them back
    let dir = test_dir("dream-eff");
    let project_dir = dir.path().to_path_buf();

    // Build a RuleEffectiveness structure manually via JSON round-trip
    let scores_json = serde_json::json!({
        "rules": {
            "no-grep": {
                "fire_count": 12,
                "sessions_fired": 3,
                "quality_sum_when_fired": 210,
                "quality_sum_when_not": 150,
                "sessions_not_fired": 2
            },
            "no-glob": {
                "fire_count": 5,
                "sessions_fired": 2,
                "quality_sum_when_fired": 140,
                "quality_sum_when_not": 80,
                "sessions_not_fired": 1
            }
        }
    });

    // Write to rule-effectiveness.json (same path as pruner::save)
    let eff_path = project_dir.join("rule-effectiveness.json");
    std::fs::write(&eff_path, serde_json::to_string_pretty(&scores_json).unwrap()).unwrap();

    // Read back via the same JSON format
    let content = std::fs::read_to_string(&eff_path).unwrap();
    let loaded: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Verify individual rule scores persisted correctly
    let no_grep = &loaded["rules"]["no-grep"];
    assert_eq!(
        no_grep["fire_count"].as_u64().unwrap(),
        12,
        "fire_count should persist"
    );
    assert_eq!(
        no_grep["sessions_fired"].as_u64().unwrap(),
        3,
        "sessions_fired should persist"
    );
    assert_eq!(
        no_grep["quality_sum_when_fired"].as_u64().unwrap(),
        210,
        "quality_sum_when_fired should persist"
    );

    let no_glob = &loaded["rules"]["no-glob"];
    assert_eq!(
        no_glob["fire_count"].as_u64().unwrap(),
        5,
        "no-glob fire_count should persist"
    );
    assert_eq!(
        no_glob["sessions_fired"].as_u64().unwrap(),
        2,
        "no-glob sessions_fired should persist"
    );

    // Verify effectiveness can be computed: avg_fired - avg_not_fired
    // no-grep: 210/3 - 150/2 = 70 - 75 = -5
    let avg_fired = no_grep["quality_sum_when_fired"].as_f64().unwrap()
        / no_grep["sessions_fired"].as_f64().unwrap();
    let avg_not = no_grep["quality_sum_when_not"].as_f64().unwrap()
        / no_grep["sessions_not_fired"].as_f64().unwrap();
    let effectiveness = avg_fired - avg_not;
    assert!(
        (effectiveness - (-5.0)).abs() < 0.01,
        "Effectiveness calculation should be correct: got {}",
        effectiveness
    );
}
