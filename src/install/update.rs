// ─── update — real self-update system ────────────────────────────────────────
//
// Detects install method, checks for updates, and performs platform-specific
// binary replacement. `warden update` defaults to --check. `warden update --apply`
// performs the actual upgrade.
// ──────────────────────────────────────────────────────────────────────────────

use super::term;
use std::path::PathBuf;

/// How Warden was installed on this system
#[derive(Debug, Clone, PartialEq)]
pub enum InstallMethod {
    /// Installed via `cargo install warden-ai`
    Cargo,
    /// Standalone binary in ~/.warden/bin/
    Standalone,
    /// npm wrapper (`@bitmilldev/warden`)
    Npm,
    /// Unknown — can't determine install method
    Unknown,
}

impl std::fmt::Display for InstallMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cargo => write!(f, "cargo"),
            Self::Standalone => write!(f, "standalone binary"),
            Self::Npm => write!(f, "npm"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Detect how Warden was installed
pub fn detect_install_method() -> InstallMethod {
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_str = exe.to_string_lossy().to_lowercase();

    // Check for cargo install path pattern
    if exe_str.contains(".cargo") && exe_str.contains("bin") {
        return InstallMethod::Cargo;
    }

    // Check for npm global path patterns
    if exe_str.contains("node_modules") || exe_str.contains("npm") || exe_str.contains("npx") {
        return InstallMethod::Npm;
    }

    // Check for standalone install in ~/.warden/bin/
    if exe_str.contains(".warden") && exe_str.contains("bin") {
        return InstallMethod::Standalone;
    }

    // Check if npm package marker exists near the binary
    let exe_dir = exe.parent().unwrap_or(std::path::Path::new("."));
    if exe_dir.join("package.json").exists() {
        return InstallMethod::Npm;
    }

    InstallMethod::Unknown
}

/// Parsed version from GitHub release
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub url: String,
}

/// Check the latest version from GitHub Releases API
pub fn check_latest() -> Option<ReleaseInfo> {
    let output = if cfg!(windows) {
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Invoke-RestMethod -Uri 'https://api.github.com/repos/ekud12/warden/releases/latest' -Headers @{'User-Agent'='warden'}).tag_name",
            ])
            .output()
            .ok()?
    } else {
        std::process::Command::new("sh")
            .args([
                "-c",
                "curl -sL -H 'User-Agent: warden' https://api.github.com/repos/ekud12/warden/releases/latest | grep -o '\"tag_name\":\"[^\"]*\"' | head -1 | cut -d'\"' -f4",
            ])
            .output()
            .ok()?
    };

    let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if tag.is_empty() || !tag.starts_with('v') {
        return None;
    }

    let version = tag.trim_start_matches('v').to_string();
    let url = format!("https://github.com/ekud12/warden/releases/tag/{}", tag);

    Some(ReleaseInfo { tag, version, url })
}

/// Compare semantic versions. Returns true if `latest` is newer than `current`.
pub fn is_newer(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v
            .trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };

    let c = parse(current);
    let l = parse(latest);

    l > c
}

/// Run the update check and print results
pub fn run_check() {
    let current = env!("CARGO_PKG_VERSION");
    let method = detect_install_method();

    eprintln!();
    term::print_colored(term::BRAND, "  Warden Update\n");
    term::print_colored(term::DIM, &format!("  Installed via: {}\n", method));
    term::print_colored(term::DIM, &format!("  Current: v{}\n", current));
    eprintln!();

    let spinner = term::Spinner::start("Checking for updates...");
    let release = check_latest();
    spinner.finish_ok("done");

    match release {
        Some(info) if is_newer(current, &info.version) => {
            term::print_colored(term::SUCCESS, &format!("  New version available: v{}\n", info.version));
            term::print_colored(term::DIM, &format!("  Release: {}\n", info.url));
            eprintln!();
            term::print_colored(term::TEXT, "  Upgrade:\n");
            print_upgrade_instructions(&method, &info);
            eprintln!();
        }
        Some(_) => {
            term::print_colored(term::SUCCESS, "  Already on the latest version.\n");
            eprintln!();
        }
        None => {
            term::print_colored(term::WARN, "  Could not check for updates.\n");
            term::hint("Check https://github.com/ekud12/warden/releases manually.");
            eprintln!();
        }
    }
}

/// Run the actual update
pub fn run_apply() {
    let current = env!("CARGO_PKG_VERSION");
    let method = detect_install_method();

    eprintln!();
    term::print_colored(term::BRAND, "  Warden Update — Apply\n");
    eprintln!();

    let spinner = term::Spinner::start("Checking for updates...");
    let release = check_latest();
    spinner.finish_ok("done");

    let info = match release {
        Some(info) if is_newer(current, &info.version) => info,
        Some(_) => {
            term::print_colored(term::SUCCESS, "  Already on the latest version.\n");
            eprintln!();
            return;
        }
        None => {
            term::print_colored(term::WARN, "  Could not check for updates.\n");
            eprintln!();
            return;
        }
    };

    term::print_colored(term::TEXT, &format!("  Upgrading v{} → v{}\n", current, info.version));
    eprintln!();

    match method {
        InstallMethod::Cargo => apply_cargo(&info),
        InstallMethod::Npm => apply_npm(&info),
        InstallMethod::Standalone => apply_standalone(&info),
        InstallMethod::Unknown => {
            term::print_colored(term::WARN, "  Cannot auto-update: unknown install method.\n");
            term::hint("Update manually from https://github.com/ekud12/warden/releases");
            eprintln!();
        }
    }
}

fn apply_cargo(info: &ReleaseInfo) {
    let spinner = term::Spinner::start("Running cargo install --locked --force warden-ai...");
    let result = std::process::Command::new("cargo")
        .args(["install", "--locked", "--force", "warden-ai"])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            spinner.finish_ok("installed");
            term::print_colored(term::SUCCESS, &format!("  Updated to v{}\n", info.version));
            post_update_verify();
        }
        Ok(output) => {
            spinner.finish_fail("failed");
            let stderr = String::from_utf8_lossy(&output.stderr);
            term::print_colored(term::ERROR, &format!("  cargo install failed: {}\n", stderr.lines().last().unwrap_or("")));
        }
        Err(e) => {
            spinner.finish_fail("failed");
            term::print_colored(term::ERROR, &format!("  Could not run cargo: {}\n", e));
        }
    }
    eprintln!();
}

fn apply_npm(_info: &ReleaseInfo) {
    let spinner = term::Spinner::start("Running npm update -g @bitmilldev/warden...");
    let result = std::process::Command::new("npm")
        .args(["update", "-g", "@bitmilldev/warden"])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            spinner.finish_ok("updated");
            term::print_colored(term::SUCCESS, "  npm package updated.\n");
            post_update_verify();
        }
        Ok(output) => {
            spinner.finish_fail("failed");
            let stderr = String::from_utf8_lossy(&output.stderr);
            term::print_colored(term::ERROR, &format!("  npm update failed: {}\n", stderr.lines().last().unwrap_or("")));
        }
        Err(e) => {
            spinner.finish_fail("failed");
            term::print_colored(term::ERROR, &format!("  Could not run npm: {}\n", e));
        }
    }
    eprintln!();
}

fn apply_standalone(info: &ReleaseInfo) {
    // Stop daemon first (if running)
    let _ = stop_daemon();

    let target = detect_target();
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let asset_name = format!("warden-{}{}", target, ext);
    let download_url = format!(
        "https://github.com/ekud12/warden/releases/download/{}/{}",
        info.tag, asset_name
    );

    let spinner = term::Spinner::start(&format!("Downloading {}...", asset_name));

    let exe = std::env::current_exe().unwrap_or_default();
    let tmp = exe.with_extension("tmp");

    // Download to temp file
    let success = if cfg!(windows) {
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Invoke-WebRequest -Uri '{}' -OutFile '{}' -Headers @{{'User-Agent'='warden'}}",
                    download_url,
                    tmp.to_string_lossy()
                ),
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        std::process::Command::new("sh")
            .args([
                "-c",
                &format!(
                    "curl -sL -H 'User-Agent: warden' -o '{}' '{}'",
                    tmp.to_string_lossy(),
                    download_url
                ),
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };

    if !success || !tmp.exists() || std::fs::metadata(&tmp).map(|m| m.len() < 1_000_000).unwrap_or(true) {
        spinner.finish_fail("Download failed or file too small");
        term::print_colored(term::ERROR, "  Download failed or file too small.\n");
        let _ = std::fs::remove_file(&tmp);
        eprintln!();
        return;
    }

    spinner.finish_ok("downloaded");

    // Swap binary
    let backup = exe.with_extension("bak");
    if cfg!(windows) {
        // Windows: can't replace running exe, rename first
        let _ = std::fs::remove_file(&backup);
        if std::fs::rename(&exe, &backup).is_err() {
            term::print_colored(term::ERROR, "  Could not rename current binary. Is it in use?\n");
            let _ = std::fs::remove_file(&tmp);
            eprintln!();
            return;
        }
        if std::fs::rename(&tmp, &exe).is_err() {
            // Rollback
            let _ = std::fs::rename(&backup, &exe);
            term::print_colored(term::ERROR, "  Could not place new binary. Rolled back.\n");
            eprintln!();
            return;
        }
    } else {
        // Unix: atomic rename
        let _ = std::fs::rename(&exe, &backup);
        if std::fs::rename(&tmp, &exe).is_err() {
            let _ = std::fs::rename(&backup, &exe);
            term::print_colored(term::ERROR, "  Could not place new binary. Rolled back.\n");
            eprintln!();
            return;
        }
        // Restore execute permission
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
        }
    }

    term::print_colored(term::SUCCESS, &format!("  Updated to v{}\n", info.version));
    post_update_verify();
    eprintln!();
}

fn detect_target() -> &'static str {
    if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else {
        "x86_64-unknown-linux-gnu"
    }
}

fn stop_daemon() -> bool {
    // Try to stop daemon gracefully
    let exe = std::env::current_exe().unwrap_or_default();
    std::process::Command::new(&exe)
        .args(["debug-daemon-stop"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn post_update_verify() {
    eprintln!();
    term::print_colored(term::DIM, "  Verifying installation...\n");

    // Check version
    let exe = std::env::current_exe().unwrap_or_default();
    if let Ok(output) = std::process::Command::new(&exe).args(["version"]).output() {
        let version = String::from_utf8_lossy(&output.stdout);
        term::print_colored(term::DIM, &format!("  Version: {}", version.trim()));
        eprintln!();
    }

    // Check binary exists
    let bin_dir = super::bin_dir();
    let bin_name = if cfg!(windows) { "warden.exe" } else { "warden" };
    let binary = bin_dir.join(bin_name);
    if binary.exists() {
        term::print_colored(term::SUCCESS, "  Binary: OK\n");
    } else {
        term::print_colored(term::WARN, "  Binary: not found in ~/.warden/bin/\n");
    }
}

fn print_upgrade_instructions(method: &InstallMethod, info: &ReleaseInfo) {
    match method {
        InstallMethod::Cargo => {
            term::print_colored(term::DIM, "    cargo install --locked --force warden-ai\n");
            term::print_colored(term::DIM, "    (or: warden update --apply)\n");
        }
        InstallMethod::Npm => {
            term::print_colored(term::DIM, "    npm update -g @bitmilldev/warden\n");
            term::print_colored(term::DIM, "    (or: warden update --apply)\n");
        }
        InstallMethod::Standalone => {
            term::print_colored(term::DIM, "    warden update --apply\n");
            term::print_colored(term::DIM, &format!("    (downloads from {})\n", info.url));
        }
        InstallMethod::Unknown => {
            term::print_colored(term::DIM, &format!("    Download from: {}\n", info.url));
            term::print_colored(term::DIM, "    Or: cargo install --locked --force warden-ai\n");
        }
    }
}

/// Run `warden doctor` — verify installation health
pub fn run_doctor() {
    eprintln!();
    term::print_colored(term::BRAND, "  Warden Doctor\n");
    eprintln!();

    let mut ok_count = 0u32;
    let mut warn_count = 0u32;

    // 1. Binary
    let exe = std::env::current_exe().unwrap_or_default();
    if exe.exists() {
        term::print_colored(term::SUCCESS, "  [OK] ");
        term::print_colored(term::TEXT, &format!("Binary: {}\n", exe.display()));
        ok_count += 1;
    } else {
        term::print_colored(term::WARN, "  [!!] Binary not found\n");
        warn_count += 1;
    }

    // 2. Install method
    let method = detect_install_method();
    term::print_colored(term::SUCCESS, "  [OK] ");
    term::print_colored(term::TEXT, &format!("Install method: {}\n", method));
    ok_count += 1;

    // 3. Version
    term::print_colored(term::SUCCESS, "  [OK] ");
    term::print_colored(term::TEXT, &format!("Version: v{}\n", env!("CARGO_PKG_VERSION")));
    ok_count += 1;

    // 4. Home directory
    let home = super::home_dir();
    if home.exists() {
        term::print_colored(term::SUCCESS, "  [OK] ");
        term::print_colored(term::TEXT, &format!("Home: {}\n", home.display()));
        ok_count += 1;
    } else {
        term::print_colored(term::WARN, "  [!!] Home directory missing: ");
        term::print_colored(term::DIM, &format!("{}\n", home.display()));
        warn_count += 1;
    }

    // 5. Config
    let config_path = home.join("config.toml");
    if config_path.exists() {
        term::print_colored(term::SUCCESS, "  [OK] ");
        term::print_colored(term::TEXT, "Config: config.toml present\n");
        ok_count += 1;
    } else {
        term::print_colored(term::WARN, "  [!!] Config missing: ");
        term::print_colored(term::DIM, "run `warden init` to create\n");
        warn_count += 1;
    }

    // 6. Daemon
    let daemon_bin = super::bin_dir().join(if cfg!(windows) { "warden-daemon.exe" } else { "warden-daemon" });
    if daemon_bin.exists() {
        term::print_colored(term::SUCCESS, "  [OK] ");
        term::print_colored(term::TEXT, "Daemon binary: present\n");
        ok_count += 1;
    } else {
        term::print_colored(term::WARN, "  [!!] Daemon binary missing\n");
        warn_count += 1;
    }

    // 7. Claude Code hooks
    let claude_settings = dirs_check("claude");
    if let Some(status) = claude_settings {
        if status {
            term::print_colored(term::SUCCESS, "  [OK] ");
            term::print_colored(term::TEXT, "Claude Code: hooks configured\n");
            ok_count += 1;
        } else {
            term::print_colored(term::WARN, "  [!!] Claude Code: settings exists but no Warden hooks\n");
            warn_count += 1;
        }
    } else {
        term::print_colored(term::DIM, "  [--] Claude Code: not configured\n");
    }

    eprintln!();
    if warn_count == 0 {
        term::print_colored(term::SUCCESS, &format!("  All {} checks passed.\n", ok_count));
    } else {
        term::print_colored(term::WARN, &format!("  {} OK, {} warnings.\n", ok_count, warn_count));
    }
    eprintln!();
}

fn dirs_check(assistant: &str) -> Option<bool> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();

    let settings_path = match assistant {
        "claude" => PathBuf::from(&home).join(".claude").join("settings.json"),
        "gemini" => PathBuf::from(&home).join(".gemini").join("settings.json"),
        _ => return None,
    };

    if !settings_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&settings_path).ok()?;
    let has_warden = content.to_lowercase().contains("warden");
    Some(has_warden)
}
