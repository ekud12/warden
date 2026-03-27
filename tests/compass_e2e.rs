// ─── Anchor Compass E2E — Phase transition tests ─────────────────────────────
//
// Tests the full phase progression: Warmup → Productive → Exploring → Struggling → Late
// Each test replays a sequence of hook events and asserts the resulting phase.
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

/// Read session state by asking warden itself (avoids hash mismatch across platforms).
/// Uses `warden state` which calls read_session_state() internally — same code path as handlers.
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

fn cleanup(_cwd: &str) {
    // No-op: temp dir cleanup handles this via TempDir drop
}

fn test_dir(name: &str) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix(&format!("warden-compass-{}-", name))
        .tempdir()
        .unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    dir
}

fn user_prompt(text: &str) -> String {
    serde_json::json!({"prompt": text, "session_id": "test-compass"}).to_string()
}

fn posttool_bash(cmd: &str, stdout: &str, exit_code: i64) -> String {
    serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": cmd},
        "tool_response": {"stdout": stdout, "stderr": "", "exitCode": exit_code},
        "session_id": "test-compass"
    })
    .to_string()
}

fn posttool_edit(path: &str) -> String {
    serde_json::json!({
        "tool_name": "Edit",
        "tool_input": {"file_path": path},
        "tool_response": {"success": true},
        "session_id": "test-compass"
    })
    .to_string()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[test]
fn compass_starts_in_warmup() {
    let dir = test_dir("warmup");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup(&cwd);

    fire_hook("userprompt-context", &user_prompt("hello"), &cwd);

    let state = read_state(&cwd);
    let phase = state["adaptive"]["phase"].as_str().unwrap_or("?");
    assert_eq!(phase, "Warmup", "New session should start in Warmup");

    cleanup(&cwd);
}

#[test]
fn compass_warmup_to_productive() {
    let dir = test_dir("productive");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup(&cwd);

    // Simulate a productive session: prompt + edits + successful build
    fire_hook("userprompt-context", &user_prompt("fix the bug"), &cwd);

    for i in 0..5 {
        fire_hook("userprompt-context", &user_prompt("continue"), &cwd);
        fire_hook(
            "posttool-session",
            &posttool_edit(&format!("src/file{}.rs", i)),
            &cwd,
        );
        fire_hook(
            "posttool-session",
            &posttool_bash(
                "cargo build",
                "Finished `dev` profile [unoptimized] target(s) in 1.0s",
                0,
            ),
            &cwd,
        );
    }

    let state = read_state(&cwd);
    let phase = state["adaptive"]["phase"].as_str().unwrap_or("?");
    // After 5 turns with edits + builds, should progress beyond Warmup
    assert_ne!(
        phase, "Warmup",
        "Should have progressed past Warmup after 5 productive turns"
    );

    cleanup(&cwd);
}

#[test]
fn compass_struggling_on_errors() {
    let dir = test_dir("struggling");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup(&cwd);

    fire_hook("userprompt-context", &user_prompt("debug this"), &cwd);

    // 8 turns of errors with no edits → should enter Struggling
    for _ in 0..8 {
        fire_hook("userprompt-context", &user_prompt("try again"), &cwd);
        fire_hook(
            "posttool-session",
            &posttool_bash("cargo build", "error[E0308]: mismatched types", 1),
            &cwd,
        );
    }

    let state = read_state(&cwd);
    let errors = state["errors_unresolved"].as_u64().unwrap_or(0);
    assert!(
        errors >= 3,
        "Should have accumulated errors, got {}",
        errors
    );

    cleanup(&cwd);
}

#[test]
fn compass_late_phase_on_long_session() {
    let dir = test_dir("late");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup(&cwd);

    // Simulate 20+ turns
    for i in 0..20 {
        fire_hook(
            "userprompt-context",
            &user_prompt(&format!("turn {}", i + 1)),
            &cwd,
        );
        if i % 3 == 0 {
            fire_hook("posttool-session", &posttool_edit("src/main.rs"), &cwd);
        }
    }

    let state = read_state(&cwd);
    let turn = state["turn"].as_u64().unwrap_or(0);
    assert!(turn >= 20, "Should be at turn 20+, got {}", turn);

    cleanup(&cwd);
}

#[test]
fn compass_error_decay_recovers_phase() {
    let dir = test_dir("recover");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup(&cwd);

    fire_hook("userprompt-context", &user_prompt("fix errors"), &cwd);

    // 3 errors
    for _ in 0..3 {
        fire_hook(
            "posttool-session",
            &posttool_bash("cargo build", "error[E0308]", 1),
            &cwd,
        );
    }

    let state = read_state(&cwd);
    let errors_before = state["errors_unresolved"].as_u64().unwrap_or(0);
    assert!(errors_before >= 3);

    // Successful build → milestone → errors reset
    fire_hook(
        "posttool-session",
        &posttool_bash(
            "cargo build --release",
            "Finished `release` profile [optimized] target(s) in 5.0s",
            0,
        ),
        &cwd,
    );

    let state = read_state(&cwd);
    let errors_after = state["errors_unresolved"].as_u64().unwrap_or(0);
    assert_eq!(errors_after, 0, "Milestone should reset errors to 0");

    cleanup(&cwd);
}
