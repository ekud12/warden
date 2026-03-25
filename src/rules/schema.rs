// ─── rules::schema — TOML configuration schema ──────────────────────────────

use serde::Deserialize;

/// Top-level rules.toml schema
#[derive(Deserialize, Debug, Default)]
pub struct RulesFile {
    #[serde(default)]
    pub safety: PatternSection,
    #[serde(default)]
    pub destructive: PatternSection,
    #[serde(default)]
    pub substitutions: PatternSection,
    #[serde(default)]
    pub advisories: PatternSection,
    #[serde(default)]
    pub hallucination: PatternSection,
    #[serde(default)]
    pub hallucination_advisory: PatternSection,
    #[serde(default)]
    pub sensitive_paths_deny: PatternSection,
    #[serde(default)]
    pub sensitive_paths_warn: PatternSection,
    #[serde(default)]
    pub auto_allow: AutoAllowSection,
    #[serde(default)]
    pub zero_trace: ZeroTraceSection,
    #[serde(default)]
    pub just: JustSection,
    #[serde(default)]
    pub thresholds: ThresholdsSection,
    #[serde(default)]
    pub restrictions: Option<RestrictionsConfig>,
    /// Enable git read-only mode (block all mutating git commands). OFF by default.
    #[serde(default)]
    pub git_readonly: Option<bool>,
    /// Session mode: diagnose, implement, refactor, release. Adjusts thresholds.
    #[serde(default)]
    pub session_mode: Option<String>,
    /// Custom command filters for output compression (extends compiled defaults)
    #[serde(default)]
    pub command_filters: Vec<CommandFilter>,
}

/// A section of pattern+message pairs with optional replace mode
#[derive(Deserialize, Debug, Default)]
pub struct PatternSection {
    /// If true, replaces compiled defaults entirely. If false (default), appends.
    #[serde(default)]
    pub replace: bool,
    /// Pattern entries: each has a regex `match` and a `msg` string
    #[serde(default)]
    pub patterns: Vec<PatternEntry>,
}

/// Single pattern entry: regex + message
#[derive(Deserialize, Debug, Clone)]
pub struct PatternEntry {
    #[serde(rename = "match")]
    pub regex: String,
    pub msg: String,
    /// Shadow mode: log "would deny" without blocking. Enables safe rule rollout.
    #[serde(default)]
    pub shadow: bool,
}

/// Auto-allow section (regex list)
#[derive(Deserialize, Debug, Default)]
pub struct AutoAllowSection {
    #[serde(default)]
    pub replace: bool,
    #[serde(default)]
    pub patterns: Vec<String>,
}

/// Zero-trace overrides
#[derive(Deserialize, Debug, Default)]
pub struct ZeroTraceSection {
    pub content_pattern: Option<String>,
    pub cmd_pattern: Option<String>,
    pub write_pattern: Option<String>,
    pub path_exclude: Option<String>,
}

/// Just-first configuration
#[derive(Deserialize, Debug, Default)]
pub struct JustSection {
    #[serde(default)]
    pub replace_map: bool,
    #[serde(default)]
    pub map: Vec<JustMapEntry>,
    #[serde(default)]
    pub replace_verbose: bool,
    #[serde(default)]
    pub verbose: Vec<String>,
    #[serde(default)]
    pub replace_short: bool,
    #[serde(default)]
    pub short: Vec<String>,
}

/// Single just-map entry: prefix → recipe
#[derive(Deserialize, Debug, Clone)]
pub struct JustMapEntry {
    pub prefix: String,
    pub recipe: String,
}

/// Command filter rule for data-driven output compression
#[derive(Deserialize, Debug, Clone, serde::Serialize)]
pub struct CommandFilter {
    /// Regex or substring to match against the command string
    #[serde(rename = "match")]
    pub cmd_match: String,
    /// Filter strategy: strip_matching, keep_matching, dedup, head_tail, passthrough
    #[serde(default = "default_strategy")]
    pub strategy: String,
    /// Patterns for lines to keep (regex)
    #[serde(default)]
    pub keep_patterns: Vec<String>,
    /// Patterns for lines to strip (regex)
    #[serde(default)]
    pub strip_patterns: Vec<String>,
    /// Number of lines to keep from the start
    #[serde(default = "default_keep_n")]
    pub keep_first: usize,
    /// Number of lines to keep from the end
    #[serde(default = "default_keep_n")]
    pub keep_last: usize,
    /// Summary line template ({kept}, {total}, {stripped} placeholders)
    #[serde(default)]
    pub summary_template: String,
    /// Maximum output lines after filtering
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
}

fn default_strategy() -> String {
    "strip_matching".to_string()
}
fn default_keep_n() -> usize {
    3
}
fn default_max_lines() -> usize {
    40
}

/// Restriction disable list — allows selectively disabling restrictions by ID
#[derive(Deserialize, Debug, Default)]
pub struct RestrictionsConfig {
    #[serde(default)]
    pub disable: Vec<String>,
}

/// Threshold overrides (all optional — falls back to compiled defaults)
#[derive(Deserialize, Debug, Default)]
pub struct ThresholdsSection {
    pub max_read_size_kb: Option<u64>,
    pub max_mcp_output_kb: Option<usize>,
    pub max_string_len: Option<usize>,
    pub max_array_len: Option<usize>,
    /// Doom-loop threshold: inject warning after N identical tool calls (default: 3)
    pub doom_loop_threshold: Option<u8>,
    /// Output offload threshold in KB: write to scratch file if output exceeds (default: 8)
    pub offload_threshold_kb: Option<usize>,
    /// Token budget advisory threshold in K tokens (default: 700)
    pub token_budget_advisory_k: Option<u64>,
    /// Progressive read deny turn threshold (default: 80)
    pub progressive_read_deny_turn: Option<u32>,
    /// Progressive read advisory turn threshold (default: 50)
    pub progressive_read_advisory_turn: Option<u32>,
    /// Rules re-injection interval in turns (default: 30)
    pub rules_reinject_interval: Option<u32>,
    /// Drift detection threshold: deny count in 10-turn window (default: 3)
    pub drift_threshold: Option<u8>,
    /// Error slope threshold for heuristic advisory (default: 0.5)
    pub error_slope_threshold: Option<f64>,
    /// Turns without milestone before stale-session advisory (default: 10)
    pub stale_milestone_turns: Option<u32>,
    /// Token burn threshold in K tokens/turn (default: 15)
    pub token_burn_threshold_k: Option<u64>,
    /// Consecutive stagnation snapshots before advisory (default: 3)
    pub stagnation_turns: Option<u32>,
}
