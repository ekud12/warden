// ─── Engine: Anchor — Git Summary ────────────────────────────────────────────
//
// Git status parsing and caching.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use std::process::Command;

/// Get or refresh git summary. Returns cached if fresh, runs git if stale.
pub fn get_or_refresh(state: &mut common::SessionState) -> Option<String> {
    // Cache hit: no edits since last refresh
    if !state.git_summary.is_empty() && state.last_edit_turn <= state.git_summary_turn {
        return Some(state.git_summary.clone());
    }

    // Cache miss: run git status
    let raw = run_git_status()?;
    let summary = parse_porcelain(&raw)?;

    state.git_summary = summary.clone();
    state.git_summary_turn = state.turn;

    Some(summary)
}

/// Run `git status --porcelain=v1 -b` and return raw stdout (3s timeout)
fn run_git_status() -> Option<String> {
    use std::time::Duration;

    let child = Command::new("git")
        .args(["status", "--porcelain=v1", "-b"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let output = wait_with_timeout(child, Duration::from_secs(3))?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

/// Wait for a child process with timeout. Kills and returns None on timeout.
fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: std::time::Duration,
) -> Option<std::process::Output> {
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut b = Vec::new();
                        let _ = std::io::Read::read_to_end(&mut s, &mut b);
                        b
                    })
                    .unwrap_or_default();
                return Some(std::process::Output {
                    status,
                    stdout,
                    stderr: Vec::new(),
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    common::log("git-summary", "git status timed out");
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(_) => return None,
        }
    }
}

/// Parse porcelain v1 output into "Git: branch | NM NU NS" format
fn parse_porcelain(output: &str) -> Option<String> {
    let mut lines = output.lines();

    // First line: ## branch...tracking
    let branch_line = lines.next()?;
    let branch = if let Some(rest) = branch_line.strip_prefix("## ") {
        // "main...origin/main" -> "main"
        rest.split("...").next().unwrap_or(rest)
    } else {
        "?"
    };

    let mut modified = 0u32;
    let mut untracked = 0u32;
    let mut staged = 0u32;

    for line in lines {
        if line.len() < 2 {
            continue;
        }
        let x = line.as_bytes()[0];
        let y = line.as_bytes()[1];

        if x == b'?' {
            untracked += 1;
        } else {
            if x != b' ' && x != b'?' {
                staged += 1;
            }
            if y != b' ' && y != b'?' {
                modified += 1;
            }
        }
    }

    Some(format!(
        "Git: {} | {}M {}U {}S",
        branch, modified, untracked, staged
    ))
}
