// ─── warden-relay — windowless hook shim ──────────────────────────────────────
//
// Thin relay that forwards hook invocations to warden.exe without creating
// a visible console window on Windows. Inherits stdin/stdout/stderr directly
// from the parent process (Claude Code) — no buffering, no hang risk.
//
// The #![windows_subsystem = "windows"] attribute prevents CMD flicker.
// ──────────────────────────────────────────────────────────────────────────────

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::process::{Command, Stdio};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Find warden.exe next to this relay binary
    let relay_path = std::env::current_exe().unwrap_or_default();
    let bin_dir = relay_path.parent().unwrap_or(std::path::Path::new("."));
    let warden = bin_dir.join(if cfg!(windows) {
        "warden.exe"
    } else {
        "warden"
    });

    if !warden.exists() {
        std::process::exit(0); // Fail open — never block the AI
    }

    // Spawn warden with inherited I/O — transparent passthrough, no buffering
    let mut cmd = Command::new(&warden);
    cmd.args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(0)),
        Err(_) => std::process::exit(0), // Fail open
    }
}
