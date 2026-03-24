// ─── git_guardian — git branch awareness + safety ────────────────────────────
//
// Detects risky git states and injects warnings:
//   - Working on main/master branch
//   - Branch significantly behind remote
//   - Long-running uncommitted changes
//   - File co-change suggestions (files that usually change together)
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;

/// Check git branch state and return warnings (called from session_start + userprompt_context)
pub fn check_branch_state() -> Vec<String> {
    let mut warnings = Vec::new();

    // Get current branch name
    let branch = match common::subprocess::run("git", &["rev-parse", "--abbrev-ref", "HEAD"]) {
        Some(r) if r.exit_code == 0 => r.stdout.trim().to_string(),
        _ => return warnings,
    };

    // Warn if on a protected branch
    let protected = crate::config::PROTECTED_BRANCHES;
    if protected.iter().any(|&b| b == branch) {
        warnings.push(format!(
            "Branch warning: You are on `{}`. Consider creating a feature branch before editing.",
            branch
        ));
    }

    // Check if branch is behind remote (quick: use rev-list count)
    if let Some(behind) = commits_behind(&branch)
        && behind >= 10 {
            warnings.push(format!(
                "Branch `{}` is {} commits behind remote. Consider pulling or rebasing.",
                branch, behind
            ));
        }

    warnings
}

/// Check for long-running uncommitted changes (called from userprompt_context periodically)
pub fn check_uncommitted_duration(state: &common::SessionState) -> Option<String> {
    // Only check every 10 turns
    if state.turn < 10 || !state.turn.is_multiple_of(10) {
        return None;
    }

    // Check if there are uncommitted changes
    let status = common::subprocess::run("git", &["status", "--porcelain"])?;
    if status.exit_code != 0 || status.stdout.trim().is_empty() {
        return None;
    }

    let changed_files = status.stdout.lines().count();
    let edits = state.files_edited.len();

    if edits >= 5 && changed_files >= 3 {
        Some(format!(
            "You have {} edited files with uncommitted changes across {} modified files. Consider a checkpoint commit.",
            edits, changed_files
        ))
    } else {
        None
    }
}

/// Suggest related files that usually change together (git co-change analysis)
pub fn suggest_cochanges(edited_file: &str) -> Option<String> {
    // Get the short filename for matching
    let short = edited_file.rsplit('/').next()
        .or_else(|| edited_file.rsplit('\\').next())
        .unwrap_or(edited_file);

    // Use git log to find files that co-change with this file
    let result = common::subprocess::run("git", &[
        "log", "--pretty=format:", "--name-only", "--follow", "-10", "--", edited_file
    ])?;

    if result.exit_code != 0 || result.stdout.trim().is_empty() {
        return None;
    }

    // Count file co-occurrences in the same commits
    let result2 = common::subprocess::run("git", &[
        "log", "--pretty=format:---", "--name-only", "-10", "--", edited_file
    ])?;

    if result2.exit_code != 0 {
        return None;
    }

    let mut cochange_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut in_commit = false;

    for line in result2.stdout.lines() {
        if line.trim() == "---" {
            in_commit = true;
            continue;
        }
        if line.trim().is_empty() {
            in_commit = false;
            continue;
        }
        if in_commit && line.trim() != edited_file && !line.contains(short) {
            *cochange_counts.entry(line.trim().to_string()).or_default() += 1;
        }
    }

    // Find files that co-changed in >50% of commits (at least 3 times)
    let suggestions: Vec<&String> = cochange_counts.iter()
        .filter(|(_, count)| **count >= 3)
        .map(|(file, _)| file)
        .take(3)
        .collect();

    if suggestions.is_empty() {
        return None;
    }

    Some(format!(
        "Co-change hint: `{}` usually changes with: {}",
        short,
        suggestions.iter().map(|s| {
            s.rsplit('/').next().unwrap_or(s)
        }).collect::<Vec<_>>().join(", ")
    ))
}

/// Count how many commits the current branch is behind its upstream
fn commits_behind(branch: &str) -> Option<u32> {
    let remote_ref = format!("origin/{}", branch);
    let result = common::subprocess::run("git", &[
        "rev-list", "--count", &format!("HEAD..{}", remote_ref)
    ])?;

    if result.exit_code != 0 {
        return None;
    }

    result.stdout.trim().parse().ok()
}
