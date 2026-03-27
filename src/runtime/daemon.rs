// ─── daemon — background pipe server for warden ───────────────────────────────
//
// Transparent daemon that compiles regexes once, holds session state in memory,
// and responds to hook requests via named pipe IPC.
//
// Lifecycle:
//   - Auto-started on session-start (if pipe not connectable)
//   - Persists across sessions (like Docker Desktop)
//   - Auto-stops after 1 hour idle (re-spawned on next session-start)
//   - Auto-restarts on binary rebuild (mtime mismatch detection)
//   - Falls back to CLI mode if daemon crashes
//
// Named pipe: \\.\pipe\{PIPE_PREFIX}-{username}
// Protocol: length-prefixed JSON request → length-prefixed JSON response
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::runtime::ipc::{self, DaemonRequest, DaemonResponse};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;

/// Timeout for reading a request after client connects (prevents indefinite block)
const READ_TIMEOUT_SECS: u64 = 30;

/// Idle timeout: auto-shutdown after 1 hour with no requests
const IDLE_TIMEOUT_SECS: u64 = 3600;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Run the daemon server loop.
/// `source_mtime` is the mtime of the original binary at the time
/// this daemon copy was spawned. Used to detect rebuilds.
pub fn run_server(source_mtime: u64) {
    let pid = std::process::id();
    ipc::write_pid(pid);

    let startup_mtime = source_mtime;
    let startup_rules_mtime = crate::rules::rules_mtime();
    let startup_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Enable in-memory caching for session state and log buffering
    common::enable_daemon_mode();

    // Initialize platform-specific listener
    #[cfg(not(windows))]
    if !init_unix_listener() {
        common::log("daemon", "Failed to initialize Unix listener — aborting");
        return;
    }

    common::log(
        "daemon",
        &format!("Starting daemon (pid={}, mtime={})", pid, startup_mtime),
    );

    let shutdown = Arc::new(AtomicBool::new(false));
    let last_activity = Arc::new(AtomicU64::new(now_secs()));

    // Idle timeout watchdog — shuts down daemon after 1 hour of no requests
    {
        let last_act = Arc::clone(&last_activity);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(60));
                let idle_secs = now_secs().saturating_sub(last_act.load(Ordering::Relaxed));
                if idle_secs >= IDLE_TIMEOUT_SECS {
                    common::log("daemon", "Idle timeout (1h) — auto-shutdown");
                    ipc::remove_pid_file();
                    #[cfg(not(windows))]
                    {
                        let _ = std::fs::remove_file(ipc::pipe_name());
                    }
                    std::process::exit(0);
                }
            }
        });
    }

    // Dream worker — background learning during idle time
    {
        let last_act = Arc::clone(&last_activity);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(30));
                let idle_secs = now_secs().saturating_sub(last_act.load(Ordering::Relaxed));
                if idle_secs < 10 {
                    continue;
                } // Only dream when genuinely idle

                if let Some(batch) = crate::engines::dream::next_batch() {
                    crate::engines::dream::process_batch(batch);
                }
            }
        });
    }

    // Main server loop
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        match accept_connection() {
            Some(mut pipe) => {
                last_activity.store(now_secs(), Ordering::Relaxed);

                let request = match read_request_with_timeout(&mut pipe, READ_TIMEOUT_SECS) {
                    Some(r) => r,
                    None => continue,
                };

                // Handle shutdown
                if request.subcmd == "shutdown" {
                    common::log("daemon", "Shutdown requested");
                    shutdown.store(true, Ordering::Relaxed);
                    let response = DaemonResponse {
                        stdout: String::new(),
                        exit_code: 0,
                    };
                    let _ = write_response(&mut pipe, &response);
                    break;
                }

                // Handle status query
                if request.subcmd == "daemon-status" {
                    let response = DaemonResponse {
                        stdout: format!(
                            "{{\"pid\":{},\"mtime\":{},\"version\":\"{}\",\"started_at\":{}}}",
                            pid,
                            startup_mtime,
                            env!("CARGO_PKG_VERSION"),
                            startup_time
                        ),
                        exit_code: 0,
                    };
                    let _ = write_response(&mut pipe, &response);
                    continue;
                }

                // Version mismatch detection: client version differs from daemon version
                if !request.version.is_empty() && request.version != env!("CARGO_PKG_VERSION") {
                    common::log(
                        "daemon",
                        &format!(
                            "Version mismatch (daemon={}, client={}), shutting down",
                            env!("CARGO_PKG_VERSION"),
                            request.version
                        ),
                    );
                    let response = DaemonResponse {
                        stdout: String::new(),
                        exit_code: ipc::EXIT_RESTART,
                    };
                    let _ = write_response(&mut pipe, &response);
                    shutdown.store(true, Ordering::Relaxed);
                    break;
                }

                // Binary rebuild detection: client mtime differs from our startup mtime
                if request.binary_mtime != 0
                    && startup_mtime != 0
                    && request.binary_mtime != startup_mtime
                {
                    common::log(
                        "daemon",
                        &format!(
                            "Binary rebuild detected (daemon={}, client={}), shutting down",
                            startup_mtime, request.binary_mtime
                        ),
                    );
                    // Tell client to restart — don't process this request
                    let response = DaemonResponse {
                        stdout: String::new(),
                        exit_code: ipc::EXIT_RESTART,
                    };
                    let _ = write_response(&mut pipe, &response);
                    // Shut down so new daemon can start
                    shutdown.store(true, Ordering::Relaxed);
                    break;
                }

                // Rules.toml change detection: restart daemon to reload merged rules
                // Also restart when a rules file appears (startup_rules_mtime was 0)
                if request.rules_mtime != startup_rules_mtime
                    && (request.rules_mtime != 0 || startup_rules_mtime != 0)
                {
                    common::log(
                        "daemon",
                        &format!(
                            "Rules.toml changed (daemon={}, client={}), shutting down",
                            startup_rules_mtime, request.rules_mtime
                        ),
                    );
                    let response = DaemonResponse {
                        stdout: String::new(),
                        exit_code: ipc::EXIT_RESTART,
                    };
                    let _ = write_response(&mut pipe, &response);
                    shutdown.store(true, Ordering::Relaxed);
                    break;
                }

                // Set per-project CWD before dispatch (thread-local for isolation)
                if !request.cwd.is_empty() {
                    common::set_project_cwd(&request.cwd);
                }

                // Dispatch to handler — run in-process with stdout capture
                let response = dispatch_handler(&request.subcmd, &request.payload);
                let _ = write_response(&mut pipe, &response);

                // Flush buffered logs and debounced session state after each request
                common::flush_daemon_buffers();
            }
            None => {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    // Cleanup — flush any remaining buffered state
    common::log("daemon", "Daemon shutting down");
    common::flush_daemon_buffers();
    ipc::remove_pid_file();

    // Remove Unix domain socket file on shutdown
    #[cfg(not(windows))]
    {
        let sock_path = ipc::pipe_name();
        let _ = std::fs::remove_file(&sock_path);
    }

    common::log("daemon", "Daemon stopped");
    common::flush_daemon_buffers();
}

/// Dispatch a hook subcmd to its handler in-process with stdout capture
fn dispatch_handler(subcmd: &str, payload: &str) -> DaemonResponse {
    let handler_start = Instant::now();
    common::start_capture();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match subcmd {
        "pretool-bash" => crate::handlers::pretool_bash::run(payload),
        "pretool-read" => crate::handlers::pretool_read::run(payload),
        "pretool-write" => crate::handlers::pretool_write::run(payload),
        "pretool-redirect" => crate::handlers::pretool_redirect::run(payload),
        "permission-approve" => crate::handlers::permission_approve::run(payload),
        "posttool-session" => crate::engines::anchor::ledger::run(payload),
        "posttool-mcp" => crate::handlers::posttool_mcp::run(payload),
        "session-start" => crate::engines::anchor::session_start::run(payload),
        "session-end" => crate::engines::anchor::session_end::run(payload),
        "precompact-memory" => crate::engines::anchor::precompact::run(payload),
        "postcompact" => crate::engines::anchor::postcompact::run(payload),
        "stop-check" => crate::handlers::stop_check::run(payload),
        "userprompt-context" => crate::handlers::userprompt_context::run(payload),
        "subagent-context" => crate::handlers::subagent_context::run(payload),
        "subagent-stop" => crate::handlers::subagent_stop::run(payload),
        "postfailure-guide" => crate::handlers::postfailure_guide::run(payload),
        "task-completed" => crate::handlers::task_completed::run(payload),
        _ => {}
    }));

    let stdout = common::take_capture();
    let elapsed = handler_start.elapsed();
    let elapsed_us = elapsed.as_micros();
    // Structured handler timing for latency tracking and regression detection
    common::log_structured(
        "daemon",
        common::LogLevel::Info,
        "handler-timing",
        &format!("{}={}us ({}ms)", subcmd, elapsed_us, elapsed.as_millis()),
    );
    // Warn if handler exceeds budget (pretool: 2ms, session-start: 500ms, others: 10ms)
    // session-start gets a higher budget — cold start includes redb init + file I/O
    let budget_us = if subcmd.starts_with("pretool") {
        2000
    } else if subcmd == "session-start" {
        500_000
    } else {
        10000
    };
    if elapsed_us > budget_us {
        common::log_structured(
            "daemon",
            common::LogLevel::Info,
            "latency-warning",
            &format!(
                "{} exceeded budget: {}us > {}us",
                subcmd, elapsed_us, budget_us
            ),
        );
    }
    DaemonResponse {
        stdout,
        exit_code: if result.is_ok() { 0 } else { 1 },
    }
}

// ─── Platform-specific pipe server ───────────────────────────────────────────

#[cfg(windows)]
struct ServerPipe {
    handle: HANDLE,
}

#[cfg(windows)]
fn accept_connection() -> Option<ServerPipe> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Security::{
        InitializeSecurityDescriptor, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
        SetSecurityDescriptorDacl,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
    };

    let pipe_path = ipc::pipe_name();
    let wide_path: Vec<u16> = OsStr::new(&pipe_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Build a security descriptor with a null DACL restricted to current user.
    // A null DACL grants full access only to the owner (the current process user).
    // SAFETY: sd is stack-allocated and lives for the duration of CreateNamedPipeW.
    let mut sd: SECURITY_DESCRIPTOR = unsafe { std::mem::zeroed() };
    unsafe {
        // SECURITY_DESCRIPTOR_REVISION = 1
        InitializeSecurityDescriptor(&mut sd as *mut _ as *mut _, 1);
        // Setting bDaclPresent=true with pDacl=null creates an empty DACL (deny all except owner)
        SetSecurityDescriptorDacl(&mut sd as *mut _ as *mut _, 1, std::ptr::null(), 0);
    }

    let mut sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: &mut sd as *mut _ as *mut _,
        bInheritHandle: 0,
    };

    // SAFETY: wide_path is null-terminated; sa is valid for pipe lifetime; handle checked below.
    unsafe {
        let handle = CreateNamedPipeW(
            wide_path.as_ptr(),
            0x00000003, // PIPE_ACCESS_DUPLEX
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            10,   // max instances
            4096, // out buffer
            4096, // in buffer
            100,  // default timeout ms
            &mut sa as *mut _ as *const _,
        );

        if handle == INVALID_HANDLE_VALUE {
            return None;
        }

        let connected = ConnectNamedPipe(handle, std::ptr::null_mut());
        if connected == 0 {
            let err = windows_sys::Win32::Foundation::GetLastError();
            // ERROR_PIPE_CONNECTED = 535
            if err != 535 {
                windows_sys::Win32::Foundation::CloseHandle(handle);
                return None;
            }
        }

        Some(ServerPipe { handle })
    }
}

#[cfg(windows)]
impl Read for ServerPipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use windows_sys::Win32::Storage::FileSystem::ReadFile;
        let mut bytes_read: u32 = 0;
        // SAFETY: self.handle is a valid pipe from CreateNamedPipeW; buf is valid for buf.len() bytes.
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
impl Write for ServerPipe {
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
impl Drop for ServerPipe {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Pipes::DisconnectNamedPipe;
        // SAFETY: self.handle is valid for the lifetime of ServerPipe; Drop runs exactly once.
        unsafe {
            DisconnectNamedPipe(self.handle);
            CloseHandle(self.handle);
        }
    }
}

#[cfg(not(windows))]
struct ServerPipe(std::os::unix::net::UnixStream);

#[cfg(not(windows))]
static UNIX_LISTENER: std::sync::LazyLock<
    std::sync::Mutex<Option<std::os::unix::net::UnixListener>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Initialize the Unix domain socket listener. Called once at daemon startup.
#[cfg(not(windows))]
pub fn init_unix_listener() -> bool {
    let path = ipc::pipe_name();
    // Remove stale socket file if it exists
    let _ = std::fs::remove_file(&path);
    match std::os::unix::net::UnixListener::bind(&path) {
        Ok(listener) => {
            // Set socket file permissions to owner-only (0o600)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            if let Ok(mut guard) = UNIX_LISTENER.lock() {
                *guard = Some(listener);
            }
            true
        }
        Err(e) => {
            common::log("daemon", &format!("Failed to bind Unix socket: {}", e));
            false
        }
    }
}

#[cfg(not(windows))]
fn accept_connection() -> Option<ServerPipe> {
    let guard = UNIX_LISTENER.lock().ok()?;
    let listener = guard.as_ref()?;
    // Set a timeout so the accept doesn't block forever (allows idle checks)
    listener.set_nonblocking(false).ok()?;
    match listener.accept() {
        Ok((stream, _)) => {
            stream
                .set_read_timeout(Some(Duration::from_secs(READ_TIMEOUT_SECS)))
                .ok()?;
            stream
                .set_write_timeout(Some(Duration::from_secs(READ_TIMEOUT_SECS)))
                .ok()?;
            Some(ServerPipe(stream))
        }
        Err(_) => None,
    }
}

#[cfg(not(windows))]
impl Read for ServerPipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

#[cfg(not(windows))]
impl Write for ServerPipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

// ─── Protocol helpers ────────────────────────────────────────────────────────

/// Read a request with a timeout to prevent indefinite blocking.
/// Uses PeekNamedPipe on Windows to poll for data availability.
fn read_request_with_timeout(pipe: &mut ServerPipe, timeout_secs: u64) -> Option<DaemonRequest> {
    #[cfg(windows)]
    {
        wait_for_data(pipe, Duration::from_secs(timeout_secs))?;
    }
    #[cfg(not(windows))]
    {
        let _ = timeout_secs;
    }
    read_request(pipe)
}

/// Wait until data is available on the pipe, or timeout.
/// Returns Some(()) if data available, None on timeout.
#[cfg(windows)]
fn wait_for_data(pipe: &ServerPipe, timeout: Duration) -> Option<()> {
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;

    let deadline = Instant::now() + timeout;
    loop {
        let mut available: u32 = 0;
        // SAFETY: pipe.handle is a valid server pipe; passing null buffers with size 0 is valid for peek.
        let ok = unsafe {
            PeekNamedPipe(
                pipe.handle,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        if ok != 0 && available > 0 {
            return Some(());
        }
        if ok == 0 {
            // Pipe error (client disconnected)
            return None;
        }
        if Instant::now() >= deadline {
            common::log("daemon", "Read timeout — client didn't send data");
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn read_request<R: Read>(pipe: &mut R) -> Option<DaemonRequest> {
    let mut len_buf = [0u8; 4];
    pipe.read_exact(&mut len_buf).ok()?;
    let len = u32::from_le_bytes(len_buf) as usize;

    if len > 1_048_576 {
        return None;
    }

    let mut buf = vec![0u8; len];
    pipe.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn write_response<W: Write>(pipe: &mut W, response: &DaemonResponse) -> Option<()> {
    let bytes = serde_json::to_vec(response).ok()?;
    let len = bytes.len() as u32;
    pipe.write_all(&len.to_le_bytes()).ok()?;
    pipe.write_all(&bytes).ok()?;
    pipe.flush().ok()?;
    Some(())
}
