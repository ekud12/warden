// ─── server — persistent warden background server ────────────────────────────
//
// Thin entry point for the unified warden server. Wraps daemon.rs logic.
// Called via: warden.exe __server
// Spawned by: relay.exe (on first hook call) or CLI (daemon-start)
// ──────────────────────────────────────────────────────────────────────────────

/// Start the persistent warden server (blocks until idle timeout or shutdown).
pub fn run() {
    let mtime = super::ipc::get_binary_mtime();
    super::daemon::run_server(mtime);
}

/// Spawn warden.exe __server as a detached background process.
/// No binary copy needed — runs the same warden.exe in server mode.
pub fn spawn() {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return,
    };

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;
        match std::process::Command::new(&exe)
            .args(["__server"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()
        {
            Ok(child) => {
                crate::common::log("server", &format!("Server spawned (pid={})", child.id()));
            }
            Err(e) => {
                crate::common::log("server", &format!("Failed to spawn server: {}", e));
            }
        }
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt;
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(["__server"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        match cmd.spawn() {
            Ok(child) => {
                crate::common::log("server", &format!("Server spawned (pid={})", child.id()));
            }
            Err(e) => {
                crate::common::log("server", &format!("Failed to spawn server: {}", e));
            }
        }
    }
}
