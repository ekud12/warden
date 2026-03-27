// ─── ipc — Named pipe IPC client for daemon communication ───────────────────
//
// Provides the client side of warden's transparent daemon protocol.
// Each hook invocation tries the daemon first (fast path ~0.5ms), falling
// back to direct execution if the daemon isn't running.
//
// Protocol: Single JSON request → single JSON response per connection.
// The request/response format matches the current stdin/stdout JSON format.
//
// Named pipe: \\.\pipe\{PIPE_PREFIX}-{username}
// ──────────────────────────────────────────────────────────────────────────────

use std::io::{Read, Write};
use std::time::Duration;

#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;

/// IPC request sent to daemon
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DaemonRequest {
    pub subcmd: String,
    pub payload: String,
    /// Binary mtime — daemon uses this to detect rebuilds
    #[serde(default)]
    pub binary_mtime: u64,
    /// CWD of the calling process — used for per-project state isolation
    #[serde(default)]
    pub cwd: String,
    /// Rules.toml mtime -- daemon uses this to detect rule file changes
    #[serde(default)]
    pub rules_mtime: u64,
    /// Client version — daemon uses this to detect version mismatches
    #[serde(default)]
    pub version: String,
}

/// Special exit code: daemon detected binary rebuild, client should restart
pub const EXIT_RESTART: i32 = -2;

/// IPC response from daemon
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DaemonResponse {
    pub stdout: String,
    pub exit_code: i32,
}

/// Get the named pipe path for this user
pub fn pipe_name() -> String {
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\{}-{}", crate::constants::PIPE_PREFIX, username)
    }
    #[cfg(not(windows))]
    {
        format!("/tmp/{}-{}.sock", crate::constants::PIPE_PREFIX, username)
    }
}

/// Get the modification time of the current binary as epoch seconds.
/// Returns 0 if unable to determine (safe — daemon will never match 0).
pub fn get_binary_mtime() -> u64 {
    std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Total round-trip timeout for IPC exchange (connect + write + read).
/// Prevents hangs even if daemon accepts connection but never responds.
const IPC_TIMEOUT: Duration = Duration::from_millis(2000);

/// Try to send a request to the daemon and get a response.
/// Returns None if daemon isn't running or communication fails.
/// If daemon appears dead (stale pidfile), attempts auto-restart with storm protection.
///
/// The entire exchange is wrapped in a 2-second timeout via channel — if the daemon
/// accepts our connection but hangs during processing, the caller is never blocked.
pub fn try_daemon(subcmd: &str, payload: &str) -> Option<DaemonResponse> {
    let subcmd_owned = subcmd.to_string();
    let payload_owned = payload.to_string();

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(try_daemon_inner(&subcmd_owned, &payload_owned));
    });

    match rx.recv_timeout(IPC_TIMEOUT) {
        Ok(result) => result,
        Err(_) => {
            crate::common::log("ipc", "IPC round-trip timeout (2s) — falling through");
            crate::common::storage::append_diagnostic(
                "ipc_timeout",
                &format!("daemon IPC timeout for '{}'", subcmd),
            );
            None
        }
    }
}

/// Inner implementation of try_daemon — runs on a spawned thread so the caller
/// can enforce a total timeout via channel recv_timeout.
fn try_daemon_inner(subcmd: &str, payload: &str) -> Option<DaemonResponse> {
    let pipe_path = pipe_name();

    let pipe_result = connect_pipe(&pipe_path, Duration::from_millis(50));
    if pipe_result.is_none() {
        // Pipe connection failed — check for stale daemon
        if let Some(pid) = read_pid()
            && !pid_is_alive(pid)
        {
            crate::common::log("ipc", &format!("Stale pidfile (pid={}) — cleaning up", pid));
            remove_pid_file();
            // Check restart storm before auto-restarting
            if !restart_storm_active() {
                record_restart();
                return spawn_and_wait(subcmd, payload);
            } else {
                crate::common::log(
                    "ipc",
                    "Restart storm detected (3+ in 5min) — skipping auto-restart",
                );
            }
        }
        return None;
    }
    let mut pipe = pipe_result.unwrap();

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let request = DaemonRequest {
        subcmd: subcmd.to_string(),
        payload: payload.to_string(),
        binary_mtime: get_binary_mtime(),
        cwd,
        rules_mtime: crate::rules::rules_mtime(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let request_bytes = serde_json::to_vec(&request).ok()?;

    // Write length-prefixed request
    let len = request_bytes.len() as u32;
    pipe.write_all(&len.to_le_bytes()).ok()?;
    pipe.write_all(&request_bytes).ok()?;
    pipe.flush().ok()?;

    // Read length-prefixed response with timeout protection.
    // On Windows, set PIPE_WAIT mode with timeout on the handle so ReadFile
    // doesn't block forever if the daemon is unresponsive.
    #[cfg(windows)]
    pipe.set_read_timeout(Duration::from_millis(2000));

    let mut len_buf = [0u8; 4];
    pipe.read_exact(&mut len_buf).ok()?;
    let resp_len = u32::from_le_bytes(len_buf) as usize;

    if resp_len > 1_048_576 {
        return None;
    }

    let mut resp_buf = vec![0u8; resp_len];
    pipe.read_exact(&mut resp_buf).ok()?;

    serde_json::from_slice(&resp_buf).ok()
}

/// Check if the daemon is running (pipe exists and is connectable)
pub fn daemon_is_running() -> bool {
    let pipe_path = pipe_name();
    connect_pipe(&pipe_path, Duration::from_millis(25)).is_some()
}

/// PID file path for daemon
pub fn pid_file_path() -> std::path::PathBuf {
    crate::common::hooks_dir().join(format!("{}.pid", crate::constants::DAEMON_NAME))
}

/// Write daemon PID + exe path to file for identity validation
pub fn write_pid(pid: u32) {
    let path = pid_file_path();
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let _ = std::fs::write(&path, format!("{}\n{}", pid, exe));
}

/// Read daemon PID from file
pub fn read_pid() -> Option<u32> {
    let path = pid_file_path();
    let content = std::fs::read_to_string(&path).ok()?;
    content.lines().next()?.trim().parse().ok()
}

/// Read the exe path stored alongside the PID (for identity validation)
#[allow(dead_code)]
pub fn read_pid_exe() -> Option<String> {
    let path = pid_file_path();
    let content = std::fs::read_to_string(&path).ok()?;
    content.lines().nth(1).map(|s| s.trim().to_string())
}

/// Remove PID file
pub fn remove_pid_file() {
    let _ = std::fs::remove_file(pid_file_path());
}

/// Check if a PID is alive on Windows
#[cfg(windows)]
pub fn pid_is_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    // SAFETY: OpenProcess returns null on failure (checked); handle closed immediately.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        CloseHandle(handle);
        true
    }
}

/// Validate that the PID belongs to a warden process (guards against PID reuse)
#[cfg(windows)]
pub fn pid_is_warden(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };

    // SAFETY: handle validity checked (null guard); buf is stack-allocated MAX_PATH; handle closed after query.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }

        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);

        if ok == 0 {
            return false;
        }

        let name = String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
        name.contains(crate::constants::NAME)
    }
}

#[cfg(not(windows))]
pub fn pid_is_warden(_pid: u32) -> bool {
    true
}

#[cfg(not(windows))]
pub fn pid_is_alive(pid: u32) -> bool {
    // Check /proc/{pid} on Linux, or use kill -0 via std::process::Command
    std::path::Path::new(&format!("/proc/{}", pid)).exists()
        || std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
}

/// Spawn a new daemon process in the background (detached).
/// Copies the current binary to {DAEMON_NAME}.exe so the original is never locked.
/// Passes source binary mtime as CLI arg for rebuild detection.
pub fn spawn_daemon() {
    let source = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return,
    };

    let source_mtime = get_binary_mtime();

    // Lock-free deployment: include mtime in daemon exe name so a new binary
    // can be deployed without conflicting with the running daemon's open file handle.
    let daemon_exe_name = if cfg!(windows) {
        format!("{}-{}.exe", crate::constants::DAEMON_NAME, source_mtime)
    } else {
        format!("{}-{}", crate::constants::DAEMON_NAME, source_mtime)
    };
    let daemon_exe = crate::common::hooks_dir().join(&daemon_exe_name);

    // Clean up old daemon copies (different mtime) before spawning
    cleanup_old_daemon_copies(source_mtime);

    // Copy current binary to mtime-stamped daemon location
    if !daemon_exe.exists() && std::fs::copy(&source, &daemon_exe).is_err() {
        crate::common::log("ipc", "Cannot copy daemon binary");
        return;
    }

    let mtime_arg = source_mtime.to_string();

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        match std::process::Command::new(&daemon_exe)
            .args(["daemon", &mtime_arg])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()
        {
            Ok(child) => {
                crate::common::log(
                    "ipc",
                    &format!(
                        "Daemon spawned (pid={}, mtime={})",
                        child.id(),
                        source_mtime
                    ),
                );
            }
            Err(e) => {
                crate::common::log("ipc", &format!("Failed to spawn daemon: {}", e));
            }
        }
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid() is async-signal-safe and creates a new session,
        // ensuring the daemon is fully detached from the caller's process group.
        let mut cmd = std::process::Command::new(&daemon_exe);
        cmd.args(["daemon", &mtime_arg])
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
                crate::common::log(
                    "ipc",
                    &format!(
                        "Daemon spawned (pid={}, mtime={})",
                        child.id(),
                        source_mtime
                    ),
                );
            }
            Err(e) => {
                crate::common::log("ipc", &format!("Failed to spawn daemon: {}", e));
            }
        }
    }
}

/// Spawn daemon and wait for it to become available.
/// Returns None if daemon doesn't start within ~500ms.
pub fn spawn_and_wait(subcmd: &str, payload: &str) -> Option<DaemonResponse> {
    spawn_daemon();
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(150));
        if let Some(resp) = try_daemon(subcmd, payload) {
            return Some(resp);
        }
    }
    None
}

/// Remove old daemon binary copies that don't match the current mtime.
/// Best-effort: locked files (running daemon) will fail silently and get cleaned next time.
fn cleanup_old_daemon_copies(current_mtime: u64) {
    let dir = crate::common::hooks_dir();
    let prefix = crate::constants::DAEMON_NAME;
    let current_suffix = current_mtime.to_string();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Match pattern: warden-daemon-{mtime}[.exe]
            if name_str.starts_with(prefix)
                && name_str != format!("{}.pid", prefix)
                && !name_str.contains(&current_suffix)
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// Path to the restart tracking file
fn restart_tracking_path() -> std::path::PathBuf {
    crate::common::hooks_dir().join("daemon-restarts.json")
}

/// Check if 3+ restarts occurred in the last 5 minutes (storm protection)
fn restart_storm_active() -> bool {
    let path = restart_tracking_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let timestamps: Vec<u64> = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff = now.saturating_sub(300); // 5 minutes
    let recent = timestamps.iter().filter(|&&t| t > cutoff).count();
    recent >= 3
}

/// Record a daemon restart timestamp
fn record_restart() {
    let path = restart_tracking_path();
    let mut timestamps: Vec<u64> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Prune entries older than 5 minutes
    let cutoff = now.saturating_sub(300);
    timestamps.retain(|&t| t > cutoff);
    timestamps.push(now);
    if let Ok(json) = serde_json::to_string(&timestamps) {
        let _ = std::fs::write(&path, json);
    }
}

// ─── Graceful daemon shutdown ─────────────────────────────────────────────────

/// Send shutdown signal to daemon and wait for it to exit.
/// Returns true if daemon stopped (or wasn't running), false on timeout.
pub fn stop_daemon_graceful(timeout_ms: u64) -> bool {
    // Try IPC shutdown first
    if let Some(_resp) = try_daemon("shutdown", "{}") {
        // Daemon acknowledged — wait for it to actually exit
        let start = std::time::Instant::now();
        while start.elapsed().as_millis() < timeout_ms as u128 {
            if !daemon_is_running() {
                remove_pid_file();
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    // Fallback: kill by PID if still alive
    if let Some(pid) = read_pid() {
        if pid_is_alive(pid) {
            kill_daemon(pid);
            // Brief wait for process to exit after kill
            let start = std::time::Instant::now();
            while start.elapsed().as_millis() < 1000 {
                if !pid_is_alive(pid) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        remove_pid_file();
    }

    true
}

/// Forcibly terminate a daemon process by PID.
#[cfg(windows)]
fn kill_daemon(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

    // SAFETY: OpenProcess returns null on failure (checked); handle closed after use.
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if !handle.is_null() {
            TerminateProcess(handle, 1);
            CloseHandle(handle);
        }
    }
    crate::common::log("ipc", &format!("Terminated daemon pid={}", pid));
}

/// Forcibly terminate a daemon process by PID.
#[cfg(not(windows))]
fn kill_daemon(pid: u32) {
    // Send SIGTERM for graceful shutdown
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    crate::common::log("ipc", &format!("Sent SIGTERM to daemon pid={}", pid));
}

// ─── Platform-specific pipe connection ───────────────────────────────────────

#[cfg(windows)]
fn connect_pipe(pipe_path: &str, timeout: Duration) -> Option<PipeStream> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Pipes::WaitNamedPipeW;

    let wide_path: Vec<u16> = OsStr::new(pipe_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let timeout_ms = timeout.as_millis() as u32;
    // SAFETY: wide_path is null-terminated; WaitNamedPipeW/CreateFileW return errors checked below.
    unsafe {
        if WaitNamedPipeW(wide_path.as_ptr(), timeout_ms) == 0 {
            return None;
        }

        let handle = CreateFileW(
            wide_path.as_ptr(),
            0x80000000 | 0x40000000, // GENERIC_READ | GENERIC_WRITE
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );

        if handle == INVALID_HANDLE_VALUE {
            return None;
        }

        Some(PipeStream { handle })
    }
}

#[cfg(not(windows))]
fn connect_pipe(pipe_path: &str, timeout: Duration) -> Option<PipeStream> {
    let stream = std::os::unix::net::UnixStream::connect(pipe_path).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    Some(PipeStream(stream))
}

// ─── PipeStream abstraction ─────────────────────────────────────────────────

#[cfg(windows)]
struct PipeStream {
    handle: HANDLE,
}

#[cfg(windows)]
impl PipeStream {
    /// Set a read timeout on the pipe handle using SetNamedPipeHandleState.
    /// Falls back silently if it fails (pipe will block as before).
    fn set_read_timeout(&mut self, timeout: Duration) {
        use windows_sys::Win32::System::Pipes::SetNamedPipeHandleState;
        let mode: u32 = 0x00000000; // PIPE_READMODE_BYTE | PIPE_WAIT
        let timeout_ms = timeout.as_millis() as u32;
        // SAFETY: self.handle is a valid pipe handle from CreateFileW.
        // SetNamedPipeHandleState modifies the pipe mode; null pointers skip unchanged params.
        unsafe {
            SetNamedPipeHandleState(
                self.handle,
                &mode,
                std::ptr::null_mut(),
                &timeout_ms as *const u32 as *mut u32,
            );
        }
    }
}

#[cfg(windows)]
impl Read for PipeStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use windows_sys::Win32::Storage::FileSystem::ReadFile;
        let mut bytes_read: u32 = 0;
        // SAFETY: self.handle is a valid pipe handle from CreateFileW; buf is valid for buf.len() bytes.
        let ok = unsafe {
            ReadFile(
                self.handle,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut bytes_read,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(bytes_read as usize)
        }
    }
}

#[cfg(windows)]
impl Write for PipeStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use windows_sys::Win32::Storage::FileSystem::WriteFile;
        let mut bytes_written: u32 = 0;
        // SAFETY: self.handle is a valid pipe handle; buf is valid for buf.len() bytes.
        let ok = unsafe {
            WriteFile(
                self.handle,
                buf.as_ptr(),
                buf.len() as u32,
                &mut bytes_written,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(bytes_written as usize)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        use windows_sys::Win32::Storage::FileSystem::FlushFileBuffers;
        // SAFETY: self.handle is a valid pipe handle; FlushFileBuffers only requires a valid handle.
        let ok = unsafe { FlushFileBuffers(self.handle) };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(windows)]
impl Drop for PipeStream {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        // SAFETY: self.handle is valid for the lifetime of PipeStream; Drop runs exactly once.
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

#[cfg(not(windows))]
struct PipeStream(std::os::unix::net::UnixStream);

#[cfg(not(windows))]
impl Read for PipeStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

#[cfg(not(windows))]
impl Write for PipeStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
