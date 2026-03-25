// ─── common::io — file I/O, logging, and stdin reading ───────────────────────

use super::types::HookInput;
use super::util::now_iso;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

static HOOKS_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    // Resolution chain: WARDEN_HOME env → ~/.warden/ → ~/.claude/hooks/ (fallback)
    if let Ok(dir) = std::env::var("WARDEN_HOME") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let preferred = PathBuf::from(&home).join(crate::constants::DIR);
    if preferred.exists() {
        return preferred;
    }
    PathBuf::from(home).join(".claude").join("hooks")
});

// ─── CI/CD detection ─────────────────────────────────────────────────────────

/// Check if running in a CI/CD environment. When true, Warden runs in minimal
/// mode: safety rules only, no analytics, no session state writes.
pub fn is_ci() -> bool {
    std::env::var("CI").is_ok()
        || std::env::var("GITHUB_ACTIONS").is_ok()
        || std::env::var("GITLAB_CI").is_ok()
        || std::env::var("JENKINS_URL").is_ok()
        || std::env::var("CIRCLECI").is_ok()
        || std::env::var("TRAVIS").is_ok()
        || std::env::var("TF_BUILD").is_ok()
        || std::env::var("BUILDKITE").is_ok()
        || std::env::var("CODEBUILD_BUILD_ID").is_ok()
        || std::env::var("WARDEN_CI").is_ok()
}

// ─── Per-project CWD isolation ──────────────────────────────────────────────

thread_local! {
    static PROJECT_CWD: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Dirs already created (avoids repeated create_dir_all)
static CREATED_DIRS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Set the project CWD for this thread (daemon: from request, direct: from env).
/// Normalizes to git root so all subdirs of a repo map to the same project.
pub fn set_project_cwd(cwd: &str) {
    let root = find_git_root(cwd);
    PROJECT_CWD.with(|c| *c.borrow_mut() = root);
}

/// Get the project CWD for this thread (normalized to git root)
pub fn get_project_cwd() -> String {
    PROJECT_CWD.with(|c| c.borrow().clone())
}

/// Walk up from `cwd` to find `.git` directory, return that root.
/// Falls back to `cwd` itself if no git root found.
/// Always returns normalized path (forward slashes, lowercase drive).
fn find_git_root(cwd: &str) -> String {
    let normalized = normalize_cwd(cwd);
    let mut dir = PathBuf::from(&normalized);
    loop {
        if dir.join(".git").exists() {
            return normalize_cwd(&dir.to_string_lossy());
        }
        if !dir.pop() {
            break;
        }
    }
    normalized
}

/// Compute the hash8 key from a CWD string.
/// Normalizes path separators and drive letter case so the same project
/// always maps to the same hash regardless of invocation context
/// (Windows backslash vs MSYS forward slash).
pub fn cwd_hash8(cwd: &str) -> String {
    use std::hash::{Hash, Hasher};
    let normalized = normalize_cwd(cwd);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    normalized.hash(&mut hasher);
    format!("{:016x}", hasher.finish())[..8].to_string()
}

/// Normalize CWD for consistent hashing:
///   - Convert backslashes to forward slashes
///   - Lowercase drive letter (C: → c:)
///   - Convert MSYS paths (/c/... → c:/...)
///   - Strip trailing slash
fn normalize_cwd(cwd: &str) -> String {
    let mut s = cwd.replace('\\', "/");
    // MSYS path: /c/Projects/... → c:/Projects/...
    if s.len() >= 3 && s.starts_with('/') && s.as_bytes()[2] == b'/' {
        let drive = s.as_bytes()[1].to_ascii_lowercase() as char;
        s = format!("{}:/{}", drive, &s[3..]);
    }
    // Lowercase drive letter: C:/... → c:/...
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        // SAFETY: drive letter is ASCII, so replacing first byte is valid UTF-8
        let mut bytes = s.into_bytes();
        bytes[0] = bytes[0].to_ascii_lowercase();
        s = String::from_utf8(bytes).unwrap_or_default();
    }
    // Strip trailing slash
    s.trim_end_matches('/').to_string()
}

/// Per-project directory: `~/.warden/projects/{hash8}/`
/// Falls back to `hooks_dir()` when CWD is empty (backward compat).
pub fn project_dir() -> PathBuf {
    let cwd = get_project_cwd();
    if cwd.is_empty() {
        return hooks_dir().to_path_buf();
    }

    let hash8 = cwd_hash8(&cwd);
    let dir = hooks_dir().join("projects").join(&hash8);

    // Lazy-create dir + breadcrumb (deduped by hash8)
    let mut created = CREATED_DIRS.lock().unwrap_or_else(|e| e.into_inner());
    if !created.contains(&hash8) {
        let _ = fs::create_dir_all(&dir);
        let breadcrumb = dir.join("project.txt");
        if !breadcrumb.exists() {
            let _ = fs::write(&breadcrumb, &cwd);
        }
        created.insert(hash8);
    }

    dir
}

static LOG_DIR_CREATED: AtomicBool = AtomicBool::new(false);

/// Whether log writes are buffered (daemon mode)
static LOG_BUFFERED: AtomicBool = AtomicBool::new(false);

/// Buffered log entries: hook_name → Vec<formatted_lines>
static LOG_BUFFER: LazyLock<Mutex<HashMap<String, Vec<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Read all stdin into a string (capped at 1 MiB to prevent memory spikes)
pub fn read_stdin() -> String {
    const MAX_STDIN: u64 = 1_048_576; // 1 MiB
    let mut buf = String::new();
    io::stdin()
        .take(MAX_STDIN)
        .read_to_string(&mut buf)
        .unwrap_or(0);
    buf
}

/// Parse stdin JSON into HookInput
pub fn parse_input(raw: &str) -> Option<HookInput> {
    if raw.is_empty() || !raw.starts_with('{') {
        return None;
    }
    serde_json::from_str(raw).ok()
}

/// Resolve ~/.warden/ directory (cached — env var lookup happens once)
pub fn hooks_dir() -> &'static Path {
    &HOOKS_DIR
}

/// Resolve the active assistant's rules directory (e.g. ~/.claude/rules/ for Claude Code).
/// Cached after first call via LazyLock.
static ASSISTANT_RULES_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| crate::assistant::detect_assistant().rules_dir());

pub fn assistant_rules_dir() -> &'static Path {
    &ASSISTANT_RULES_DIR
}

/// Check if running in test mode (WARDEN_TEST=1)
fn is_test_mode() -> bool {
    std::env::var("WARDEN_TEST").is_ok()
}

/// Enable log buffering (daemon mode — flushes once per request)
pub fn enable_log_buffering() {
    LOG_BUFFERED.store(true, Ordering::Relaxed);
}

/// Log level for structured logging
#[allow(dead_code)]
pub enum LogLevel {
    Deny,
    Allow,
    Advisory,
    Info,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Deny => "DENY",
            LogLevel::Allow => "ALLOW",
            LogLevel::Advisory => "ADVISORY",
            LogLevel::Info => "INFO",
            LogLevel::Error => "ERROR",
        }
    }
}

/// Structured log entry with level, turn, action, and detail.
/// Format: `TIMESTAMP [LEVEL] t=TURN action: detail`
pub fn log_structured(hook_name: &str, level: LogLevel, action: &str, detail: &str) {
    if is_test_mode() {
        return;
    }

    let ts = now_iso();
    let turn = {
        let state = super::read_session_state();
        state.turn
    };
    let line = format!(
        "{} [{}] t={} {}: {}",
        ts,
        level.as_str(),
        turn,
        action,
        detail
    );

    if LOG_BUFFERED.load(Ordering::Relaxed) {
        if let Ok(mut buf) = LOG_BUFFER.lock() {
            buf.entry(hook_name.to_string()).or_default().push(line);
        }
        return;
    }

    write_log_line(hook_name, &line);
}

/// Append a log line to logs/<hook_name>.log with rotation at 100KB.
/// In daemon mode, buffers entries and flushes once per request.
pub fn log(hook_name: &str, message: &str) {
    if is_test_mode() {
        return;
    }

    let ts = now_iso();
    let line = format!("{} {}", ts, message);

    // Buffer in daemon mode — one file open per flush instead of per log call
    if LOG_BUFFERED.load(Ordering::Relaxed) {
        if let Ok(mut buf) = LOG_BUFFER.lock() {
            buf.entry(hook_name.to_string()).or_default().push(line);
        }
        return;
    }

    write_log_line(hook_name, &line);
}

/// Flush buffered log entries to disk (one file open per hook name)
pub fn flush_log_buffer() {
    let entries = {
        let mut buf = match LOG_BUFFER.lock() {
            Ok(b) => b,
            Err(e) => e.into_inner(),
        };
        std::mem::take(&mut *buf)
    };

    for (hook_name, lines) in entries {
        if lines.is_empty() {
            continue;
        }

        let dir = project_dir().join("logs");
        if !LOG_DIR_CREATED.load(Ordering::Relaxed) {
            let _ = fs::create_dir_all(&dir);
            LOG_DIR_CREATED.store(true, Ordering::Relaxed);
        }
        let path = dir.join(format!("{}.log", hook_name));

        rotate_if_needed(&path, 102_400, 51_200);

        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            for line in &lines {
                let _ = writeln!(f, "{}", line);
            }
        }
    }
}

/// Write a single log line immediately (non-daemon mode)
fn write_log_line(hook_name: &str, line: &str) {
    let dir = project_dir().join("logs");
    if !LOG_DIR_CREATED.load(Ordering::Relaxed) {
        let _ = fs::create_dir_all(&dir);
        LOG_DIR_CREATED.store(true, Ordering::Relaxed);
    }
    let path = dir.join(format!("{}.log", hook_name));

    rotate_if_needed(&path, 102_400, 51_200);

    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{}", line);
    }
}

/// Append a JSONL entry to session-notes.jsonl
pub fn add_session_note(note_type: &str, detail: &str) {
    add_session_note_ext(note_type, detail, None);
}

/// Append a JSONL entry with optional structured data field
pub fn add_session_note_ext(note_type: &str, detail: &str, data: Option<&serde_json::Value>) {
    if is_test_mode() {
        return;
    }
    let path = project_dir().join("session-notes.jsonl");

    rotate_if_needed(&path, 102_400, 51_200);

    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let mut entry = serde_json::json!({
            "ts": now_iso(),
            "type": note_type,
            "detail": detail,
        });
        if let Some(d) = data {
            entry["data"] = d.clone();
        }
        let _ = writeln!(f, "{}", entry);
    }

    // Also persist to redb events table when available
    if super::storage::is_available() {
        let mut entry = serde_json::json!({
            "ts": now_iso(),
            "type": note_type,
            "detail": detail,
        });
        if let Some(d) = data {
            entry["data"] = d.clone();
        }
        let _ = super::storage::append_event(entry.to_string().as_bytes());
    }
}

/// Read the last N bytes from a file (for dedup checks)
pub fn read_tail(path: &Path, bytes: u64) -> String {
    if let Ok(mut f) = fs::File::open(path)
        && let Ok(meta) = f.metadata()
    {
        let start = meta.len().saturating_sub(bytes);
        let _ = f.seek(SeekFrom::Start(start));
        let mut buf = String::new();
        let _ = f.read_to_string(&mut buf);
        return buf;
    }
    String::new()
}

/// Rotate a file if it exceeds max_size, keeping keep_tail bytes.
/// Uses advisory file locking to prevent two processes from rotating simultaneously.
fn rotate_if_needed(path: &Path, max_size: u64, keep_tail: u64) {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return,
    };
    if meta.len() <= max_size {
        return;
    }

    // Try to acquire advisory lock — if another process is rotating, skip
    let lock_path = path.with_extension("lock");
    let _lock = match try_file_lock(&lock_path) {
        Some(l) => l,
        None => return, // Another process is rotating — skip this time
    };

    // Re-check size after acquiring lock (might have been rotated already)
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return,
    };
    if meta.len() <= max_size {
        return;
    }

    // Read tail, write truncated
    if let Ok(mut f) = fs::File::open(path) {
        let start = meta.len().saturating_sub(keep_tail);
        let _ = f.seek(SeekFrom::Start(start));
        let mut tail = String::new();
        let _ = f.read_to_string(&mut tail);
        if let Some(pos) = tail.find('\n') {
            let _ = fs::write(path, &tail[pos + 1..]);
        }
    }
}

// ─── Advisory file locking ──────────────────────────────────────────────────

struct FileLock {
    #[cfg(windows)]
    _handle: *mut std::ffi::c_void,
    #[cfg(not(windows))]
    _path: PathBuf,
}

impl Drop for FileLock {
    fn drop(&mut self) {
        #[cfg(windows)]
        // SAFETY: self._handle is a valid file handle from CreateFileW; Drop runs exactly once.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self._handle);
        }
    }
}

/// Try to acquire an advisory file lock. Returns None if lock is held.
fn try_file_lock(lock_path: &Path) -> Option<FileLock> {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows_sys::Win32::Storage::FileSystem::{
            CREATE_ALWAYS, CreateFileW, FILE_ATTRIBUTE_NORMAL,
        };

        let wide_path: Vec<u16> = OsStr::new(lock_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: wide_path is null-terminated; exclusive (no sharing) open acts as advisory lock; handle checked.
        unsafe {
            let handle = CreateFileW(
                wide_path.as_ptr(),
                0x80000000 | 0x40000000, // GENERIC_READ | GENERIC_WRITE
                0,                       // No sharing — exclusive
                std::ptr::null(),
                CREATE_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            );

            if handle == INVALID_HANDLE_VALUE {
                return None; // Lock held by another process
            }

            Some(FileLock { _handle: handle })
        }
    }

    #[cfg(not(windows))]
    {
        // Simple lock file — create exclusively
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock_path)
        {
            Ok(_) => Some(FileLock {
                _path: lock_path.to_path_buf(),
            }),
            Err(_) => None,
        }
    }
}
