// ─── install::detect — fast cached CLI/MCP detection ─────────────────────────
//
// Runs ONCE per process via LazyLock. Checks which CLIs and MCPs are available.
// Results cached — zero cost on subsequent queries.
// Used by substitution rules and advisories to auto-disable when target missing.
// ──────────────────────────────────────────────────────────────────────────────

use std::collections::HashSet;
use std::sync::LazyLock;

/// Cached set of available CLI tools (detected once at first access)
pub static AVAILABLE_TOOLS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut tools = HashSet::new();
    let checks = [
        "rg", "fd", "bat", "eza", "dust", "just", "jq", "xh", "ouch", "huniq", "procs", "sd",
        "tokei", "tsx",
    ];
    for tool in checks {
        if is_on_path(tool) {
            tools.insert(tool);
        }
    }
    tools
});

/// Check if a binary exists on PATH — fast, no output capture
fn is_on_path(binary: &str) -> bool {
    let cmd = if cfg!(windows) { "where" } else { "which" };
    std::process::Command::new(cmd)
        .arg(binary)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check if a specific tool is available (O(1) lookup after first call)
pub fn has_tool(name: &str) -> bool {
    AVAILABLE_TOOLS.contains(name)
}

/// Check if a substitution target is available.
/// Maps source tool → required target tool.
pub fn substitution_target_available(source_cmd: &str) -> bool {
    match source_cmd {
        "grep" => has_tool("rg"),
        "find" => has_tool("fd"),
        "curl" => has_tool("xh"),
        "du" => has_tool("dust"),
        "ts-node" => has_tool("tsx"),
        "sd" => false, // sd is always blocked on Windows, no substitute needed
        _ => {
            // sort|uniq → huniq
            if source_cmd.contains("sort") || source_cmd.contains("uniq") {
                return has_tool("huniq");
            }
            true // unknown — allow the substitution
        }
    }
}
