// ─── session e2e tests — end-to-end binary invocations ──────────────────────

use std::process::Command;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn run_warden(subcmd: &str, input: &str) -> std::process::Output {
    let exe = env!("CARGO_BIN_EXE_warden");
    Command::new(exe)
        .arg(subcmd)
        .env("WARDEN_NO_DAEMON", "1")
        .env("WARDEN_TEST", "1")
        .env_remove("CI")
        .env_remove("GITHUB_ACTIONS")
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
        .expect("failed to run warden")
}

fn run_warden_cmd(args: &[&str]) -> std::process::Output {
    let exe = env!("CARGO_BIN_EXE_warden");
    Command::new(exe)
        .args(args)
        .env("WARDEN_NO_DAEMON", "1")
        .env("WARDEN_TEST", "1")
        .env_remove("CI")
        .env_remove("GITHUB_ACTIONS")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("failed to run warden")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn combined(output: &std::process::Output) -> String {
    format!("{}{}", stdout(output), stderr(output))
}

fn bash_input(cmd: &str) -> String {
    format!(
        r#"{{"tool_name":"Bash","tool_input":{{"command":"{}"}}}}"#,
        cmd
    )
}

fn tool_available(name: &str) -> bool {
    Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ─── 1. pretool-bash blocks rm -rf ──────────────────────────────────────────

#[test]
fn test_pretool_bash_blocks_rm_rf() {
    let output = run_warden("pretool-bash", &bash_input("rm -rf /"));
    let out = stdout(&output);
    assert!(
        out.contains("deny"),
        "rm -rf / should be denied, got: {}",
        out
    );
}

// ─── 2. pretool-bash allows safe command ────────────────────────────────────

#[test]
fn test_pretool_bash_allows_safe_command() {
    let output = run_warden("pretool-bash", &bash_input("echo hello"));
    let out = stdout(&output);
    assert!(
        !out.contains("deny"),
        "echo hello should not be denied, got: {}",
        out
    );
}

// ─── 3. substitution: Grep tool -> rg ───────────────────────────────────────

#[test]
fn test_substitution_grep_to_rg() {
    if !tool_available("rg") {
        return; // skip if rg not installed
    }
    let output = run_warden(
        "pretool-redirect",
        r#"{"tool_name":"Grep","tool_input":{"pattern":"foo"}}"#,
    );
    let out = stdout(&output);
    assert!(
        out.contains("deny") || out.contains("rg"),
        "Grep tool should be redirected to rg, got: {}",
        out
    );
}

// ─── 4. substitution: Glob tool -> fd ───────────────────────────────────────

#[test]
fn test_substitution_glob_to_fd() {
    if !tool_available("fd") {
        return; // skip if fd not installed
    }
    let output = run_warden(
        "pretool-redirect",
        r#"{"tool_name":"Glob","tool_input":{"pattern":"*.rs"}}"#,
    );
    let out = stdout(&output);
    assert!(
        out.contains("deny") || out.contains("fd"),
        "Glob tool should be redirected to fd, got: {}",
        out
    );
}

// ─── 5. describe default output ─────────────────────────────────────────────

#[test]
fn test_describe_default_output() {
    let output = run_warden_cmd(&["describe"]);
    let all = combined(&output);
    // Default describe should produce reasonable output, not hundreds of lines
    let line_count = all.lines().count();
    assert!(
        line_count < 100,
        "describe default should not dump hundreds of lines, got {} lines",
        line_count
    );
    // Should contain the warden branding (printed to stderr)
    assert!(
        all.contains("W A R D E N") || all.contains("warden"),
        "describe should contain warden branding, got: {}",
        all
    );
}

// ─── 6. describe --all output ───────────────────────────────────────────────

#[test]
fn test_describe_all_output() {
    let output = run_warden_cmd(&["describe", "--all"]);
    let out = stdout(&output);
    // --all should produce JSON on stdout
    assert!(
        out.contains('{') && out.contains("version"),
        "describe --all should output JSON with version field, got: {}",
        &out[..out.len().min(200)]
    );
    // Validate it actually parses as JSON
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&out);
    assert!(
        parsed.is_ok(),
        "describe --all output should be valid JSON, parse error: {:?}",
        parsed.err()
    );
}

// ─── 7. version output ─────────────────────────────────────────────────────

#[test]
fn test_version_output() {
    let output = run_warden_cmd(&["version"]);
    let out = combined(&output);
    // Should contain "warden" and a semver-like version
    assert!(
        out.contains("warden"),
        "version output should contain 'warden', got: {}",
        out
    );
    let pkg_version = env!("CARGO_PKG_VERSION");
    assert!(
        out.contains(pkg_version),
        "version output should contain CARGO_PKG_VERSION ({}), got: {}",
        pkg_version,
        out
    );
}

// ─── 8. help output ────────────────────────────────────────────────────────

#[test]
fn test_help_output() {
    // Running with no args shows help
    let output = run_warden_cmd(&[]);
    let all = combined(&output);
    // Should contain key commands
    assert!(
        all.contains("init"),
        "help should list 'init' command, got: {}",
        all
    );
    assert!(
        all.contains("install"),
        "help should list 'install' command, got: {}",
        all
    );
    assert!(
        all.contains("describe"),
        "help should list 'describe' command, got: {}",
        all
    );
    assert!(
        all.contains("version"),
        "help should list 'version' command, got: {}",
        all
    );
    assert!(
        all.contains("COMMANDS") || all.contains("USAGE"),
        "help should contain COMMANDS or USAGE section, got: {}",
        all
    );
}
