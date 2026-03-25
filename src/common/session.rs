// ─── common::session — session state persistence ─────────────────────────────

use super::io::{cwd_hash8, get_project_cwd, project_dir};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Type alias: BTreeMap for deterministic serialization order
type HashMap<K, V> = BTreeMap<K, V>;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

/// Whether we're running in daemon mode (in-memory cache active)
static DAEMON_MODE: AtomicBool = AtomicBool::new(false);

/// In-memory session state cache (daemon mode only), keyed by hash8 of CWD.
/// DashMap provides lock-free concurrent access — no Mutex contention.
static SESSION_CACHE: LazyLock<DashMap<String, SessionState>> =
    LazyLock::new(DashMap::new);

/// Which cache keys have unsaved changes needing disk flush
static CACHE_DIRTY_KEYS: LazyLock<DashMap<String, ()>> =
    LazyLock::new(DashMap::new);

/// Per-file read tracking entry
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FileReadEntry {
    pub hash: u64,
    pub turn: u32,
    pub size: u64,
    /// File modification time (seconds since epoch) for stale-read detection
    #[serde(default)]
    pub mtime: u64,
}

/// Structured goal stack: primary goal + current subgoal + blocked-on status
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct GoalStack {
    pub primary: String,
    pub subgoal: String,
    pub blocked_on: String,
}

/// Per-command output tracking entry
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CommandEntry {
    pub hash: u64,
    pub turn: u32,
    #[serde(default)]
    pub output_tokens: u64,
}

/// Per-turn telemetry snapshot for trend analysis
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TurnSnapshot {
    pub turn: u32,
    pub errors_unresolved: u32,
    pub explore_count: u32,
    pub files_edited_count: u16,
    pub files_read_count: u16,
    pub tokens_in_delta: u64,
    pub tokens_out_delta: u64,
    pub milestones_hit: bool,
    pub edits_this_turn: bool,
    pub denials_this_turn: u8,
}

/// Mutable session state persisted to session-state.json
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SessionState {
    #[serde(default)]
    pub turn: u32,
    #[serde(default)]
    pub files_read: HashMap<String, FileReadEntry>,
    #[serde(default)]
    pub files_edited: Vec<String>,
    #[serde(default)]
    pub explore_count: u32,
    #[serde(default)]
    pub last_edit_turn: u32,
    #[serde(default)]
    pub commands: HashMap<String, CommandEntry>,
    #[serde(default)]
    pub errors_unresolved: u32,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub current_task: String,
    #[serde(default)]
    pub last_milestone: String,
    #[serde(default)]
    pub last_build_turn: u32,
    #[serde(default)]
    pub git_summary: String,
    #[serde(default)]
    pub git_summary_turn: u32,
    #[serde(default)]
    pub estimated_tokens_in: u64,
    #[serde(default)]
    pub estimated_tokens_out: u64,
    #[serde(default)]
    pub estimated_tokens_saved: u64,
    #[serde(default)]
    pub savings_dedup: u32,
    #[serde(default)]
    pub savings_deny: u32,
    #[serde(default)]
    pub savings_build_skip: u32,
    #[serde(default)]
    pub savings_truncation: u32,
    #[serde(default)]
    pub last_build_output_tokens: u64,
    #[serde(default)]
    pub injection_warn_counts: BTreeMap<String, u32>,
    /// Turn at which last compaction occurred (0 = never compacted)
    #[serde(default)]
    pub last_compaction_turn: u32,
    /// Last file path edited (for post-edit read suppression)
    #[serde(default)]
    pub last_edited_file: String,
    /// Per-category advisory cooldowns: category → last turn emitted
    #[serde(default)]
    pub advisory_cooldowns: HashMap<String, u32>,
    /// Last injected context hash (for context-delta dedup)
    #[serde(default)]
    pub last_context_hash: u64,
    /// Doom-loop detection: tool call fingerprint → consecutive repeat count
    #[serde(default)]
    pub tool_fingerprints: HashMap<u64, u8>,
    /// Recent denial turn numbers for drift detection (bounded to last 20)
    #[serde(default)]
    pub recent_denial_turns: Vec<u32>,
    /// Per-turn telemetry snapshots (bounded to last 20)
    #[serde(default)]
    pub turn_snapshots: Vec<TurnSnapshot>,
    /// Previous snapshot's cumulative tokens_in (for delta computation)
    #[serde(default)]
    pub prev_snapshot_tokens_in: u64,
    /// Previous snapshot's cumulative tokens_out (for delta computation)
    #[serde(default)]
    pub prev_snapshot_tokens_out: u64,
    #[serde(default)]
    pub adaptive: crate::handlers::adaptation::AdaptiveState,
    // ─── Predictive intelligence fields ──
    /// Extracted session goal from first user message
    #[serde(default)]
    pub session_goal: String,
    /// Action history for entropy calculation (bounded to 20)
    #[serde(default)]
    pub action_history: Vec<String>,
    /// Initial working set (first 5 file directories touched)
    #[serde(default)]
    pub initial_working_set: Vec<String>,
    /// Markov transition counts: "action_a→action_b" → count
    #[serde(default)]
    pub action_transitions: HashMap<String, u32>,
    /// Last turn where a file in initial_working_set was touched
    #[serde(default)]
    pub last_initial_set_touch_turn: u32,
    /// Whether a context switch was detected this session
    #[serde(default)]
    pub context_switch_detected: bool,
    /// Rolling working set: recent directories (bounded to 10)
    #[serde(default)]
    pub rolling_working_set: Vec<String>,
    /// Rule IDs that fired (denied) during this session — for effectiveness tracking
    #[serde(default)]
    pub rules_fired: Vec<String>,

    // ─── Intelligence: Verification Debt ──
    /// Edits since last successful build/test verification
    #[serde(default)]
    pub edits_since_verification: u32,
    /// Reads since last edit (exploration without commitment)
    #[serde(default)]
    pub reads_since_edit: u32,

    // ─── Intelligence: Focus Score ──
    /// All directories touched this session (bounded to 30)
    #[serde(default)]
    pub directories_touched: Vec<String>,
    /// Subsystem switches without milestone
    #[serde(default)]
    pub subsystem_switches: u32,

    // ─── Intelligence: Negative Memory ──
    /// Dead ends: "file_or_cmd:reason" (bounded to 20)
    #[serde(default)]
    pub dead_ends: Vec<String>,
    /// Failed command prefixes: hash → failure count
    #[serde(default)]
    pub failed_commands: HashMap<String, u32>,

    // ─── Intelligence: Goal Stack ──
    /// Structured goal (replaces flat session_goal for new sessions)
    #[serde(default)]
    pub goal_stack: GoalStack,

    // ─── Intelligence: Checkpoint ──
    /// Turns since last milestone or verification
    #[serde(default)]
    pub turns_since_checkpoint: u32,

    // ─── Intelligence: Compression Risk ──
    /// Times a command was re-run after truncation
    #[serde(default)]
    pub retries_after_truncation: u32,

    // ─── Project metadata ──
    /// Auto-detected project type (rust/node/python/go/java/unknown)
    #[serde(default)]
    pub project_type: String,
}

/// Bounds for session state collections
const MAX_FILES_READ: usize = 50;
const MAX_COMMANDS: usize = 20;
const MAX_DECISIONS: usize = 10;
const MAX_FILES_EDITED: usize = 30;
const MAX_TURN_SNAPSHOTS: usize = 20;
const MAX_FINGERPRINTS: usize = 30;
const MAX_RECENT_DENIALS: usize = 20;

/// Advisory cooldown window (turns). Same-category advisory won't fire again within this window.
const ADVISORY_COOLDOWN: u32 = 5;

impl SessionState {
    /// Check if an advisory category is on cooldown. If not, marks it as emitted.
    pub fn advisory_ready(&mut self, category: &str) -> bool {
        if let Some(&last_turn) = self.advisory_cooldowns.get(category)
            && self.turn.saturating_sub(last_turn) < ADVISORY_COOLDOWN {
                return false;
            }
        self.advisory_cooldowns.insert(category.to_string(), self.turn);
        true
    }

    /// Record a tool denial for drift detection
    pub fn record_denial(&mut self) {
        self.recent_denial_turns.push(self.turn);
        if self.recent_denial_turns.len() > 20 {
            self.recent_denial_turns.drain(..self.recent_denial_turns.len() - 20);
        }
    }

    /// Count denials in the last N turns
    pub fn denial_rate(&self, window: u32) -> u32 {
        let cutoff = self.turn.saturating_sub(window);
        self.recent_denial_turns.iter().filter(|&&t| t > cutoff).count() as u32
    }

    /// Enforce collection bounds by evicting oldest entries.
    /// Uses O(n) batch eviction instead of repeated O(n) min_by_key scans.
    pub fn enforce_bounds(&mut self) {
        // files_read: evict oldest by turn (O(n) sort + drain)
        if self.files_read.len() > MAX_FILES_READ {
            let excess = self.files_read.len() - MAX_FILES_READ;
            let mut entries: Vec<_> = self.files_read.keys().cloned().collect();
            entries.sort_by_key(|k| self.files_read.get(k).map(|v| v.turn).unwrap_or(0));
            for key in entries.into_iter().take(excess) {
                self.files_read.remove(&key);
            }
        }
        // commands: evict oldest by turn (O(n) sort + drain)
        if self.commands.len() > MAX_COMMANDS {
            let excess = self.commands.len() - MAX_COMMANDS;
            let mut entries: Vec<_> = self.commands.keys().cloned().collect();
            entries.sort_by_key(|k| self.commands.get(k).map(|v| v.turn).unwrap_or(0));
            for key in entries.into_iter().take(excess) {
                self.commands.remove(&key);
            }
        }
        // decisions: keep last N
        if self.decisions.len() > MAX_DECISIONS {
            let start = self.decisions.len() - MAX_DECISIONS;
            self.decisions.drain(..start);
        }
        // files_edited: keep last N
        if self.files_edited.len() > MAX_FILES_EDITED {
            let start = self.files_edited.len() - MAX_FILES_EDITED;
            self.files_edited.drain(..start);
        }
        // tool_fingerprints: evict lowest-count entries
        if self.tool_fingerprints.len() > MAX_FINGERPRINTS {
            let excess = self.tool_fingerprints.len() - MAX_FINGERPRINTS;
            let mut entries: Vec<_> = self.tool_fingerprints.iter().map(|(&k, &v)| (k, v)).collect();
            entries.sort_by_key(|(_, count)| *count);
            for (key, _) in entries.into_iter().take(excess) {
                self.tool_fingerprints.remove(&key);
            }
        }
        // recent_denial_turns: keep last MAX_RECENT_DENIALS
        if self.recent_denial_turns.len() > MAX_RECENT_DENIALS {
            let start = self.recent_denial_turns.len() - MAX_RECENT_DENIALS;
            self.recent_denial_turns.drain(..start);
        }
        // turn_snapshots: keep last MAX_TURN_SNAPSHOTS
        if self.turn_snapshots.len() > MAX_TURN_SNAPSHOTS {
            let start = self.turn_snapshots.len() - MAX_TURN_SNAPSHOTS;
            self.turn_snapshots.drain(..start);
        }
        // rolling_working_set: keep last 10
        if self.rolling_working_set.len() > 10 {
            let start = self.rolling_working_set.len() - 10;
            self.rolling_working_set.drain(..start);
        }
        // action_history: keep last 20
        if self.action_history.len() > 20 {
            let start = self.action_history.len() - 20;
            self.action_history.drain(..start);
        }
        // action_transitions: keep top 50 by count
        if self.action_transitions.len() > 50 {
            let mut entries: Vec<_> = self.action_transitions.iter().map(|(k, &v)| (k.clone(), v)).collect();
            entries.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
            self.action_transitions = entries.into_iter().take(50).collect();
        }
    }

    /// Emergency pruning when session state exceeds size threshold.
    /// Halves all collections to bring size down.
    pub fn aggressive_prune(&mut self) {
        if self.files_read.len() > 25 {
            let excess = self.files_read.len() - 25;
            let mut entries: Vec<_> = self.files_read.keys().cloned().collect();
            entries.sort_by_key(|k| self.files_read.get(k).map(|v| v.turn).unwrap_or(0));
            for key in entries.into_iter().take(excess) {
                self.files_read.remove(&key);
            }
        }
        self.decisions.truncate(5);
        self.files_edited.truncate(15);
        self.turn_snapshots.truncate(10);
        self.action_history.truncate(10);
        self.rolling_working_set.truncate(5);
        if self.action_transitions.len() > 25 {
            let mut entries: Vec<_> = std::mem::take(&mut self.action_transitions).into_iter().collect();
            entries.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
            self.action_transitions = entries.into_iter().take(25).collect();
        }
    }
}

/// Path to session-state.json (per-project, per-session if CLAUDE_SESSION_ID set)
pub fn session_state_path() -> PathBuf {
    let base = project_dir();
    if let Ok(sid) = std::env::var("CLAUDE_SESSION_ID")
        && !sid.is_empty() {
            let short = &cwd_hash8(&sid)[..4];
            return base.join(format!("session-state-{}.json", short));
        }
    base.join("session-state.json")
}

/// Cache key for the current project (hash8 of CWD + session ID if available)
fn cache_key() -> String {
    let cwd = get_project_cwd();
    let base = if cwd.is_empty() { "global".to_string() } else { cwd_hash8(&cwd) };
    if let Ok(sid) = std::env::var("CLAUDE_SESSION_ID")
        && !sid.is_empty() {
            return format!("{}-{}", base, &cwd_hash8(&sid)[..4]);
        }
    base
}

/// Enable daemon mode — activates in-memory session state cache
pub fn enable_daemon_mode() {
    DAEMON_MODE.store(true, Ordering::Relaxed);
}

/// Read session state — uses in-memory cache in daemon mode
pub fn read_session_state() -> SessionState {
    if DAEMON_MODE.load(Ordering::Relaxed) {
        let key = cache_key();
        if let Some(entry) = SESSION_CACHE.get(&key) {
            return entry.value().clone();
        }
        // First access — load from disk into cache
        let state = read_session_state_from_disk();
        SESSION_CACHE.insert(key, state.clone());
        return state;
    }
    read_session_state_from_disk()
}

/// Read session state from storage — tries redb first, falls back to JSON, then defaults (fail open)
fn read_session_state_from_disk() -> SessionState {
    // Try redb first (primary storage)
    if super::storage::is_available()
        && let Some(state) = super::storage::read_json::<SessionState>("session_state", "current") {
            return state;
        }
    // Fall back to JSON file (pre-migration or redb unavailable)
    let path = session_state_path();
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => SessionState::default(),
    }
}

/// Write session state — updates cache in daemon mode, debounces disk write.
/// No-op in CI environments (no state persistence needed).
pub fn write_session_state(state: &SessionState) {
    if crate::common::is_ci() { return; }
    if DAEMON_MODE.load(Ordering::Relaxed) {
        let key = cache_key();
        SESSION_CACHE.insert(key.clone(), state.clone());
        CACHE_DIRTY_KEYS.insert(key, ());
        return; // Disk write deferred to flush_session_cache()
    }
    write_session_state_to_disk(state);
}

/// Flush cached session state to disk (called after each daemon request)
pub fn flush_session_cache() {
    let dirty_keys: Vec<String> = CACHE_DIRTY_KEYS.iter()
        .map(|entry| entry.key().clone())
        .collect();
    CACHE_DIRTY_KEYS.clear();

    if dirty_keys.is_empty() {
        return;
    }
    for key in &dirty_keys {
        if let Some(entry) = SESSION_CACHE.get(key) {
            write_session_state_to_disk(entry.value());
        }
    }
}

/// Invalidate the session cache (called on session-start reset)
#[allow(dead_code)]
pub fn invalidate_session_cache() {
    SESSION_CACHE.clear();
    CACHE_DIRTY_KEYS.clear();
}

/// Write session state to storage — redb primary, JSON fallback.
/// Triggers aggressive pruning if serialized state exceeds 50KB.
fn write_session_state_to_disk(state: &SessionState) {
    // Size monitoring: prune if too large
    let state = {
        let json_size = serde_json::to_string(state).map(|j| j.len()).unwrap_or(0);
        if json_size > 50_000 {
            crate::common::log("session", &format!("State size {}KB > 50KB, pruning", json_size / 1024));
            let mut pruned = state.clone();
            pruned.aggressive_prune();
            pruned
        } else {
            state.clone()
        }
    };

    // Try redb first (primary storage)
    if super::storage::is_available()
        && super::storage::write_json("session_state", "current", &state).is_some() {
            return;
        }

    // Fall back to JSON file
    let path = session_state_path();
    let tmp_path = path.with_extension("json.tmp");
    let json = match serde_json::to_string_pretty(&state) {
        Ok(j) => j,
        Err(_) => return,
    };
    if fs::write(&tmp_path, &json).is_ok()
        && fs::rename(&tmp_path, &path).is_err()
    {
        let _ = fs::write(&path, &json);
        let _ = fs::remove_file(&tmp_path);
    }
}
