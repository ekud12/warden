// ─── warden-relay — IPC client for warden server ─────────────────────────────
//
// v2.4: Relay connects directly to the persistent warden server via named pipe.
// No longer spawns warden.exe per hook call — just IPC (~7ms total).
//
// Flow:
//   1. Read stdin from Claude Code
//   2. Connect to warden server via named pipe
//   3. Send hook request, receive response
//   4. Print response to stdout, exit
//
// If server not running: spawn warden.exe __server, wait, retry.
// If all else fails: spawn warden.exe directly (cold fallback).
// ──────────────────────────────────────────────────────────────────────────────

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let subcmd = args.first().map(|s| s.as_str()).unwrap_or("");

    let stdin_data = read_stdin();

    let relay_path = std::env::current_exe().unwrap_or_default();
    let bin_dir = relay_path.parent().unwrap_or(std::path::Path::new("."));
    let warden = bin_dir.join(if cfg!(windows) {
        "warden.exe"
    } else {
        "warden"
    });

    if !warden.exists() {
        std::process::exit(0);
    }

    // Try 1: connect to running server
    if let Some((stdout, code)) = try_server(subcmd, &stdin_data) {
        print!("{}", stdout);
        std::process::exit(code);
    }

    // Try 2: spawn server, wait, retry
    spawn_server(&warden);
    std::thread::sleep(Duration::from_millis(200));
    if let Some((stdout, code)) = try_server(subcmd, &stdin_data) {
        print!("{}", stdout);
        std::process::exit(code);
    }

    // Try 3: one more retry after extra wait
    std::thread::sleep(Duration::from_millis(200));
    if let Some((stdout, code)) = try_server(subcmd, &stdin_data) {
        print!("{}", stdout);
        std::process::exit(code);
    }

    // Try 4: cold fallback — run warden.exe directly
    exec_direct(&warden, &args, &stdin_data);
}

fn read_stdin() -> String {
    let mut buf = vec![0u8; 1_048_576];
    let n = std::io::stdin().read(&mut buf).unwrap_or(0);
    String::from_utf8_lossy(&buf[..n]).to_string()
}

fn try_server(subcmd: &str, payload: &str) -> Option<(String, i32)> {
    let pipe_path = pipe_name();
    let mut pipe = connect_pipe(&pipe_path)?;

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let request = serde_json::json!({
        "subcmd": subcmd,
        "payload": payload,
        "binary_mtime": get_mtime(),
        "cwd": cwd,
        "rules_mtime": 0u64,
        "version": env!("CARGO_PKG_VERSION"),
    });
    let request_bytes = serde_json::to_vec(&request).ok()?;

    // Write length-prefixed request
    let len = request_bytes.len() as u32;
    pipe.write_all(&len.to_le_bytes()).ok()?;
    pipe.write_all(&request_bytes).ok()?;
    pipe.flush().ok()?;

    // Read length-prefixed response
    let mut len_buf = [0u8; 4];
    pipe.read_exact(&mut len_buf).ok()?;
    let resp_len = u32::from_le_bytes(len_buf) as usize;
    if resp_len > 1_048_576 {
        return None;
    }
    let mut resp_buf = vec![0u8; resp_len];
    pipe.read_exact(&mut resp_buf).ok()?;

    let resp: serde_json::Value = serde_json::from_slice(&resp_buf).ok()?;
    let stdout = resp.get("stdout")?.as_str()?.to_string();
    let code = resp.get("exit_code")?.as_i64()? as i32;
    if code == -2 {
        return None; // EXIT_RESTART
    }
    Some((stdout, code))
}

fn spawn_server(warden: &std::path::Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;
        let _ = Command::new(warden)
            .args(["__server"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt;
        let mut cmd = Command::new(warden);
        cmd.args(["__server"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let _ = cmd.spawn();
    }
}

fn exec_direct(warden: &std::path::Path, args: &[String], stdin_data: &str) {
    let mut cmd = Command::new(warden);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.spawn() {
        Ok(mut child) => {
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(stdin_data.as_bytes());
            }
            if let Ok(output) = child.wait_with_output() {
                print!("{}", String::from_utf8_lossy(&output.stdout));
                std::process::exit(output.status.code().unwrap_or(0));
            }
        }
        Err(_) => std::process::exit(0),
    }
}

fn get_mtime() -> u64 {
    std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn pipe_name() -> String {
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\warden-{}", username)
    }
    #[cfg(not(windows))]
    {
        format!("/tmp/warden-{}.sock", username)
    }
}

// ─── Platform pipe connection (uses windows-sys for correctness) ─────────────

#[cfg(windows)]
fn connect_pipe(pipe_path: &str) -> Option<WinPipe> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING,
    };

    let wide: Vec<u16> = OsStr::new(pipe_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        // Try direct connect first (faster than WaitNamedPipeW if pipe is available)
        let handle = CreateFileW(
            wide.as_ptr(),
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
        Some(WinPipe(handle))
    }
}

#[cfg(windows)]
struct WinPipe(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Read for WinPipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use windows_sys::Win32::Storage::FileSystem::ReadFile;
        let mut n: u32 = 0;
        let ok = unsafe {
            ReadFile(
                self.0,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut n,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }
}

#[cfg(windows)]
impl Write for WinPipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use windows_sys::Win32::Storage::FileSystem::WriteFile;
        let mut n: u32 = 0;
        let ok = unsafe {
            WriteFile(
                self.0,
                buf.as_ptr(),
                buf.len() as u32,
                &mut n,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        use windows_sys::Win32::Storage::FileSystem::FlushFileBuffers;
        let ok = unsafe { FlushFileBuffers(self.0) };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(windows)]
impl Drop for WinPipe {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(not(windows))]
fn connect_pipe(pipe_path: &str) -> Option<UnixPipe> {
    let stream = std::os::unix::net::UnixStream::connect(pipe_path).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(2000)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(2000)))
        .ok()?;
    Some(UnixPipe(stream))
}

#[cfg(not(windows))]
struct UnixPipe(std::os::unix::net::UnixStream);

#[cfg(not(windows))]
impl Read for UnixPipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

#[cfg(not(windows))]
impl Write for UnixPipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
