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

/// Run the update flow.
///
/// Default (`warden update`): check for updates, prompt to apply if available.
/// `--check`: print-only, no prompt.
/// `--yes`: skip prompt, apply directly (CI-friendly).
pub fn run(args: &[String]) {
    let check_only = args.iter().any(|a| a == "--check");
    let auto_yes = args.iter().any(|a| a == "--yes" || a == "-y");

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

    let info = match release {
        Some(info) if is_newer(current, &info.version) => info,
        Some(_) => {
            term::print_colored(term::SUCCESS, "  Already on the latest version.\n");
            eprintln!();
            return;
        }
        None => {
            term::print_colored(term::WARN, "  Could not check for updates.\n");
            term::hint("Check https://github.com/ekud12/warden/releases manually.");
            eprintln!();
            return;
        }
    };

    term::print_colored(
        term::SUCCESS,
        &format!("  New version available: v{}\n", info.version),
    );
    term::print_colored(term::DIM, &format!("  Release: {}\n", info.url));
    eprintln!();

    if check_only {
        term::print_colored(term::TEXT, "  Upgrade:\n");
        print_upgrade_instructions(&method, &info);
        eprintln!();
        return;
    }

    // Interactive prompt (or auto-yes)
    let should_apply =
        auto_yes || term::confirm(&format!("  Update v{} → v{}?", current, info.version), true);

    if !should_apply {
        term::print_colored(term::DIM, "  Update skipped.\n");
        eprintln!();
        return;
    }

    eprintln!();
    match method {
        InstallMethod::Cargo => apply_cargo(&info),
        InstallMethod::Npm => apply_npm(&info),
        InstallMethod::Standalone => apply_standalone(&info),
        InstallMethod::Unknown => {
            term::print_colored(
                term::WARN,
                "  Cannot auto-update: unknown install method.\n",
            );
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
            term::print_colored(
                term::ERROR,
                &format!(
                    "  cargo install failed: {}\n",
                    stderr.lines().last().unwrap_or("")
                ),
            );
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
            term::print_colored(
                term::ERROR,
                &format!(
                    "  npm update failed: {}\n",
                    stderr.lines().last().unwrap_or("")
                ),
            );
        }
        Err(e) => {
            spinner.finish_fail("failed");
            term::print_colored(term::ERROR, &format!("  Could not run npm: {}\n", e));
        }
    }
    eprintln!();
}

fn apply_standalone(info: &ReleaseInfo) {
    // Stop daemon first (if running) — graceful IPC shutdown with 3s timeout
    crate::runtime::ipc::stop_daemon_graceful(3000);

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

    if !success
        || !tmp.exists()
        || std::fs::metadata(&tmp)
            .map(|m| m.len() < 1_000_000)
            .unwrap_or(true)
    {
        spinner.finish_fail("Download failed or file too small");
        term::print_colored(term::ERROR, "  Download failed or file too small.\n");
        let _ = std::fs::remove_file(&tmp);
        eprintln!();
        return;
    }

    spinner.finish_ok("downloaded");

    // Verify SHA-256 checksum against published checksums-sha256.txt
    let checksums_url = format!(
        "https://github.com/ekud12/warden/releases/download/{}/checksums-sha256.txt",
        info.tag
    );
    match verify_checksum(&tmp, &checksums_url, &asset_name) {
        Ok(true) => {
            term::print_colored(term::SUCCESS, "  ✓ Checksum verified\n");
        }
        Ok(false) => {
            term::print_colored(
                term::ERROR,
                "  ✗ Checksum mismatch — downloaded binary does not match published hash.\n",
            );
            term::print_colored(
                term::ERROR,
                "    This could indicate a corrupted download or tampered release.\n",
            );
            let _ = std::fs::remove_file(&tmp);
            eprintln!();
            return;
        }
        Err(msg) => {
            // Warn but don't block — checksums file may not exist for older releases
            term::print_colored(
                term::WARN,
                &format!("  ⚠ Could not verify checksum: {}\n", msg),
            );
        }
    }

    // Swap binary with rollback guarantee
    if let Err(msg) = swap_binary(&exe, &tmp) {
        term::print_colored(term::ERROR, &format!("  {}\n", msg));
        eprintln!();
        return;
    }

    term::print_colored(term::SUCCESS, &format!("  Updated to v{}\n", info.version));
    post_update_verify();

    // Restart daemon with the new binary
    crate::runtime::ipc::spawn_daemon();

    eprintln!();
}

/// Swap the current binary with a new one, rolling back on failure.
/// Returns Ok(()) on success, Err(message) on failure (backup restored).
fn swap_binary(exe: &std::path::Path, tmp: &std::path::Path) -> Result<(), String> {
    let backup = exe.with_extension("bak");

    // Remove stale backup
    let _ = std::fs::remove_file(&backup);

    // Step 1: move current exe to backup
    if let Err(e) = std::fs::rename(exe, &backup) {
        let _ = std::fs::remove_file(tmp);
        return Err(format!(
            "Could not rename current binary: {}. Is it in use?",
            e
        ));
    }

    // Step 2: move new binary into place
    if let Err(e) = std::fs::rename(tmp, exe) {
        // ALWAYS restore backup on failure
        if let Err(restore_err) = std::fs::rename(&backup, exe) {
            return Err(format!(
                "Could not place new binary ({}), AND failed to restore backup ({}). \
                 Manual recovery needed: rename {} to {}",
                e,
                restore_err,
                backup.display(),
                exe.display()
            ));
        }
        return Err(format!("Could not place new binary: {}. Rolled back.", e));
    }

    // Step 3: restore execute permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(exe, std::fs::Permissions::from_mode(0o755));
    }

    Ok(())
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

/// Verify SHA-256 checksum of a downloaded file against published checksums.
/// Returns Ok(true) if match, Ok(false) if mismatch, Err if checksums unavailable.
fn verify_checksum(
    file: &std::path::Path,
    checksums_url: &str,
    asset_name: &str,
) -> Result<bool, String> {
    use sha2::{Digest, Sha256};

    // Fetch checksums file
    let checksums_output = if cfg!(windows) {
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "(Invoke-WebRequest -Uri '{}' -Headers @{{'User-Agent'='warden'}}).Content",
                    checksums_url
                ),
            ])
            .output()
    } else {
        std::process::Command::new("sh")
            .args([
                "-c",
                &format!("curl -sL -H 'User-Agent: warden' '{}'", checksums_url),
            ])
            .output()
    };

    let output = checksums_output.map_err(|e| format!("fetch failed: {}", e))?;
    if !output.status.success() {
        return Err("checksums file not available for this release".to_string());
    }

    let checksums_text = String::from_utf8_lossy(&output.stdout);
    if checksums_text.trim().is_empty() {
        return Err("checksums file is empty".to_string());
    }

    // Parse checksums file (format: "hash  filename" per line)
    let expected_hash = checksums_text
        .lines()
        .find_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == asset_name {
                Some(parts[0].to_lowercase())
            } else {
                None
            }
        })
        .ok_or_else(|| format!("no checksum found for {}", asset_name))?;

    // Compute SHA-256 of downloaded file
    let file_bytes = std::fs::read(file).map_err(|e| format!("read failed: {}", e))?;
    let mut hasher = Sha256::new();
    hasher.update(&file_bytes);
    let actual_hash = format!("{:x}", hasher.finalize());

    Ok(actual_hash == expected_hash)
}

fn post_update_verify() {
    eprintln!();
    term::print_colored(term::DIM, "  Verifying installation...\n");

    let bin_dir = super::bin_dir();
    let bin_name = if cfg!(windows) {
        "warden.exe"
    } else {
        "warden"
    };
    let canonical = bin_dir.join(bin_name);
    let exe = std::env::current_exe().unwrap_or_default();

    // Sync: if the updated binary is NOT in ~/.warden/bin/, copy it there
    if exe != canonical && exe.exists() {
        let _ = std::fs::create_dir_all(&bin_dir);
        if std::fs::copy(&exe, &canonical).is_ok() {
            term::print_colored(term::DIM, "  Synced to ~/.warden/bin/\n");
        }
    }

    // Sync: if the updated binary IS in ~/.warden/bin/, copy back to origin
    // so `warden` on PATH is also up to date
    if exe != canonical && canonical.exists() && std::fs::copy(&canonical, &exe).is_ok() {
        term::print_colored(term::DIM, "  Synced back to install location\n");
    }

    // Check version
    if let Ok(output) = std::process::Command::new(&canonical)
        .args(["version"])
        .output()
    {
        let version = String::from_utf8_lossy(&output.stdout);
        term::print_colored(term::DIM, &format!("  Version: {}", version.trim()));
        eprintln!();
    }

    if canonical.exists() {
        term::print_colored(term::SUCCESS, "  Binary: OK\n");
    } else {
        term::print_colored(term::WARN, "  Binary: not found in ~/.warden/bin/\n");
    }

    // Also sync relay binary
    let relay_name = if cfg!(windows) {
        "warden-relay.exe"
    } else {
        "warden-relay"
    };
    let relay_src = exe
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join(relay_name);
    let relay_dest = bin_dir.join(relay_name);
    if relay_src.exists() && relay_src != relay_dest {
        let _ = std::fs::copy(&relay_src, &relay_dest);
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
            term::print_colored(
                term::DIM,
                "    Or: cargo install --locked --force warden-ai\n",
            );
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

    // 1. CLI binary
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
    let cli_version = env!("CARGO_PKG_VERSION");
    term::print_colored(term::SUCCESS, "  [OK] ");
    term::print_colored(term::TEXT, &format!("Version: v{}\n", cli_version));
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

    // 6. Installed binaries — check all 3 exist in bin_dir and hooks_dir
    let bin_dir = super::bin_dir();
    let _hooks_dir = crate::common::hooks_dir();
    let ext = if cfg!(windows) { ".exe" } else { "" };

    let bin_binaries = [
        (format!("warden{}", ext), "CLI"),
        (format!("warden-relay{}", ext), "Relay"),
    ];
    for (name, label) in &bin_binaries {
        let path = bin_dir.join(name);
        if path.exists() {
            term::print_colored(term::SUCCESS, "  [OK] ");
            term::print_colored(term::TEXT, &format!("{} binary: present", label));
            term::print_colored(term::DIM, &format!(" ({})\n", path.display()));
            ok_count += 1;
        } else {
            term::print_colored(term::WARN, "  [!!] ");
            term::print_colored(term::TEXT, &format!("{} binary missing: ", label));
            term::print_colored(term::DIM, &format!("{}\n", path.display()));
            warn_count += 1;
        }
    }

    // 7. Server process health — check if running, query version via IPC
    doctor_server_health(&mut ok_count, &mut warn_count, cli_version);

    // 9. Claude Code hooks
    let claude_settings = dirs_check("claude");
    if let Some(status) = claude_settings {
        if status {
            term::print_colored(term::SUCCESS, "  [OK] ");
            term::print_colored(term::TEXT, "Claude Code: hooks configured\n");
            ok_count += 1;
        } else {
            term::print_colored(
                term::WARN,
                "  [!!] Claude Code: settings exists but no Warden hooks\n",
            );
            warn_count += 1;
        }
    } else {
        term::print_colored(term::DIM, "  [--] Claude Code: not configured\n");
    }

    // 10. Stale global files check
    let stale_files = crate::cli::find_stale_global_files(&home);
    if stale_files.is_empty() {
        term::print_colored(term::SUCCESS, "  [OK] ");
        term::print_colored(term::TEXT, "No stale global files\n");
        ok_count += 1;
    } else {
        term::print_colored(
            term::WARN,
            &format!(
                "  [!!] {} stale file(s) detected. Run `warden cleanup` to remove.\n",
                stale_files.len()
            ),
        );
        for (path, reason) in &stale_files {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            term::print_colored(term::DIM, &format!("       {} — {}\n", name, reason));
        }
        warn_count += 1;
    }

    eprintln!();
    if warn_count == 0 {
        term::print_colored(
            term::SUCCESS,
            &format!("  All {} checks passed.\n", ok_count),
        );
    } else {
        term::print_colored(
            term::WARN,
            &format!("  {} OK, {} warnings.\n", ok_count, warn_count),
        );
    }
    eprintln!();
}

/// Check server process health: running status, PID, uptime, version match
fn doctor_server_health(ok_count: &mut u32, warn_count: &mut u32, cli_version: &str) {
    // Check PID file first
    let pid = crate::runtime::ipc::read_pid();

    match crate::runtime::ipc::try_daemon("server-status", "") {
        Some(resp) if resp.exit_code == 0 => {
            // Parse the status JSON
            let status: serde_json::Value = serde_json::from_str(&resp.stdout).unwrap_or_default();
            let server_pid = status.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
            let server_version = status
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let started_at = status
                .get("started_at")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            // Running status
            term::print_colored(term::SUCCESS, "  [OK] ");
            term::print_colored(term::TEXT, &format!("Server: running (PID {})", server_pid));

            // Uptime
            if started_at > 0 {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let uptime_secs = now.saturating_sub(started_at);
                term::print_colored(
                    term::DIM,
                    &format!(", uptime {}\n", format_duration(uptime_secs)),
                );
            } else {
                eprintln!();
            }
            *ok_count += 1;

            // Version match
            if server_version == cli_version {
                term::print_colored(term::SUCCESS, "  [OK] ");
                term::print_colored(
                    term::TEXT,
                    &format!("Server version: v{} (matches CLI)\n", server_version),
                );
                *ok_count += 1;
            } else {
                term::print_colored(term::WARN, "  [!!] ");
                term::print_colored(
                    term::TEXT,
                    &format!(
                        "Server version mismatch: server=v{}, CLI=v{}\n",
                        server_version, cli_version
                    ),
                );
                term::print_colored(
                    term::DIM,
                    "       Run `warden server-stop` — it will auto-restart with the correct version\n",
                );
                *warn_count += 1;
            }
        }
        _ => {
            // Daemon not reachable via IPC — try to auto-start
            let alive_but_unresponsive =
                pid.map(crate::runtime::ipc::pid_is_alive).unwrap_or(false);

            if alive_but_unresponsive {
                let pid_val = pid.unwrap();
                term::print_colored(term::WARN, "  [!!] ");
                term::print_colored(
                    term::TEXT,
                    &format!("Server: PID {} alive but not responding on pipe\n", pid_val),
                );
                *warn_count += 1;
            } else {
                // Server not running — attempt auto-start
                term::print_colored(
                    term::DIM,
                    "  [--] Server: not running, attempting start...\n",
                );
                crate::runtime::server::spawn();
                std::thread::sleep(std::time::Duration::from_millis(500));

                // Retry health check after spawn
                match crate::runtime::ipc::try_daemon("server-status", "") {
                    Some(resp) if resp.exit_code == 0 => {
                        let status: serde_json::Value =
                            serde_json::from_str(&resp.stdout).unwrap_or_default();
                        let server_pid = status.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
                        let server_version = status
                            .get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");

                        term::print_colored(term::SUCCESS, "  [OK] ");
                        term::print_colored(
                            term::TEXT,
                            &format!("Server: started (PID {})\n", server_pid),
                        );
                        *ok_count += 1;

                        if server_version == cli_version {
                            term::print_colored(term::SUCCESS, "  [OK] ");
                            term::print_colored(
                                term::TEXT,
                                &format!("Server version: v{} (matches CLI)\n", server_version),
                            );
                            *ok_count += 1;
                        } else {
                            term::print_colored(term::WARN, "  [!!] ");
                            term::print_colored(
                                term::TEXT,
                                &format!(
                                    "Server version mismatch: server=v{}, CLI=v{}\n",
                                    server_version, cli_version
                                ),
                            );
                            *warn_count += 1;
                        }
                    }
                    _ => {
                        term::print_colored(term::WARN, "  [!!] ");
                        term::print_colored(term::TEXT, "Server: failed to start (check logs)\n");
                        *warn_count += 1;
                    }
                }
            }
        }
    }
}

/// Format seconds into a human-readable duration string
fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {}m", h, m)
    } else {
        let d = secs / 86400;
        let h = (secs % 86400) / 3600;
        format!("{}d {}h", d, h)
    }
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
