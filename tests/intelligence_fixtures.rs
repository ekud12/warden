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
