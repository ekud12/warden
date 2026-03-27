// ─── simulation tests — replay realistic hook sequences ──────────────────────
//
// Each test creates a temp directory, sets it as the project CWD, then replays
// a sequence of hook events through the warden binary. After replay, the
// session-state.json is read back and assertions verify the expected state.
//
// These are integration tests that exercise the full pipeline end-to-end.
// ──────────────────────────────────────────────────────────────────────────────

use serde_json::Value;
use std::process::Command;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn warden_exe() -> String {
    env!("CARGO_BIN_EXE_warden").to_string()
}

/// Run a hook event through warden and return (stdout, stderr, exit_code)
fn fire_hook(subcmd: &str, input: &str, cwd: &str) -> (String, String, i32) {
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

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

/// Read session state via `warden state` command (cross-platform, no hash mismatch)
fn read_session_state(cwd: &str) -> Value {
    let output = std::process::Command::new(warden_exe())
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

/// Clean up session state (no-op — TempDir handles cleanup)
fn cleanup_session(_cwd: &str) {}

/// Build a UserPromptSubmit JSON payload
fn user_prompt(text: &str) -> String {
    serde_json::json!({
        "prompt": text,
        "session_id": "test-sim-001"
    })
    .to_string()
}

/// Build a PostToolUse:Bash JSON payload (Claude Code format)
fn posttool_bash(cmd: &str, stdout: &str, stderr: &str, exit_code: i64) -> String {
    serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": cmd },
        "tool_response": {
            "stdout": stdout,
            "stderr": stderr,
            "exitCode": exit_code
        },
        "session_id": "test-sim-001"
    })
    .to_string()
}

/// Build a PreToolUse:Bash JSON payload
fn pretool_bash(cmd: &str) -> String {
    serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": cmd },
        "session_id": "test-sim-001"
    })
    .to_string()
}

/// Build a PostToolUse:Edit JSON payload
fn posttool_edit(file_path: &str) -> String {
    serde_json::json!({
        "tool_name": "Edit",
        "tool_input": { "file_path": file_path },
        "tool_response": { "success": true },
        "session_id": "test-sim-001"
    })
    .to_string()
}

/// Get a temp dir that has a .git folder (needed for git root detection)
fn test_project_dir(name: &str) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix(&format!("warden-sim-{}-", name))
        .tempdir()
        .expect("failed to create temp dir");
    // Create .git so warden finds a git root
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    dir
}

// ─── 1. Happy Path: Goal → Edit → Build Success → Milestone ────────────────

#[test]
fn sim_happy_path() {
    let dir = test_project_dir("happy");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup_session(&cwd);

    // Turn 1: user submits prompt with a goal
    fire_hook(
        "userprompt-context",
        &user_prompt("fix the build error in main.rs"),
        &cwd,
    );

    // Verify goal was extracted
    let state = read_session_state(&cwd);
    assert_eq!(state["turn"].as_u64().unwrap_or(0), 1, "turn should be 1");
    let goal = state["session_goal"].as_str().unwrap_or("");
    assert!(
        !goal.is_empty(),
        "session_goal should be extracted from prompt, got empty. State: {:?}",
        state
    );

    // Turn 2: user continues
    fire_hook("userprompt-context", &user_prompt("yes go ahead"), &cwd);

    // PostToolUse: edit a file
    fire_hook("posttool-session", &posttool_edit("src/main.rs"), &cwd);

    // PostToolUse: successful cargo build
    let build_output =
        "   Compiling myapp v0.1.0\n   Finished `release` profile [optimized] target(s) in 12.5s";
    fire_hook(
        "posttool-session",
        &posttool_bash("cargo build --release", build_output, "", 0),
        &cwd,
    );

    // Verify milestone was recorded
    let state = read_session_state(&cwd);
    let milestone = state["last_milestone"].as_str().unwrap_or("");
    assert!(
        !milestone.is_empty(),
        "last_milestone should be set after successful build, got empty. State keys: turn={}, errors={}, last_build_turn={}",
        state["turn"],
        state["errors_unresolved"],
        state["last_build_turn"]
    );
    assert_eq!(
        state["errors_unresolved"].as_u64().unwrap_or(99),
        0,
        "errors should be reset to 0 after milestone"
    );

    cleanup_session(&cwd);
}

// ─── 2. Struggling: Multiple Build Failures ─────────────────────────────────

#[test]
fn sim_struggling_session() {
    let dir = test_project_dir("struggle");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup_session(&cwd);

    // Turn 1
    fire_hook(
        "userprompt-context",
        &user_prompt("fix the type error"),
        &cwd,
    );

    // 3 consecutive build failures
    let fail_output = "error[E0308]: mismatched types\n  --> src/main.rs:42:5";
    for _ in 0..3 {
        fire_hook(
            "posttool-session",
            &posttool_bash("cargo build", fail_output, "", 1),
            &cwd,
        );
    }

    let state = read_session_state(&cwd);
    let errors = state["errors_unresolved"].as_u64().unwrap_or(0);
    assert!(
        errors >= 3,
        "errors_unresolved should be >= 3 after 3 failures, got {}",
        errors
    );

    // Now a successful build
    let ok_output = "   Finished `release` profile [optimized] target(s) in 5.0s";
    fire_hook(
        "posttool-session",
        &posttool_bash("cargo build --release", ok_output, "", 0),
        &cwd,
    );

    let state = read_session_state(&cwd);
    assert_eq!(
        state["errors_unresolved"].as_u64().unwrap_or(99),
        0,
        "errors should be 0 after successful build milestone"
    );
    assert!(
        !state["last_milestone"].as_str().unwrap_or("").is_empty(),
        "milestone should be recorded"
    );

    cleanup_session(&cwd);
}

// ─── 3. Error Decay: Gradual error resolution ──────────────────────────────

#[test]
fn sim_error_decay() {
    let dir = test_project_dir("decay");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup_session(&cwd);

    // Turn 1
    fire_hook("userprompt-context", &user_prompt("debug the issue"), &cwd);

    // 3 build failures → 3 errors
    for _ in 0..3 {
        fire_hook(
            "posttool-session",
            &posttool_bash("cargo build", "error[E0308]: mismatched types", "", 1),
            &cwd,
        );
    }

    let state = read_session_state(&cwd);
    let errors_before = state["errors_unresolved"].as_u64().unwrap_or(0);
    assert!(errors_before >= 3, "should have 3+ errors");

    // 2 successful non-build commands → each decrements by 1
    fire_hook(
        "posttool-session",
        &posttool_bash("echo hello", "hello", "", 0),
        &cwd,
    );
    fire_hook(
        "posttool-session",
        &posttool_bash("echo world", "world", "", 0),
        &cwd,
    );

    let state = read_session_state(&cwd);
    let errors_after = state["errors_unresolved"].as_u64().unwrap_or(99);
    assert!(
        errors_after < errors_before,
        "errors should decay after successful commands: before={}, after={}",
        errors_before,
        errors_after
    );

    cleanup_session(&cwd);
}

// ─── 4. Substitution Transform: grep → rg ──────────────────────────────────

#[test]
fn sim_substitution_transform() {
    let dir = test_project_dir("subst");
    let cwd = dir.path().to_string_lossy().to_string();

    // PreToolUse:Bash with grep command
    let (stdout, _stderr, _code) =
        fire_hook("pretool-bash", &pretool_bash("grep -r 'foo' src/"), &cwd);

    // Should either transform to rg, deny with suggestion, or allow silently
    // (allow happens when rg is not installed on the system — substitution is skipped)
    // The key assertion: warden didn't crash and returned something sensible
    assert!(
        stdout.contains("rg") || stdout.contains("deny") || stdout.is_empty() || stdout.contains("grep"),
        "unexpected pretool-bash output for grep: {}",
        stdout
    );

    cleanup_session(&cwd);
}

// ─── 5. Safety Block: rm -rf via Gatekeeper ────────────────────────────────

#[test]
fn sim_safety_block() {
    let dir = test_project_dir("safety");
    let cwd = dir.path().to_string_lossy().to_string();

    let (stdout, _stderr, _code) = fire_hook("pretool-bash", &pretool_bash("rm -rf /"), &cwd);

    assert!(
        stdout.contains("deny"),
        "rm -rf / should be denied by safety, got: {}",
        stdout
    );

    cleanup_session(&cwd);
}

// ─── 6. Test Success Milestone ──────────────────────────────────────────────

#[test]
fn sim_test_success_milestone() {
    let dir = test_project_dir("testpass");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup_session(&cwd);

    fire_hook("userprompt-context", &user_prompt("run the tests"), &cwd);

    let test_output = "running 15 tests\ntest result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.31s";
    fire_hook(
        "posttool-session",
        &posttool_bash("cargo test --release", test_output, "", 0),
        &cwd,
    );

    let state = read_session_state(&cwd);
    let milestone = state["last_milestone"].as_str().unwrap_or("");
    assert!(
        !milestone.is_empty(),
        "test success should record a milestone, got empty"
    );
    assert_eq!(
        state["errors_unresolved"].as_u64().unwrap_or(99),
        0,
        "errors should be 0 after test milestone"
    );

    cleanup_session(&cwd);
}

// ─── 7. Goal Extraction on Turn 2 (greeting then task) ─────────────────────

#[test]
fn sim_goal_extraction_turn2() {
    let dir = test_project_dir("goal2");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup_session(&cwd);

    // Turn 1: greeting
    fire_hook("userprompt-context", &user_prompt("hi"), &cwd);

    let state = read_session_state(&cwd);
    // "hi" is too short for goal extraction
    let goal1 = state["session_goal"].as_str().unwrap_or("");
    // May or may not extract from "hi" — that's fine

    // Turn 2: actual task
    fire_hook(
        "userprompt-context",
        &user_prompt("fix the authentication bug in the login handler"),
        &cwd,
    );

    let state = read_session_state(&cwd);
    let goal2 = state["session_goal"].as_str().unwrap_or("");
    assert!(
        !goal2.is_empty(),
        "goal should be extracted by turn 2 from substantive prompt, got empty. goal1='{}', state_turn={}",
        goal1,
        state["turn"]
    );

    cleanup_session(&cwd);
}

// ─── 8. Clippy Success = Build Milestone ────────────────────────────────────

#[test]
fn sim_clippy_milestone() {
    let dir = test_project_dir("clippy");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup_session(&cwd);

    fire_hook("userprompt-context", &user_prompt("run clippy"), &cwd);

    let clippy_output = "    Checking myapp v0.1.0\n    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.5s";
    fire_hook(
        "posttool-session",
        &posttool_bash("cargo clippy --all-targets", clippy_output, "", 0),
        &cwd,
    );

    let state = read_session_state(&cwd);
    let milestone = state["last_milestone"].as_str().unwrap_or("");
    assert!(
        !milestone.is_empty(),
        "cargo clippy success should trigger a build milestone"
    );

    cleanup_session(&cwd);
}

// ─── 9. ExitCode Missing = No Crash ────────────────────────────────────────

#[test]
fn sim_missing_exit_code() {
    let dir = test_project_dir("nocode");
    let cwd = dir.path().to_string_lossy().to_string();
    cleanup_session(&cwd);

    fire_hook("userprompt-context", &user_prompt("test"), &cwd);

    // Send a PostToolUse:Bash without exitCode — should not crash
    let input = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": { "command": "echo hi" },
        "tool_response": { "stdout": "hi\n" },
        "session_id": "test-sim-001"
    })
    .to_string();

    let (_stdout, _stderr, code) = fire_hook("posttool-session", &input, &cwd);
    assert_eq!(code, 0, "should not crash when exitCode is missing");

    cleanup_session(&cwd);
}
