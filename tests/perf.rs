// ─── Performance benchmark tests — verify website latency claims ─────────────
//
// Each test measures actual wall-clock time for key operations.
// Results are printed for human review and asserted against thresholds.
// ──────────────────────────────────────────────────────────────────────────────

use std::process::Command;
use std::time::Instant;

fn warden_exe() -> String {
    env!("CARGO_BIN_EXE_warden").to_string()
}

fn fire_hook(subcmd: &str, input: &str, cwd: &str) -> std::time::Duration {
    let start = Instant::now();
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
    let elapsed = start.elapsed();
    assert!(output.status.success() || output.status.code() == Some(0));
    elapsed
}

fn test_dir(name: &str) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix(&format!("warden-perf-{}-", name))
        .tempdir()
        .unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    dir
}

fn bash_input(cmd: &str) -> String {
    serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": cmd},
        "session_id": "test-perf"
    })
    .to_string()
}

fn posttool_bash(cmd: &str, stdout: &str, exit_code: i64) -> String {
    serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": cmd},
        "tool_response": {"stdout": stdout, "stderr": "", "exitCode": exit_code},
        "session_id": "test-perf"
    })
    .to_string()
}

fn user_prompt(text: &str) -> String {
    serde_json::json!({"prompt": text, "session_id": "test-perf"}).to_string()
}

fn normalize_cwd(cwd: &str) -> String {
    let mut s = cwd.replace('\\', "/");
    if s.len() >= 3 && s.starts_with('/') && s.as_bytes()[2] == b'/' {
        let drive = s.as_bytes()[1].to_ascii_lowercase() as char;
        s = format!("{}:/{}", drive, &s[3..]);
    }
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        let mut bytes = s.into_bytes();
        bytes[0] = bytes[0].to_ascii_lowercase();
        s = String::from_utf8(bytes).unwrap_or_default();
    }
    s.trim_end_matches('/').to_string()
}

fn cwd_hash8(cwd: &str) -> String {
    use std::hash::{Hash, Hasher};
    let normalized = normalize_cwd(cwd);
    let mut dir = std::path::PathBuf::from(&normalized);
    let root = loop {
        if dir.join(".git").exists() {
            break normalize_cwd(&dir.to_string_lossy());
        }
        if !dir.pop() {
            break normalized.clone();
        }
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hasher);
    format!("{:016x}", hasher.finish())[..8].to_string()
}

// ─── Latency Tests ──────────────────────────────────────────────────────────

#[test]
#[ignore] // benchmark — run with: cargo test -- --ignored
fn perf_pretool_bash_safe_command() {
    let dir = test_dir("pretool");
    let cwd = dir.path().to_string_lossy().to_string();

    // Warm up (first call has startup overhead)
    fire_hook("pretool-bash", &bash_input("echo warmup"), &cwd);

    // Measure 20 safe command evaluations
    let mut total = std::time::Duration::ZERO;
    for i in 0..20 {
        let d = fire_hook(
            "pretool-bash",
            &bash_input(&format!("echo test{}", i)),
            &cwd,
        );
        total += d;
    }
    let avg_ms = total.as_millis() / 20;
    eprintln!("  pretool-bash (safe cmd) avg: {}ms over 20 calls", avg_ms);

    // Website claims <50ms for Reflex. Process startup adds overhead,
    // so we allow 200ms for out-of-process (daemon would be <2ms).
    assert!(
        avg_ms < 200,
        "pretool-bash should average <200ms (process mode), got {}ms",
        avg_ms
    );
}

#[test]
#[ignore] // benchmark — run with: cargo test -- --ignored
fn perf_pretool_bash_denied_command() {
    let dir = test_dir("deny");
    let cwd = dir.path().to_string_lossy().to_string();

    fire_hook("pretool-bash", &bash_input("echo warmup"), &cwd);

    let mut total = std::time::Duration::ZERO;
    for _ in 0..10 {
        let d = fire_hook("pretool-bash", &bash_input("rm -rf /tmp/important"), &cwd);
        total += d;
    }
    let avg_ms = total.as_millis() / 10;
    eprintln!(
        "  pretool-bash (denied cmd) avg: {}ms over 10 calls",
        avg_ms
    );

    assert!(
        avg_ms < 200,
        "denied command should average <200ms, got {}ms",
        avg_ms
    );
}

#[test]
#[ignore] // benchmark — run with: cargo test -- --ignored
fn perf_posttool_session_bash() {
    let dir = test_dir("posttool");
    let cwd = dir.path().to_string_lossy().to_string();

    // Need at least 1 turn
    fire_hook("userprompt-context", &user_prompt("test"), &cwd);

    let mut total = std::time::Duration::ZERO;
    for i in 0..20 {
        let d = fire_hook(
            "posttool-session",
            &posttool_bash(&format!("echo test{}", i), "output", 0),
            &cwd,
        );
        total += d;
    }
    let avg_ms = total.as_millis() / 20;
    eprintln!("  posttool-session (bash) avg: {}ms over 20 calls", avg_ms);

    assert!(
        avg_ms < 200,
        "posttool-session should average <200ms, got {}ms",
        avg_ms
    );
}

#[test]
#[ignore] // benchmark — run with: cargo test -- --ignored
fn perf_userprompt_context() {
    let dir = test_dir("userprompt");
    let cwd = dir.path().to_string_lossy().to_string();

    let mut total = std::time::Duration::ZERO;
    for i in 0..10 {
        let d = fire_hook(
            "userprompt-context",
            &user_prompt(&format!("turn {} continue working", i)),
            &cwd,
        );
        total += d;
    }
    let avg_ms = total.as_millis() / 10;
    eprintln!("  userprompt-context avg: {}ms over 10 calls", avg_ms);

    assert!(
        avg_ms < 300,
        "userprompt-context should average <300ms, got {}ms",
        avg_ms
    );
}

#[test]
#[ignore] // benchmark — run with: cargo test -- --ignored
fn perf_session_state_size() {
    let dir = test_dir("statesize");
    let cwd = dir.path().to_string_lossy().to_string();

    // Build up session state over 15 turns
    for i in 0..15 {
        fire_hook(
            "userprompt-context",
            &user_prompt(&format!("turn {}", i)),
            &cwd,
        );
        fire_hook(
            "posttool-session",
            &posttool_bash("echo hello", "hello", 0),
            &cwd,
        );
    }

    // Check state file size
    let hash8 = cwd_hash8(&cwd);
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let state_path = std::path::PathBuf::from(home)
        .join(".warden")
        .join("projects")
        .join(&hash8)
        .join("session-state.json");

    if let Ok(meta) = std::fs::metadata(&state_path) {
        let size_kb = meta.len() / 1024;
        eprintln!("  session-state.json after 15 turns: {}KB", size_kb);
        assert!(
            size_kb < 50,
            "Session state should be <50KB after 15 turns, got {}KB",
            size_kb
        );
    }
}
