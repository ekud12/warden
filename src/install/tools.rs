// ─── install::tools — CLI tool detection and installation ────────────────────
//
// Detects which CLI tools are available, offers to install missing ones,
// and writes the detected capabilities to config.toml.
// ──────────────────────────────────────────────────────────────────────────────

use std::process::Command;

/// A CLI tool that Warden can leverage
pub struct ToolInfo {
    pub name: &'static str,
    pub binary: &'static str,
    pub description: &'static str,
    pub install_cargo: Option<&'static str>,
    pub install_brew: Option<&'static str>,
    pub install_scoop: Option<&'static str>,
    pub install_npm: Option<&'static str>,
    /// If false, Warden works fine without it (substitution rules just won't fire)
    pub required: bool,
}

/// All tools Warden can integrate with
pub static TOOLS: &[ToolInfo] = &[
    ToolInfo {
        name: "ripgrep",
        binary: "rg",
        description: "Fast regex search (replaces grep)",
        install_cargo: Some("cargo install ripgrep"),
        install_brew: Some("brew install ripgrep"),
        install_scoop: Some("scoop install ripgrep"),
        install_npm: None,
        required: false,
    },
    ToolInfo {
        name: "fd",
        binary: "fd",
        description: "Fast file finder (replaces find)",
        install_cargo: Some("cargo install fd-find"),
        install_brew: Some("brew install fd"),
        install_scoop: Some("scoop install fd"),
        install_npm: None,
        required: false,
    },
    ToolInfo {
        name: "bat",
        binary: "bat",
        description: "Cat with syntax highlighting",
        install_cargo: Some("cargo install bat"),
        install_brew: Some("brew install bat"),
        install_scoop: Some("scoop install bat"),
        install_npm: None,
        required: false,
    },
    ToolInfo {
        name: "eza",
        binary: "eza",
        description: "Modern ls replacement",
        install_cargo: Some("cargo install eza"),
        install_brew: Some("brew install eza"),
        install_scoop: Some("scoop install eza"),
        install_npm: None,
        required: false,
    },
    ToolInfo {
        name: "dust",
        binary: "dust",
        description: "Disk usage visualization (replaces du)",
        install_cargo: Some("cargo install du-dust"),
        install_brew: Some("brew install dust"),
        install_scoop: Some("scoop install dust"),
        install_npm: None,
        required: false,
    },
    ToolInfo {
        name: "just",
        binary: "just",
        description: "Task runner (Justfile support)",
        install_cargo: Some("cargo install just"),
        install_brew: Some("brew install just"),
        install_scoop: Some("scoop install just"),
        install_npm: None,
        required: false,
    },
    ToolInfo {
        name: "jq",
        binary: "jq",
        description: "JSON processor",
        install_cargo: None,
        install_brew: Some("brew install jq"),
        install_scoop: Some("scoop install jq"),
        install_npm: None,
        required: false,
    },
    ToolInfo {
        name: "xh",
        binary: "xh",
        description: "HTTP client (replaces curl)",
        install_cargo: Some("cargo install xh"),
        install_brew: Some("brew install xh"),
        install_scoop: Some("scoop install xh"),
        install_npm: None,
        required: false,
    },
    ToolInfo {
        name: "ouch",
        binary: "ouch",
        description: "Archive tool (replaces tar/zip/unzip)",
        install_cargo: Some("cargo install ouch"),
        install_brew: Some("brew install ouch"),
        install_scoop: None,
        install_npm: None,
        required: false,
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

/// Install a tool using a shell command
pub fn install_tool(cmd: &str) -> Result<(), String> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty install command".to_string());
    }

    let status = Command::new(parts[0])
        .args(&parts[1..])
        .status()
        .map_err(|e| format!("Failed to run '{}': {}", cmd, e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("'{}' exited with code {:?}", cmd, status.code()))
    }
}
