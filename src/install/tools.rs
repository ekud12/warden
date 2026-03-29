// ─── install::tools — CLI tool detection and installation ────────────────────
//
// Detects which CLI tools are available, offers to install missing ones.
// Each tool has a description explaining WHY it matters for AI coding.
// ──────────────────────────────────────────────────────────────────────────────

use std::process::Command;

/// A CLI tool that Warden can leverage
pub struct ToolInfo {
    pub name: &'static str,
    pub binary: &'static str,
    pub description: &'static str,
    /// Explains why this tool matters for AI-assisted coding
    pub why: &'static str,
    pub install_cargo: Option<&'static str>,
    pub install_brew: Option<&'static str>,
    pub install_scoop: Option<&'static str>,
    pub install_npm: Option<&'static str>,
    /// If true, Warden strongly recommends this tool
    pub recommended: bool,
}

/// All tools Warden can integrate with
pub static TOOLS: &[ToolInfo] = &[
    ToolInfo {
        name: "ripgrep",
        binary: "rg",
        description: "Fast regex search",
        why: "10-50x faster than grep. Warden auto-redirects grep calls to rg, saving tokens on large codebases.",
        install_cargo: Some("cargo install ripgrep"),
        install_brew: Some("brew install ripgrep"),
        install_scoop: Some("scoop install ripgrep"),
        install_npm: None,
        recommended: true,
    },
    ToolInfo {
        name: "fd",
        binary: "fd",
        description: "Fast file finder",
        why: "Replaces find with sane defaults. Warden redirects file lookups to fd, cutting search time dramatically.",
        install_cargo: Some("cargo install fd-find"),
        install_brew: Some("brew install fd"),
        install_scoop: Some("scoop install fd"),
        install_npm: None,
        recommended: true,
    },
    ToolInfo {
        name: "bat",
        binary: "bat",
        description: "Syntax-highlighted file viewer",
        why: "Replaces cat with syntax highlighting. Useful for quick file previews during code review.",
        install_cargo: Some("cargo install bat"),
        install_brew: Some("brew install bat"),
        install_scoop: Some("scoop install bat"),
        install_npm: None,
        recommended: true,
    },
    ToolInfo {
        name: "eza",
        binary: "eza",
        description: "Modern ls replacement",
        why: "Better directory listings with git status. Warden substitutes ls calls for cleaner output.",
        install_cargo: Some("cargo install eza"),
        install_brew: Some("brew install eza"),
        install_scoop: Some("scoop install eza"),
        install_npm: None,
        recommended: false,
    },
    ToolInfo {
        name: "dust",
        binary: "dust",
        description: "Disk usage visualizer",
        why: "Replaces du with a visual tree. Helps AI agents understand project size at a glance.",
        install_cargo: Some("cargo install du-dust"),
        install_brew: Some("brew install dust"),
        install_scoop: Some("scoop install dust"),
        install_npm: None,
        recommended: false,
    },
    ToolInfo {
        name: "just",
        binary: "just",
        description: "Command runner (Justfile)",
        why: "Task runner that Warden hooks into. Provides consistent recipes across projects (build, test, lint).",
        install_cargo: Some("cargo install just"),
        install_brew: Some("brew install just"),
        install_scoop: Some("scoop install just"),
        install_npm: None,
        recommended: true,
    },
    ToolInfo {
        name: "jq",
        binary: "jq",
        description: "JSON processor",
        why: "Essential for parsing API responses, config files, and structured data in automation scripts.",
        install_cargo: None,
        install_brew: Some("brew install jq"),
        install_scoop: Some("scoop install jq"),
        install_npm: None,
        recommended: false,
    },
    ToolInfo {
        name: "xh",
        binary: "xh",
        description: "HTTP client",
        why: "Replaces curl with friendlier syntax. Warden redirects curl calls for cleaner HTTP debugging.",
        install_cargo: Some("cargo install xh"),
        install_brew: Some("brew install xh"),
        install_scoop: Some("scoop install xh"),
        install_npm: None,
        recommended: false,
    },
    ToolInfo {
        name: "ouch",
        binary: "ouch",
        description: "Universal archive tool",
        why: "Replaces tar/zip/unzip with auto-detection. Warden blocks raw archive commands in favor of ouch.",
        install_cargo: Some("cargo install ouch"),
        install_brew: Some("brew install ouch"),
        install_scoop: None,
        install_npm: None,
        recommended: false,
    },
];

/// Result of checking tool availability
pub struct ToolStatus {
    pub name: &'static str,
    pub binary: &'static str,
    pub installed: bool,
}

/// Check which tools are installed
pub fn detect_tools() -> Vec<ToolStatus> {
    TOOLS
        .iter()
        .map(|tool| ToolStatus {
            name: tool.name,
            binary: tool.binary,
            installed: is_installed(tool.binary),
        })
        .collect()
}

/// Check if a binary is available on PATH
pub fn is_installed(binary: &str) -> bool {
    let cmd = if cfg!(windows) { "where" } else { "which" };
    Command::new(cmd)
        .arg(binary)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Detect available package manager
pub fn detect_package_manager() -> Option<&'static str> {
    let candidates = if cfg!(windows) {
        vec!["scoop", "winget", "cargo"]
    } else if cfg!(target_os = "macos") {
        vec!["brew", "cargo"]
    } else {
        vec!["apt", "brew", "pacman", "cargo"]
    };

    candidates
        .into_iter()
        .find(|&pm| is_installed(pm))
        .map(|v| v as _)
}

/// Get the install command for a tool using the given package manager
pub fn install_command(tool: &ToolInfo, pm: &str) -> Option<&'static str> {
    match pm {
        "cargo" => tool.install_cargo,
        "brew" => tool.install_brew,
        "scoop" => tool.install_scoop,
        "npm" | "bun" => tool.install_npm,
        _ => tool.install_cargo, // fallback to cargo
    }
}

/// Install a tool using a shell command.
/// Handles quoted arguments and paths with spaces via platform shell.
pub fn install_tool(cmd: &str) -> Result<(), String> {
    if cmd.trim().is_empty() {
        return Err("Empty install command".to_string());
    }

    // Run through platform shell to properly handle quoting and paths with spaces
    let status = if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", cmd])
            .status()
    } else {
        Command::new("sh")
            .args(["-c", cmd])
            .status()
    }
    .map_err(|e| format!("Failed to run '{}': {}", cmd, e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("'{}' exited with code {:?}", cmd, status.code()))
    }
}
