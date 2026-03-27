// ─── Compiled Patterns — shared regex compilation ────────────────────────────
//
// One-time compilation of all safety/hallucination/destructive/substitution
// patterns into RegexSets for O(1) matching. Used by both pretool_bash
// (gatekeeper pipeline) and sentinel (signal production).
//
// Moved here from pretool_bash so sentinel can use the same compiled sets
// instead of recompiling Regex::new() per pattern per call.
// ──────────────────────────────────────────────────────────────────────────────

use crate::config;
use crate::rules;
use regex::{Regex, RegexSet};
use std::sync::LazyLock;

/// Compiled regex collections — built once, reused across calls.
/// Each category has both a RegexSet (fast boolean match in single DFA pass)
/// and a parallel messages Vec (indexed by RegexSet match result).
pub struct CompiledPatterns {
    // RegexSet for single-pass matching (Phase 1 optimization)
    // Parallel vecs: messages[i], shadow[i], ids[i] correspond to RegexSet match index i
    pub safety_set: RegexSet,
    pub safety_messages: Vec<String>,
    pub safety_shadow: Vec<bool>,
    pub safety_ids: Vec<String>,
    pub hallucination_set: RegexSet,
    pub hallucination_messages: Vec<String>,
    pub hallucination_shadow: Vec<bool>,
    pub hallucination_ids: Vec<String>,
    pub hallucination_advisory_set: RegexSet,
    pub hallucination_advisory_messages: Vec<String>,
    pub hallucination_advisory_ids: Vec<String>,
    pub destructive_set: RegexSet,
    pub destructive_messages: Vec<String>,
    pub destructive_shadow: Vec<bool>,
    pub destructive_ids: Vec<String>,
    pub advisories_set: RegexSet,
    pub advisories_messages: Vec<String>,
    pub advisories_ids: Vec<String>,
    pub auto_allow_set: RegexSet,
    // Sequential (needs per-pattern runtime checks like tool availability)
    pub transforms: Vec<(Regex, String, String)>, // (regex, source_tool, target_tool)
    pub substitutions: Vec<(Regex, String)>,
    // Special-purpose single regexes
    pub cd_just_re: Option<Regex>,
    pub zero_trace_cmd: Option<Regex>,
    pub zero_trace_path_exclude: Option<Regex>,
    pub zero_trace_write: Option<Regex>,
    pub verbose: Vec<Regex>,
    pub short: Vec<Regex>,
    pub just_verbose_re: Option<Regex>,
    pub just_short_re: Option<Regex>,
    pub port_re: Option<Regex>,
}

pub static PATTERNS: LazyLock<CompiledPatterns> = LazyLock::new(|| {
    let r = &*rules::RULES;

    let compile_merged_pairs = |pairs: &[rules::RuleEntry]| -> Vec<(Regex, String)> {
        pairs
            .iter()
            .filter_map(|(_id, pat, msg, _shadow)| {
                regex::RegexBuilder::new(pat)
                    .size_limit(1 << 16)
                    .dfa_size_limit(1 << 18)
                    .nest_limit(50)
                    .build()
                    .ok()
                    .map(|re| (re, msg.clone()))
            })
            .collect()
    };

    // Validate a regex pattern with size limits to prevent ReDoS from user-supplied TOML
    let validated_regex = |pat: &str| -> bool {
        regex::RegexBuilder::new(pat)
            .size_limit(1 << 16) // 64KB DFA size limit
            .dfa_size_limit(1 << 18) // 256KB DFA limit
            .nest_limit(50) // nesting depth limit
            .build()
            .is_ok()
    };

    // Build RegexSet + parallel message/shadow/id Vecs from pattern pairs (with ReDoS protection)
    let compile_set_with_messages =
        |pairs: &[rules::RuleEntry]| -> (RegexSet, Vec<String>, Vec<bool>, Vec<String>) {
            let mut valid_patterns: Vec<String> = Vec::new();
            let mut messages: Vec<String> = Vec::new();
            let mut shadow_flags: Vec<bool> = Vec::new();
            let mut rule_ids: Vec<String> = Vec::new();
            for (id, pat, msg, shadow) in pairs {
                if validated_regex(pat) {
                    valid_patterns.push(pat.clone());
                    messages.push(msg.clone());
                    shadow_flags.push(*shadow);
                    rule_ids.push(id.clone());
                }
            }
            let set = regex::RegexSetBuilder::new(&valid_patterns)
                .size_limit(1 << 20)
                .dfa_size_limit(1 << 22)
                .nest_limit(50)
                .build()
                .unwrap_or_else(|_| RegexSet::empty());
            (set, messages, shadow_flags, rule_ids)
        };

    // Build RegexSet from string list (no messages, with ReDoS protection)
    let compile_set_from_list = |pats: &[String]| -> RegexSet {
        let valid: Vec<&str> = pats
            .iter()
            .filter(|p| validated_regex(p))
            .map(|s| s.as_str())
            .collect();
        regex::RegexSetBuilder::new(&valid)
            .size_limit(1 << 20)
            .dfa_size_limit(1 << 22)
            .nest_limit(50)
            .build()
            .unwrap_or_else(|_| RegexSet::empty())
    };

    let just_verbose_joined = r
        .just_verbose
        .iter()
        .map(|s| regex::escape(s))
        .collect::<Vec<_>>()
        .join("|");
    let just_short_joined = r
        .just_short
        .iter()
        .map(|s| regex::escape(s))
        .collect::<Vec<_>>()
        .join("|");

    // Merge legacy JSON overrides into pairs before compiling sets
    let overrides = &*crate::handlers::config_override::OVERRIDES;

    let mut safety_pairs = r.safety_pairs.clone();
    let mut hallucination_pairs = r.hallucination_pairs.clone();
    let mut hallucination_advisory_pairs = r.hallucination_advisory_pairs.clone();
    let mut substitutions_pairs = r.substitutions_pairs.clone();
    let mut advisories_pairs = r.advisories_pairs.clone();
    let mut auto_allow_patterns = r.auto_allow_patterns.clone();

    for (i, (pat, msg)) in overrides.safety.iter().enumerate() {
        safety_pairs.push((
            format!("override_safety.{}", i),
            pat.clone(),
            msg.clone(),
            false,
        ));
    }
    for (i, (pat, msg)) in overrides.hallucination.iter().enumerate() {
        hallucination_pairs.push((
            format!("override_hallucination.{}", i),
            pat.clone(),
            msg.clone(),
            false,
        ));
    }
    for (i, (pat, msg)) in overrides.hallucination_advisory.iter().enumerate() {
        hallucination_advisory_pairs.push((
            format!("override_hallucination_advisory.{}", i),
            pat.clone(),
            msg.clone(),
            false,
        ));
    }
    for (i, (pat, msg)) in overrides.substitutions.iter().enumerate() {
        substitutions_pairs.push((
            format!("override_substitution.{}", i),
            pat.clone(),
            msg.clone(),
            false,
        ));
    }
    for (i, (pat, msg)) in overrides.advisories.iter().enumerate() {
        advisories_pairs.push((
            format!("override_advisory.{}", i),
            pat.clone(),
            msg.clone(),
            false,
        ));
    }
    for pat in &overrides.auto_allow {
        auto_allow_patterns.push(pat.clone());
    }

    // Build RegexSets (single DFA pass for boolean matching)
    let (safety_set, safety_messages, safety_shadow, safety_ids) =
        compile_set_with_messages(&safety_pairs);
    let (hallucination_set, hallucination_messages, hallucination_shadow, hallucination_ids) =
        compile_set_with_messages(&hallucination_pairs);
    let (
        hallucination_advisory_set,
        hallucination_advisory_messages,
        _,
        hallucination_advisory_ids,
    ) = compile_set_with_messages(&hallucination_advisory_pairs);
    let (destructive_set, destructive_messages, destructive_shadow, destructive_ids) =
        compile_set_with_messages(&r.destructive_pairs);
    let (advisories_set, advisories_messages, _, advisories_ids) =
        compile_set_with_messages(&advisories_pairs);
    let auto_allow_set = compile_set_from_list(&auto_allow_patterns);

    // Substitutions stay sequential (need per-pattern tool availability check)
    let substitutions = compile_merged_pairs(&substitutions_pairs);

    // Transforms: compile from config::TRANSFORMS
    let transforms: Vec<(Regex, String, String)> = crate::config::core::substitutions::TRANSFORMS
        .iter()
        .filter_map(|(pat, src, tgt)| {
            Regex::new(pat)
                .ok()
                .map(|re| (re, src.to_string(), tgt.to_string()))
        })
        .collect();

    CompiledPatterns {
        safety_set,
        safety_messages,
        safety_shadow,
        safety_ids,
        hallucination_set,
        hallucination_messages,
        hallucination_shadow,
        hallucination_ids,
        hallucination_advisory_set,
        hallucination_advisory_messages,
        hallucination_advisory_ids,
        destructive_set,
        destructive_messages,
        destructive_shadow,
        destructive_ids,
        advisories_set,
        advisories_messages,
        advisories_ids,
        auto_allow_set,
        transforms,
        substitutions,
        cd_just_re: Regex::new(r#"^\s*cd\s+["']?([^"'&;]+?)["']?\s*&&\s*just\s+(.+)$"#).ok(),
        zero_trace_cmd: if r.zero_trace_cmd.is_empty() {
            None
        } else {
            Regex::new(&r.zero_trace_cmd).ok()
        },
        zero_trace_path_exclude: if r.zero_trace_path_exclude.is_empty() {
            None
        } else {
            Regex::new(&r.zero_trace_path_exclude).ok()
        },
        zero_trace_write: if r.zero_trace_write.is_empty() {
            None
        } else {
            Regex::new(&r.zero_trace_write).ok()
        },
        verbose: config::VERBOSE_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect(),
        short: config::SHORT_COMMANDS
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect(),
        just_verbose_re: Regex::new(&format!(r"(?i)^\s*just\s+({})\b", just_verbose_joined)).ok(),
        just_short_re: Regex::new(&format!(r"(?i)^\s*just\s+({})\b", just_short_joined)).ok(),
        port_re: Regex::new(r"(?:localhost|127\.0\.0\.1):(\d+)").ok(),
    }
});
