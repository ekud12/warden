// ─── install::path — cross-platform PATH registration ────────────────────────
//
// After install, ~/.warden/bin/ must be on PATH so `warden` is accessible
// from any shell: bash, zsh, cmd, PowerShell, fish.
//
// Strategy per platform:
//   Windows: Add to user PATH via registry (HKCU\Environment)
//   macOS/Linux: Append to ~/.bashrc, ~/.zshrc, ~/.profile, ~/.config/fish/config.fish
// ──────────────────────────────────────────────────────────────────────────────

use super::bin_dir;

/// Check if ~/.warden/bin is already on PATH
pub fn is_on_path() -> bool {
    let bin = bin_dir();
    let bin_str = bin.to_string_lossy();

    if let Ok(path_var) = std::env::var("PATH") {
        let separator = if cfg!(windows) { ';' } else { ':' };
        for entry in path_var.split(separator) {
            let normalized = entry.replace('\\', "/").to_lowercase();
            let target = bin_str.replace('\\', "/").to_lowercase();
            if normalized.trim_end_matches('/') == target.trim_end_matches('/') {
                return true;
            }
        }
    }
    false
}

/// Add ~/.warden/bin to PATH permanently
pub fn add_to_path() -> Result<String, String> {
    let bin = bin_dir();
    let bin_str = bin.to_string_lossy().to_string();

    if is_on_path() {
        return Ok("Already on PATH".to_string());
    }

    #[cfg(windows)]
    {
        add_to_path_windows(&bin_str)
    }

    #[cfg(not(windows))]
    {
        add_to_path_unix(&bin_str)
    }
}

/// Windows: add to user PATH via registry
#[cfg(windows)]
fn add_to_path_windows(bin_path: &str) -> Result<String, String> {
    use std::process::Command;

    // Use PowerShell to modify user PATH (no admin needed)
    let script = format!(
        r#"$current = [Environment]::GetEnvironmentVariable('PATH', 'User');
if ($current -and -not $current.Contains('{}')) {{
    $new = $current + ';{}';
    [Environment]::SetEnvironmentVariable('PATH', $new, 'User');
    Write-Output 'added';
}} elseif (-not $current) {{
    [Environment]::SetEnvironmentVariable('PATH', '{}', 'User');
    Write-Output 'added';
}} else {{
    Write-Output 'exists';
}}"#,
        bin_path.replace('/', "\\"),
        bin_path.replace('/', "\\"),
        bin_path.replace('/', "\\"),
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .map_err(|e| format!("PowerShell failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if stdout == "added" {
        Ok(format!(
            "Added {} to user PATH.\nRestart your terminal for changes to take effect.",
            bin_path
        ))
    } else {
        Ok("Already on PATH".to_string())
    }
}

/// Unix: append export to shell config files
#[cfg(not(windows))]
fn add_to_path_unix(bin_path: &str) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let export_line = format!("\n# Warden\nexport PATH=\"{}:$PATH\"\n", bin_path);

    let mut updated = Vec::new();

    // Try all common shell configs
    let configs = [
        format!("{}/.bashrc", home),
        format!("{}/.zshrc", home),
        format!("{}/.profile", home),
    ];

    for config_path in &configs {
        let path = std::path::Path::new(config_path);
        if !path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(path).unwrap_or_default();
        if content.contains(bin_path) {
            continue; // Already added
        }

        if std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(export_line.as_bytes())
            })
            .is_ok()
        {
            updated.push(config_path.clone());
        }
    }

    // Fish shell
    let fish_config = format!("{}/.config/fish/config.fish", home);
    if std::path::Path::new(&fish_config).exists() {
        let content = std::fs::read_to_string(&fish_config).unwrap_or_default();
        if !content.contains(bin_path) {
            let fish_line = format!("\n# Warden\nset -gx PATH {} $PATH\n", bin_path);
            if std::fs::OpenOptions::new()
                .append(true)
                .open(&fish_config)
                .and_then(|mut f| {
                    use std::io::Write;
                    f.write_all(fish_line.as_bytes())
                })
                .is_ok()
            {
                updated.push(fish_config);
            }
        }
    }

    if updated.is_empty() {
        Ok("Already on PATH (or no shell config found)".to_string())
    } else {
        Ok(format!(
            "Added to PATH in: {}\nRestart your terminal for changes to take effect.",
            updated.join(", ")
        ))
    }
}

/// Remove ~/.warden/bin from PATH (for uninstall)
#[allow(dead_code)]
pub fn remove_from_path() -> Result<String, String> {
    #[cfg(windows)]
    {
        let bin = bin_dir();
        let bin_str = bin.to_string_lossy().replace('/', "\\");
        let script = format!(
            r#"$current = [Environment]::GetEnvironmentVariable('PATH', 'User');
$new = ($current -split ';' | Where-Object {{ $_ -ne '{}' }}) -join ';';
[Environment]::SetEnvironmentVariable('PATH', $new, 'User');"#,
            bin_str
        );
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .map_err(|e| format!("PowerShell failed: {}", e))?;
        Ok("Removed from user PATH".to_string())
    }

    #[cfg(not(windows))]
    {
        Ok("Remove the Warden PATH line from your shell config manually".to_string())
    }
}
