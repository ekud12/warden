// ─── rules — TOML-based rule configuration with 3-tier merge ─────────────────
//
// Merge order:
//   1. Compiled defaults (config/*.rs) — always present
//   2. Global rules (~/.warden/rules.toml) — user overrides
//   3. Project rules (.warden/rules.toml in project root) — project-specific
//
// Each section supports `replace = true` to override defaults, otherwise appends.
// Loaded once via LazyLock (daemon restarts on file change via mtime check).

pub mod schema;

use crate::common;
use crate::config;
use schema::*;
use std::sync::LazyLock;

/// Merged rules from all 3 tiers, ready for consumption by handlers.
pub struct MergedRules {
    // Pattern pair sections: compiled defaults + TOML overrides
    pub safety_pairs: Vec<(String, String)>,
    pub destructive_pairs: Vec<(String, String)>,
    pub substitutions_pairs: Vec<(String, String)>,
    pub advisories_pairs: Vec<(String, String)>,
    pub hallucination_pairs: Vec<(String, String)>,
    pub hallucination_advisory_pairs: Vec<(String, String)>,
    pub sensitive_deny_pairs: Vec<(String, String)>,
    pub sensitive_warn_pairs: Vec<(String, String)>,
    pub auto_allow_patterns: Vec<String>,
    // Just config
    pub just_map: Vec<(String, String)>,
    pub just_verbose: Vec<String>,
    pub just_short: Vec<String>,
    // Zero-trace (use TOML override or compiled default)
    pub zero_trace_content: String,
    pub zero_trace_cmd: String,
    pub zero_trace_write: String,
    pub zero_trace_path_exclude: String,
    // Thresholds
    pub max_read_size: u64,
    pub max_mcp_output: usize,
    pub max_string_len: usize,
    pub max_array_len: usize,
    pub doom_loop_threshold: u8,
    pub offload_threshold: usize,
    pub token_budget_advisory: u64,
    pub progressive_read_deny_turn: u32,
    pub progressive_read_advisory_turn: u32,
    #[allow(dead_code)] // consumed indirectly via adaptation::phase_params defaults
    pub rules_reinject_interval: u32,
    #[allow(dead_code)] // consumed indirectly via adaptation::phase_params defaults
    pub drift_threshold: u8,
    // Heuristic policy thresholds
    pub error_slope_threshold: f64,
    pub stale_milestone_turns: u32,
    pub token_burn_threshold: u64,
    pub stagnation_turns: u32,
    // Disabled restrictions (checked by handlers at deny points)
    #[allow(dead_code)] // handlers will query this when restriction toggling lands
    pub disabled_restrictions: std::collections::HashSet<String>,
}

pub static RULES: LazyLock<MergedRules> = LazyLock::new(|| {
    let global = load_toml(&global_rules_path());
    let project = load_toml(&project_rules_path());
    merge(global, project)
});

/// Load a TOML rules file, returning defaults on any error (fail open).
fn load_toml(path: &std::path::Path) -> RulesFile {
    match std::fs::read_to_string(path) {
        Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
            common::log("rules", &format!("TOML parse error in {}: {}", path.display(), e));
            RulesFile::default()
        }),
        Err(_) => RulesFile::default(),
    }
}

/// Global rules path: ~/.warden/rules.toml
fn global_rules_path() -> std::path::PathBuf {
    common::hooks_dir().join("rules.toml")
}

/// Project rules path: .warden/rules.toml (relative to project root CWD)
fn project_rules_path() -> std::path::PathBuf {
    let cwd = common::io_get_project_cwd();
    if cwd.is_empty() {
        // No project context — return non-existent path
        return std::path::PathBuf::from(format!("{}/rules.toml", crate::constants::DIR));
    }
    std::path::PathBuf::from(&cwd).join(crate::constants::DIR).join("rules.toml")
}

/// Check if any rules.toml file exists (for mtime-based daemon restart)
pub fn rules_mtime() -> u64 {
    let mut max_mtime = 0u64;
    for path in [global_rules_path(), project_rules_path()] {
        if let Ok(meta) = std::fs::metadata(&path)
            && let Ok(modified) = meta.modified() {
                let mtime = modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if mtime > max_mtime {
                    max_mtime = mtime;
                }
            }
    }
    max_mtime
}

/// Merge compiled defaults + global TOML + project TOML into final rules.
fn merge(global: RulesFile, project: RulesFile) -> MergedRules {
    MergedRules {
        safety_pairs: {
            let mut pairs = merge_pairs(config::SAFETY, &global.safety, &project.safety);
            // Git readonly rules: only included when explicitly enabled
            let git_readonly = project.git_readonly.or(global.git_readonly).unwrap_or(false);
            if git_readonly {
                for (pat, msg) in config::GIT_SAFETY {
                    pairs.push((pat.to_string(), msg.to_string()));
                }
            }
            pairs
        },
        destructive_pairs: merge_pairs(config::DESTRUCTIVE, &global.destructive, &project.destructive),
        substitutions_pairs: merge_pairs(config::SUBSTITUTIONS, &global.substitutions, &project.substitutions),
        advisories_pairs: merge_pairs(config::ADVISORIES, &global.advisories, &project.advisories),
        hallucination_pairs: merge_pairs(config::HALLUCINATION, &global.hallucination, &project.hallucination),
        hallucination_advisory_pairs: merge_pairs(config::HALLUCINATION_ADVISORY, &global.hallucination_advisory, &project.hallucination_advisory),
        sensitive_deny_pairs: merge_pairs(config::SENSITIVE_PATHS_DENY, &global.sensitive_paths_deny, &project.sensitive_paths_deny),
        sensitive_warn_pairs: merge_pairs(config::SENSITIVE_PATHS_WARN, &global.sensitive_paths_warn, &project.sensitive_paths_warn),
        auto_allow_patterns: merge_string_list(config::AUTO_ALLOW, &global.auto_allow, &project.auto_allow),
        just_map: merge_just_map(&global.just, &project.just),
        just_verbose: merge_string_list_raw(
            &config::JUST_VERBOSE.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &global.just.verbose, global.just.replace_verbose,
            &project.just.verbose, project.just.replace_verbose,
        ),
        just_short: merge_string_list_raw(
            &config::JUST_SHORT.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &global.just.short, global.just.replace_short,
            &project.just.short, project.just.replace_short,
        ),
        zero_trace_content: project.zero_trace.content_pattern
            .or(global.zero_trace.content_pattern)
            .unwrap_or_else(|| config::ZERO_TRACE_CONTENT.to_string()),
        zero_trace_cmd: project.zero_trace.cmd_pattern
            .or(global.zero_trace.cmd_pattern)
            .unwrap_or_else(|| config::ZERO_TRACE_CMD.to_string()),
        zero_trace_write: project.zero_trace.write_pattern
            .or(global.zero_trace.write_pattern)
            .unwrap_or_else(|| config::ZERO_TRACE_WRITE.to_string()),
        zero_trace_path_exclude: project.zero_trace.path_exclude
            .or(global.zero_trace.path_exclude)
            .unwrap_or_else(|| config::ZERO_TRACE_PATH_EXCLUDE.to_string()),
        max_read_size: project.thresholds.max_read_size_kb
            .or(global.thresholds.max_read_size_kb)
            .map(|kb| kb * 1024)
            .unwrap_or(config::MAX_READ_SIZE),
        max_mcp_output: project.thresholds.max_mcp_output_kb
            .or(global.thresholds.max_mcp_output_kb)
            .map(|kb| kb * 1024)
            .unwrap_or(config::MAX_MCP_OUTPUT),
        max_string_len: project.thresholds.max_string_len
            .or(global.thresholds.max_string_len)
            .unwrap_or(config::MAX_STRING_LEN),
        max_array_len: project.thresholds.max_array_len
            .or(global.thresholds.max_array_len)
            .unwrap_or(config::MAX_ARRAY_LEN),
        doom_loop_threshold: project.thresholds.doom_loop_threshold
            .or(global.thresholds.doom_loop_threshold)
            .unwrap_or(3),
        offload_threshold: project.thresholds.offload_threshold_kb
            .or(global.thresholds.offload_threshold_kb)
            .map(|kb| kb * 1024)
            .unwrap_or(8 * 1024),
        token_budget_advisory: project.thresholds.token_budget_advisory_k
            .or(global.thresholds.token_budget_advisory_k)
            .unwrap_or(700) * 1000,
        progressive_read_deny_turn: project.thresholds.progressive_read_deny_turn
            .or(global.thresholds.progressive_read_deny_turn)
            .unwrap_or(80),
        progressive_read_advisory_turn: project.thresholds.progressive_read_advisory_turn
            .or(global.thresholds.progressive_read_advisory_turn)
            .unwrap_or(50),
        rules_reinject_interval: project.thresholds.rules_reinject_interval
            .or(global.thresholds.rules_reinject_interval)
            .unwrap_or(30),
        drift_threshold: project.thresholds.drift_threshold
            .or(global.thresholds.drift_threshold)
            .unwrap_or(3),
        error_slope_threshold: project.thresholds.error_slope_threshold
            .or(global.thresholds.error_slope_threshold)
            .unwrap_or(0.5),
        stale_milestone_turns: project.thresholds.stale_milestone_turns
            .or(global.thresholds.stale_milestone_turns)
            .unwrap_or(10),
        token_burn_threshold: project.thresholds.token_burn_threshold_k
            .or(global.thresholds.token_burn_threshold_k)
            .unwrap_or(15) * 1000,
        stagnation_turns: project.thresholds.stagnation_turns
            .or(global.thresholds.stagnation_turns)
            .unwrap_or(3),
        disabled_restrictions: {
            let mut set = std::collections::HashSet::new();
            if let Some(ref rc) = global.restrictions {
                set.extend(rc.disable.iter().cloned());
            }
            if let Some(ref rc) = project.restrictions {
                set.extend(rc.disable.iter().cloned());
            }
            set
        },
    }
}

/// Merge pattern pairs: compiled defaults + global TOML + project TOML.
/// If a tier has `replace = true`, it replaces everything before it.
fn merge_pairs(
    compiled: &[(&str, &str)],
    global: &PatternSection,
    project: &PatternSection,
) -> Vec<(String, String)> {
    // Start with compiled defaults
    let mut result: Vec<(String, String)> = compiled
        .iter()
        .map(|(p, m)| (p.to_string(), m.to_string()))
        .collect();

    // Apply global
    if global.replace {
        result.clear();
    }
    for entry in &global.patterns {
        result.push((entry.regex.clone(), entry.msg.clone()));
    }

    // Apply project
    if project.replace {
        result.clear();
    }
    for entry in &project.patterns {
        result.push((entry.regex.clone(), entry.msg.clone()));
    }

    result
}

/// Merge string list (auto_allow style): compiled defaults + global + project.
fn merge_string_list(
    compiled: &[&str],
    global: &AutoAllowSection,
    project: &AutoAllowSection,
) -> Vec<String> {
    let mut result: Vec<String> = compiled.iter().map(|s| s.to_string()).collect();

    if global.replace {
        result.clear();
    }
    result.extend(global.patterns.iter().cloned());

    if project.replace {
        result.clear();
    }
    result.extend(project.patterns.iter().cloned());

    result
}

/// Merge just_map: compiled defaults + global + project.
fn merge_just_map(global: &JustSection, project: &JustSection) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = config::JUST_MAP
        .iter()
        .map(|(p, r)| (p.to_string(), r.to_string()))
        .collect();

    if global.replace_map {
        result.clear();
    }
    for entry in &global.map {
        result.push((entry.prefix.clone(), entry.recipe.clone()));
    }

    if project.replace_map {
        result.clear();
    }
    for entry in &project.map {
        result.push((entry.prefix.clone(), entry.recipe.clone()));
    }

    result
}

/// Generic string list merge with replace flags.
fn merge_string_list_raw(
    compiled: &[String],
    global: &[String], global_replace: bool,
    project: &[String], project_replace: bool,
) -> Vec<String> {
    let mut result: Vec<String> = compiled.to_vec();

    if global_replace {
        result.clear();
    }
    result.extend(global.iter().cloned());

    if project_replace {
        result.clear();
    }
    result.extend(project.iter().cloned());

    result
}

/// Generate a default rules.toml template
pub fn default_template() -> String {
    format!(r#"# {name} rules.toml — customize hook behavior without recompiling
# Merge order: compiled defaults → ~/{dir}/rules.toml → {dir}/rules.toml
# Each section appends to defaults. Set `replace = true` to override entirely.

# [safety]
# patterns = [
#   {{ match = '^\s*poweroff\b', msg = "BLOCKED: poweroff" }},
# ]

# [substitutions]
# patterns = [
#   {{ match = '^\s*wget\b', msg = "Use xh instead of wget" }},
# ]

# [advisories]
# patterns = [
#   {{ match = '^\s*npm run\b', msg = "Advisory: Use just recipe instead" }},
# ]

# [auto_allow]
# patterns = ["^my-safe-tool "]

# [just]
# map = [
#   {{ prefix = "make build", recipe = "just build" }},
# ]
# verbose = ["my-verbose-recipe"]
# short = ["my-short-recipe"]

# [thresholds]
# max_read_size_kb = 50
# max_mcp_output_kb = 15

# [zero_trace]
# content_pattern = '(?i)generated\s+by\s+claude'
"#, name = crate::constants::NAME, dir = crate::constants::DIR)
}
