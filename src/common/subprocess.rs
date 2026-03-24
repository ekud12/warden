// ─── common::subprocess — shared subprocess runner with timeout ─────────────

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Default timeout for subprocess execution (5 seconds)
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct SubprocessResult {
    pub stdout: String,
    pub exit_code: i32,
}

/// Run a command with timeout. Returns None if spawn fails or timeout exceeded.
pub fn run_with_timeout(cmd: &str, args: &[&str], timeout: Duration) -> Option<SubprocessResult> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = std::io::Read::read_to_end(&mut out, &mut stdout);
                }
                return Some(SubprocessResult {
                    stdout: String::from_utf8_lossy(&stdout).into_owned(),
                    exit_code: status.code().unwrap_or(-1),
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait(); // reap the process
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return None,
        }
    }
}

/// Run a command with default 5s timeout. Returns None if spawn fails or timeout.
pub fn run(cmd: &str, args: &[&str]) -> Option<SubprocessResult> {
    run_with_timeout(cmd, args, DEFAULT_TIMEOUT)
}

/// Run a command with stdin data and default timeout.
pub fn run_with_stdin(cmd: &str, args: &[&str], stdin_data: Option<&[u8]>) -> Option<SubprocessResult> {
    if stdin_data.is_none() {
        return run(cmd, args);
    }

    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    if let Some(data) = stdin_data {
        child.stdin.take()?.write_all(data).ok()?;
    }

    // Timeout for stdin variant too
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = std::io::Read::read_to_end(&mut out, &mut stdout);
                }
                return Some(SubprocessResult {
                    stdout: String::from_utf8_lossy(&stdout).into_owned(),
                    exit_code: status.code().unwrap_or(-1),
                });
            }
            Ok(None) => {
                if start.elapsed() >= DEFAULT_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return None,
        }
    }
}

/// Run and return first N lines of stdout, with truncation note.
#[allow(dead_code)]
pub fn run_capped(cmd: &str, args: &[&str], stdin_data: Option<&[u8]>, max_lines: usize) -> Option<String> {
    let result = run_with_stdin(cmd, args, stdin_data)?;
    if result.stdout.trim().is_empty() {
        return None;
    }
    let lines: Vec<&str> = result.stdout.lines().take(max_lines).collect();
    let total = result.stdout.lines().count();
    let suffix = if total > max_lines {
        format!(" (showing first {})", max_lines)
    } else {
        String::new()
    };
    Some(format!("{}{}", lines.join("\n"), suffix))
}
