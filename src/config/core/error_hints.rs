// ─── core::error_hints — PostToolUseFailure recovery patterns ────────────────

/// Error hints: (regex, hint_message) — matched against tool failure stderr
pub const ERROR_HINTS: &[(&str, &str)] = &[
    // File system
    (
        r"(?i)EACCES|Permission denied|EPERM",
        "Permission denied. Close editors locking the file. Check permissions.",
    ),
    (
        r"(?i)ENOENT|No such file|cannot find the path|FileNotFoundError",
        "File not found. Verify path with fd or check for typos.",
    ),
    (
        r"(?i)EEXIST|already exists",
        "Already exists. Check if you need to overwrite or use a different name.",
    ),
    (
        r"(?i)ENOSPC|No space left|disk full",
        "Disk full. Check usage with dust, clean build artifacts.",
    ),
    (
        r"(?i)EISDIR|Is a directory",
        "Expected a file but got a directory. Check the path.",
    ),
    (
        r"(?i)EBUSY|resource busy|being used by another process",
        "File locked by another process. Close it, then retry.",
    ),
    // Stale state
    (
        r"(?i)old_string.*not found|old_string.*unique|not unique in the file",
        "File changed since last read. Re-read before editing.",
    ),
    (
        r"(?i)File has not been read yet",
        "Must Read the file before editing. Read first, then retry.",
    ),
    // Dependencies
    (
        r"(?i)ERESOLVE|peer dep|npm ERR!",
        "Dependency conflict. Try --legacy-peer-deps or check versions.",
    ),
    (
        r"(?i)Cannot find module|Module not found|ModuleNotFoundError",
        "Missing module. Run install, then check import paths.",
    ),
    (
        r"(?i)unresolved import|can't find crate",
        "Rust: missing dependency. Check Cargo.toml, then cargo build.",
    ),
    // Compilation
    (
        r"TS\d{4}",
        "TypeScript error. Read the error message and fix the source.",
    ),
    (
        r"(?i)error\[E\d{4}\]",
        "Rust compiler error. The compiler usually suggests the fix.",
    ),
    (
        r"(?i)CS\d{4}",
        "C# compiler error. Check the error code and fix the source.",
    ),
    (
        r"(?i)SyntaxError|IndentationError|ParseError",
        "Syntax error. Check for typos, missing brackets, or indentation.",
    ),
    // Tools
    (
        r"(?i)command not found|is not recognized|not found in PATH",
        "Tool not installed. Check with which/where, or install it.",
    ),
    (
        r"(?i)Timeout|timed out|ETIMEDOUT",
        "Timed out. Break into smaller operations or increase timeout.",
    ),
    // Network
    (
        r"(?i)ECONNREFUSED|ECONNRESET|connection refused",
        "Connection refused. Check if the service is running.",
    ),
    (
        r"(?i)ENOMEM|Out of memory|heap|allocation failed",
        "Out of memory. Reduce data size or increase limit.",
    ),
    // Git
    (
        r"(?i)CONFLICT|merge conflict|Automatic merge failed",
        "Merge conflict. Resolve markers, then stage and commit.",
    ),
    (
        r"(?i)fatal: not a git repository",
        "Not in a git repo. Check working directory.",
    ),
    // Rust-specific
    (
        r"cannot borrow .* as mutable",
        "Borrow checker: need &mut but have & reference. Check ownership.",
    ),
    (
        r"value used here after move",
        "Value moved. Clone before move, or restructure to avoid double use.",
    ),
    // Network
    (
        r"(?i)EADDRINUSE|address already in use",
        "Port in use. Check with lsof/netstat, kill the process or use a different port.",
    ),
    // Docker
    (
        r"(?i)Cannot connect to the Docker daemon",
        "Docker daemon not running. Start Docker Desktop or systemctl start docker.",
    ),
    (
        r"(?i)image .* not found|pull access denied",
        "Docker image not found. Check image name/tag or run docker pull first.",
    ),
    // Python
    (
        r"(?i)ModuleNotFoundError.*pip",
        "Python module missing. Activate venv first, then pip install.",
    ),
    (
        r"(?i)No module named 'venv'",
        "Python venv not installed. Install python3-venv package.",
    ),
];
