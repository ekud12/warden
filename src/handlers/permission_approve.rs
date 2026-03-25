// ─── permission_approve — PermissionRequest auto-approve handler ─────────────
//
// Auto-approves PermissionRequest events for known-safe tool+path combinations:
//
//   - Read: always approve (reading is safe)
//   - Write/Edit/MultiEdit: approve if path is in safe project directories
//   - NEVER approve: .env, credentials, secrets, .git/, node_modules/
//
// Falls through (no output) for anything not explicitly safe, letting the
// normal permission dialog appear.
//
// Fails open (exits 0, no output) on any error.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::rules;
use regex::Regex;
use std::sync::LazyLock;

struct CompiledRules {
    /// Compiled sensitive path deny patterns — sourced from config/core/sensitive_paths.rs
    /// via rules::RULES (single source of truth, not duplicated here)
    deny_paths: Vec<Regex>,
}

static COMPILED: LazyLock<CompiledRules> = LazyLock::new(|| CompiledRules {
    deny_paths: rules::RULES
        .sensitive_deny_pairs
        .iter()
        .filter_map(|(_id, p, _msg, _shadow)| Regex::new(p).ok())
        .collect(),
});

/// Tools that are always safe to auto-approve (read-only)
const ALWAYS_APPROVE_TOOLS: &[&str] = &["Read", "Glob", "Grep", "WebFetch", "WebSearch"];

/// Tools where we approve conditionally based on file path
const PATH_APPROVE_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit"];

pub fn run(raw: &str) {
    let input = common::parse_input_or_return!(raw);

    let tool_name = match input.tool_name.as_deref() {
        Some(name) => name,
        None => return,
    };

    // Read-only tools: always approve
    if ALWAYS_APPROVE_TOOLS.contains(&tool_name) {
        common::log("permission-approve", &format!("APPROVE read-only: {}", tool_name));
        common::permission_approve();
        return;
    }

    // Write/Edit/MultiEdit: approve if path is safe
    if PATH_APPROVE_TOOLS.contains(&tool_name) {
        let file_path = input
            .tool_input
            .as_ref()
            .and_then(|v| v.get("file_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if file_path.is_empty() {
            // No path — fall through to permission dialog
            return;
        }

        // Check deny list first
        for re in &COMPILED.deny_paths {
            if re.is_match(file_path) {
                common::log(
                    "permission-approve",
                    &format!("DENY sensitive path: {} ({})", tool_name, common::truncate(file_path, 60)),
                );
                // Fall through — no output means permission dialog shows
                return;
            }
        }

        // Path is not in deny list — approve
        common::log(
            "permission-approve",
            &format!("APPROVE: {} {}", tool_name, common::truncate(file_path, 60)),
        );
        common::permission_approve();
        return;
    }

    // Bash: auto-approve known read-only commands
    if tool_name == "Bash" {
        let cmd = input
            .tool_input
            .as_ref()
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if is_readonly_bash(cmd) {
            common::log("permission-approve", &format!("APPROVE read-only bash: {}", common::truncate(cmd, 60)));
            common::permission_approve();
            return;
        }
        // Not recognized as read-only — fall through to permission dialog
        common::log("permission-approve", &format!("PASS bash: {}", common::truncate(cmd, 40)));
        return;
    }

    // MCP tools: approve if tool name starts with mcp__
    if tool_name.starts_with("mcp__") {
        common::log("permission-approve", &format!("APPROVE mcp: {}", tool_name));
        common::permission_approve();
        return;
    }

    // Everything else: fall through (no output → normal permission dialog)
    common::log("permission-approve", &format!("PASS: {}", tool_name));
}

/// Read-only command prefixes that are always safe to auto-approve.
const READONLY_PREFIXES: &[&str] = &[
    "ls", "eza", "exa", "dir",
    "head ", "tail ", "wc ", "stat ", "file ",
    "which ", "type ", "command -v",
    "readlink ", "basename ", "dirname ", "realpath ",
    "diff ", "cmp ", "comm ",
    "env", "printenv",
    "whoami", "id ", "hostname", "uname",
    "date", "cal",
    "rg ", "fd ", "bat ", "fzf ",
    "dust ", "procs ", "tokei", "tldr ",
    "jq ", "yq ", "mdq ", "glow ",
    "git status", "git log", "git diff", "git show", "git branch",
    "git remote", "git blame", "git shortlog", "git stash list",
    "cargo --version", "rustc --version", "node --version",
    "python --version", "go version", "dotnet --version",
];

/// Check if a bash command is read-only and safe to auto-approve.
/// Conservative: only matches commands that cannot modify state.
fn is_readonly_bash(cmd: &str) -> bool {
    let trimmed = cmd.trim();

    // Quick check: if command contains output redirection, never auto-approve
    if trimmed.contains('>') {
        return false;
    }

    // Check against known read-only prefixes
    for prefix in READONLY_PREFIXES {
        if trimmed.starts_with(prefix) {
            return true;
        }
    }

    // sed -n (print-only mode, no -i flag)
    if trimmed.starts_with("sed -n") {
        return !trimmed.contains("-i");
    }

    // awk (read-only by default)
    if trimmed.starts_with("awk ") || trimmed.starts_with("gawk ") {
        return true;
    }

    // cat (read-only — redirection already checked above)
    if trimmed.starts_with("cat ") {
        return true;
    }

    false
}
