// ─── constants — single source of truth for product identity ─────────────────
//
// Change NAME to rebrand the entire product. All paths, pipes, binary names,
// log prefixes, and config keys derive from these constants.
// ──────────────────────────────────────────────────────────────────────────────

/// Product name — drives all derived identifiers
pub const NAME: &str = "warden";

/// Home directory name: ~/.warden/
pub const DIR: &str = ".warden";

/// Named pipe prefix: \\.\pipe\warden-{username}
pub const PIPE_PREFIX: &str = "warden";

/// Server binary name (used by ipc.rs for pid file and binary copy naming)
pub const DAEMON_NAME: &str = "warden-daemon";

/// Config file name inside home dir
pub const CONFIG_FILE: &str = "config.toml";

/// Personal rules template filename (used by install wizard)
pub const PERSONAL_RULES: &str = "personal.toml";

/// Session state file per project
pub const SESSION_STATE_FILE: &str = "session-state.json";
pub const SESSION_NOTES_FILE: &str = "session-notes.jsonl";
pub const PROJECT_STATS_FILE: &str = "stats.json";

/// Legacy directory name (for migration)
pub const LEGACY_DIR: &str = ".hookctl";
