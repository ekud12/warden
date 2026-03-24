// ─── proc_mgmt — process management commands ────────────────────────────────
//
// Provides: warden proc start|stop|restart|status|wait|logs
// Manages dev server processes with health checking and readiness gating.
//
// With daemon: processes are spawned by daemon, health monitored in background.
// Without daemon: processes spawned directly, health checked on demand.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Per-process state
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub cmd: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub health_url: Option<String>,
    #[serde(default)]
    pub ready_pattern: Option<String>,
    #[serde(default)]
    pub health: String,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub last_check: String,
}

/// All managed processes
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ProcState {
    #[serde(default)]
    pub processes: HashMap<String, ProcessInfo>,
}

/// Path to proc-state.json
fn proc_state_path() -> PathBuf {
    common::hooks_dir().join("proc-state.json")
}

/// Read proc state — returns defaults on any error
fn read_proc_state() -> ProcState {
    let path = proc_state_path();
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => ProcState::default(),
    }
}

/// Write proc state atomically
fn write_proc_state(state: &ProcState) {
    let path = proc_state_path();
    let tmp_path = path.with_extension("json.tmp");
    if let Ok(json) = serde_json::to_string_pretty(state)
        && fs::write(&tmp_path, &json).is_ok()
            && fs::rename(&tmp_path, &path).is_err()
        {
            let _ = fs::write(&path, &json);
            let _ = fs::remove_file(&tmp_path);
        }
}

/// Proc logs directory
fn proc_logs_dir() -> PathBuf {
    common::hooks_dir().join("proc-logs")
}

/// Route proc subcommands
pub fn run(args: &[String]) {
    let subcmd = args.first().map(|s| s.as_str()).unwrap_or("");

    match subcmd {
        "start" => cmd_start(args),
        "stop" => cmd_stop(args),
        "restart" => cmd_restart(args),
        "status" => cmd_status(),
        "wait" => cmd_wait(args),
        "logs" => cmd_logs(args),
        _ => {
            eprintln!("Usage: {} proc <start|stop|restart|status|wait|logs>", crate::constants::NAME);
            std::process::exit(1);
        }
    }
}

/// Parse a named argument from args: --name value
fn get_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn cmd_start(args: &[String]) {
    let name = match get_arg(args, "--name") {
        Some(n) => n,
        None => {
            eprintln!("--name required");
            std::process::exit(1);
        }
    };

    let cmd = match get_arg(args, "--cmd") {
        Some(c) => c,
        None => {
            eprintln!("--cmd required");
            std::process::exit(1);
        }
    };

    let port: Option<u16> = get_arg(args, "--port").and_then(|p| p.parse().ok());
    let health_url = get_arg(args, "--health-url");
    let ready_pattern = get_arg(args, "--ready-pattern");

    // Ensure proc-logs directory exists
    let logs_dir = proc_logs_dir();
    let _ = fs::create_dir_all(&logs_dir);

    let log_path = logs_dir.join(format!("{}.log", name));
    let log_file = match fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Cannot create log file: {}", e);
            std::process::exit(1);
        }
    };

    // Stop existing process with same name if running
    let mut state = read_proc_state();
    if let Some(existing) = state.processes.get(&name) {
        let _ = kill_process(existing.pid);
    }

    // Spawn process
    let child = match spawn_process(&cmd, log_file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to start '{}': {}", cmd, e);
            std::process::exit(1);
        }
    };

    let pid = child.id();
    let info = ProcessInfo {
        pid,
        cmd: cmd.clone(),
        port,
        health_url,
        ready_pattern,
        health: "starting".to_string(),
        started_at: common::now_iso(),
        last_check: String::new(),
    };

    state.processes.insert(name.clone(), info);
    write_proc_state(&state);

    common::log("proc", &format!("START {} (pid={}, cmd={})", name, pid, common::truncate(&cmd, 60)));
    println!("Started '{}' (pid={})", name, pid);
}

fn cmd_stop(args: &[String]) {
    let name = match get_arg(args, "--name") {
        Some(n) => n,
        None => {
            eprintln!("--name required");
            std::process::exit(1);
        }
    };

    let mut state = read_proc_state();
    if let Some(info) = state.processes.remove(&name) {
        let _ = kill_process(info.pid);
        write_proc_state(&state);
        common::log("proc", &format!("STOP {} (pid={})", name, info.pid));
        println!("Stopped '{}'", name);
    } else {
        eprintln!("No process named '{}'", name);
        std::process::exit(1);
    }
}

fn cmd_restart(args: &[String]) {
    cmd_stop(args);
    cmd_start(args);
}

fn cmd_status() {
    let mut state = read_proc_state();

    // Update health for all processes
    let names: Vec<String> = state.processes.keys().cloned().collect();
    for name in &names {
        if let Some(info) = state.processes.get_mut(name) {
            info.health = check_health(info);
            info.last_check = common::now_iso();
        }
    }
    write_proc_state(&state);

    if state.processes.is_empty() {
        println!("No managed processes");
        return;
    }

    for (name, info) in &state.processes {
        let port_str = info
            .port
            .map(|p| format!(":{}", p))
            .unwrap_or_default();
        println!(
            "{:<15} pid={:<8} health={:<10} {}{}",
            name, info.pid, info.health, info.cmd, port_str
        );
    }
}

fn cmd_wait(args: &[String]) {
    let name = match get_arg(args, "--name") {
        Some(n) => n,
        None => {
            eprintln!("--name required");
            std::process::exit(1);
        }
    };

    let timeout_secs: u64 = get_arg(args, "--timeout")
        .and_then(|t| t.parse().ok())
        .unwrap_or(30);

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    loop {
        let mut state = read_proc_state();
        let (health, pid) = if let Some(info) = state.processes.get_mut(&name) {
            info.health = check_health(info);
            info.last_check = common::now_iso();
            (info.health.clone(), info.pid)
        } else {
            eprintln!("No process named '{}'", name);
            std::process::exit(1);
        };
        write_proc_state(&state);

        if health == "healthy" {
            println!("'{}' is healthy", name);
            return;
        }

        if !pid_is_alive(pid) {
            eprintln!("'{}' (pid={}) has exited", name, pid);
            std::process::exit(1);
        }

        if start.elapsed() > timeout {
            eprintln!("Timeout waiting for '{}'", name);
            std::process::exit(1);
        }

        std::thread::sleep(Duration::from_millis(500));
    }
}

fn cmd_logs(args: &[String]) {
    let name = match get_arg(args, "--name") {
        Some(n) => n,
        None => {
            eprintln!("--name required");
            std::process::exit(1);
        }
    };

    let tail: usize = get_arg(args, "--tail")
        .and_then(|t| t.parse().ok())
        .unwrap_or(50);

    let log_path = proc_logs_dir().join(format!("{}.log", name));
    if !log_path.exists() {
        eprintln!("No logs for '{}'", name);
        std::process::exit(1);
    }

    // Read last N lines
    let content = common::read_tail(&log_path, 65536);
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(tail);
    for line in &lines[start..] {
        println!("{}", line);
    }
}

/// Check health of a process
fn check_health(info: &ProcessInfo) -> String {
    // Check if process is alive first
    if !pid_is_alive(info.pid) {
        return "failed".to_string();
    }

    // Health URL check
    if let Some(ref url) = info.health_url {
        return if check_http_health(url) {
            "healthy".to_string()
        } else {
            "starting".to_string()
        };
    }

    // Port check
    if let Some(port) = info.port {
        return if check_port(port) {
            "healthy".to_string()
        } else {
            "starting".to_string()
        };
    }

    // No health check configured — if alive, consider healthy
    "healthy".to_string()
}

/// Check if a TCP port is open
fn check_port(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().expect("valid loopback addr"),
        Duration::from_millis(200),
    )
    .is_ok()
}

/// Check HTTP health endpoint (basic — just check if connection succeeds)
fn check_http_health(url: &str) -> bool {
    // Extract host:port from URL
    let stripped = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    let host_port = stripped.split('/').next().unwrap_or(stripped);

    TcpStream::connect_timeout(
        &host_port.parse().unwrap_or_else(|_| "127.0.0.1:80".parse().expect("valid fallback addr")),
        Duration::from_millis(200),
    )
    .is_ok()
}

/// Spawn a process with stdout/stderr redirected to log file
fn spawn_process(cmd: &str, log_file: fs::File) -> Result<std::process::Child, std::io::Error> {
    let log_stderr = log_file.try_clone()?;

    // Use shell to interpret the command string
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", cmd])
            .stdout(log_file)
            .stderr(log_stderr)
            .stdin(std::process::Stdio::null())
            .spawn()
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new("sh")
            .args(["-c", cmd])
            .stdout(log_file)
            .stderr(log_stderr)
            .stdin(std::process::Stdio::null())
            .spawn()
    }
}

/// Kill a process by PID
fn kill_process(pid: u32) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Check if a process is alive by PID
fn pid_is_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .map(|o| {
                let out = String::from_utf8_lossy(&o.stdout);
                out.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    {
        // kill -0 checks if process exists without sending a signal
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Kill all managed processes (called on session-end)
pub fn kill_all() {
    let mut state = read_proc_state();
    for (name, info) in &state.processes {
        common::log("proc", &format!("KILL {} (pid={})", name, info.pid));
        let _ = kill_process(info.pid);
    }
    state.processes.clear();
    write_proc_state(&state);
}

/// Get proc state for health gate checks
pub fn get_process_on_port(port: u16) -> Option<(String, String)> {
    let state = read_proc_state();
    for (name, info) in &state.processes {
        if info.port == Some(port) {
            return Some((name.clone(), info.health.clone()));
        }
    }
    None
}
