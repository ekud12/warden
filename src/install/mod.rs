// ─── install — first-run wizard, PATH registration, assistant configuration ──
//
// Everything lives in ~/.warden/:
//   ~/.warden/bin/warden[.exe]        — main binary
//   ~/.warden/bin/warden-relay[.exe]  — IPC relay
//   ~/.warden/config.toml             — user config
//   ~/.warden/rules/                  — tiered rules
//   ~/.warden/projects/               — per-project state
//
// After install, ~/.warden/bin/ is on PATH — accessible from any shell.
// ──────────────────────────────────────────────────────────────────────────────

pub mod detect;
pub mod path;
pub mod term;
pub mod tools;
pub mod uninstall;
pub mod update;
pub mod wizard;

use crate::constants;
use std::fs;
use std::path::PathBuf;

/// Get the warden home directory: ~/.warden/
pub fn home_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("WARDEN_HOME") {
        return PathBuf::from(dir);
    }
    let user_home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(user_home).join(constants::DIR)
}

/// Get the bin directory: ~/.warden/bin/
pub fn bin_dir() -> PathBuf {
    home_dir().join("bin")
}

/// Ensure the full directory structure exists
pub fn ensure_dirs() -> std::io::Result<()> {
    let home = home_dir();
    fs::create_dir_all(home.join("bin"))?;
    fs::create_dir_all(home.join("rules"))?;
    fs::create_dir_all(home.join("projects"))?;
    Ok(())
}

/// Install the current binary to ~/.warden/bin/
/// Copies the running executable to the standard location.
/// Stops the old daemon first to prevent file locks and stale processes.
pub fn install_binary() -> Result<(), String> {
    // Stop old daemon before overwriting binary — prevents file locks on Windows
    // and ensures the next session starts fresh with the new version.
    crate::runtime::ipc::stop_daemon_graceful(2000);

    let source =
        std::env::current_exe().map_err(|e| format!("Cannot determine current exe: {}", e))?;

    let dest_dir = bin_dir();
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Cannot create {}: {}", dest_dir.display(), e))?;

    let binary_name = if cfg!(windows) {
        "warden.exe"
    } else {
        "warden"
    };
    let dest = dest_dir.join(binary_name);

    // Don't copy over ourselves
    if source == dest {
        return Ok(());
    }

    fs::copy(&source, &dest).map_err(|e| format!("Cannot copy binary: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o755));
    }

    // Unify: replace the source binary (e.g., ~/.cargo/bin/warden) with a
    // hardlink/copy pointing to ~/.warden/bin/warden. This ensures `warden`
    // on PATH always resolves to the same binary that hooks use.
    unify_binary_location(&source, &dest);

    // Also copy the relay binary if it exists next to the source
    let relay_name = if cfg!(windows) {
        "warden-relay.exe"
    } else {
        "warden-relay"
    };
    let relay_src = source
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join(relay_name);
    let relay_dest = dest_dir.join(relay_name);
    if relay_src.exists() && relay_src != relay_dest {
        let _ = fs::copy(&relay_src, &relay_dest);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&relay_dest, fs::Permissions::from_mode(0o755));
        }
    }

    Ok(())
}

/// Unify binary locations: replace the original install location (cargo/npm/standalone)
/// with a copy of the canonical binary from ~/.warden/bin/. This ensures that `warden`
/// on PATH always matches what hooks use, regardless of install method.
fn unify_binary_location(original: &std::path::Path, canonical: &std::path::Path) {
    // Skip if source IS the canonical location
    if original == canonical {
        return;
    }

    // Skip if source doesn't exist or isn't in a known install directory
    if !original.exists() {
        return;
    }

    // Check if source is in a package manager bin dir (cargo, npm, standalone)
    let source_str = original.to_string_lossy().replace('\\', "/").to_lowercase();
    let is_managed = source_str.contains(".cargo/bin")
        || source_str.contains("node_modules")
        || source_str.contains("npm")
        || source_str.contains("appdata");

    if !is_managed {
        return;
    }

    // Replace the original with a copy of the canonical binary.
    // We use copy (not symlink) because:
    //   - Windows symlinks require admin/developer mode
    //   - Hardlinks don't work across volumes
    //   - A copy is universally portable
    match fs::copy(canonical, original) {
        Ok(_) => {
            crate::common::log(
                "install",
                &format!(
                    "Unified: {} → copy of {}",
                    original.display(),
                    canonical.display()
                ),
            );
        }
        Err(e) => {
            crate::common::log(
                "install",
                &format!(
                    "Cannot unify {}: {} (binary may be locked)",
                    original.display(),
                    e
                ),
            );
        }
    }
}

/// Write default config.toml if it doesn't exist
pub fn write_default_config() -> Result<(), String> {
    let config_path = home_dir().join(constants::CONFIG_FILE);
    if config_path.exists() {
        return Ok(());
    }

    let content = r#"# Warden configuration
# See: https://github.com/user/warden#configuration

[assistant]
# Auto-detect from environment, or set explicitly:
# "claude-code", "gemini-cli", "auto"
type = "auto"

[tools]
# Auto-detected on init. Set to false to disable specific integrations.
# justfile = true
# rg = true
# fd = true
# bat = true

[restrictions]
# Disable specific restrictions by ID:
# disabled = ["substitution.cat", "read.post-edit"]

[telemetry]
# All features on by default. Set to false to opt-out:
# anomaly_detection = true
# quality_predictor = true
# cost_tracking = true
# error_prevention = true
# token_forecast = true
# smart_truncation = true
# project_dna = true
# rule_effectiveness = true
# drift_velocity = true
# compaction_optimizer = true
# command_recovery = true
"#;

    fs::write(&config_path, content).map_err(|e| format!("Cannot write config: {}", e))?;
    Ok(())
}
