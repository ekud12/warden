// ─── describe — dump all active rules as JSON ─────────────────────────────────
//
// Diagnostic tool: `warden describe` outputs all compiled rules (defaults +
// overrides) as JSON to stdout. Not daemon-eligible — runs directly without
// stdin JSON.
// ──────────────────────────────────────────────────────────────────────────────

use crate::config;
use crate::handlers::config_override::OVERRIDES;

pub fn run() {
    let output = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "rules": {
            "safety": serialize_pairs(config::SAFETY),
            "hallucination": serialize_pairs(config::HALLUCINATION),
            "hallucination_advisory": serialize_pairs(config::HALLUCINATION_ADVISORY),
            "destructive": serialize_pairs(config::DESTRUCTIVE),
            "substitutions": serialize_pairs(config::SUBSTITUTIONS),
            "advisories": serialize_pairs(config::ADVISORIES),
            "injection_patterns": serialize_categorized(config::INJECTION_PATTERNS),
            "sensitive_paths_deny": serialize_pairs(config::SENSITIVE_PATHS_DENY),
            "sensitive_paths_warn": serialize_pairs(config::SENSITIVE_PATHS_WARN),
            "auto_allow": config::AUTO_ALLOW,
            "just_map": config::JUST_MAP.iter()
                .map(|(from, to)| serde_json::json!({"from": from, "to": to}))
                .collect::<Vec<_>>(),
        },
        "overrides": {
            "safety": &OVERRIDES.safety,
            "hallucination": &OVERRIDES.hallucination,
            "hallucination_advisory": &OVERRIDES.hallucination_advisory,
            "substitutions": &OVERRIDES.substitutions,
            "advisories": &OVERRIDES.advisories,
            "auto_allow": &OVERRIDES.auto_allow,
        },
        "thresholds": {
            "max_read_size": config::MAX_READ_SIZE,
            "max_mcp_output": config::MAX_MCP_OUTPUT,
            "max_string_len": config::MAX_STRING_LEN,
            "max_array_len": config::MAX_ARRAY_LEN,
        },
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
}

fn serialize_pairs(pairs: &[(&str, &str)]) -> Vec<serde_json::Value> {
    pairs
        .iter()
        .map(|(pat, msg)| serde_json::json!({"pattern": pat, "message": msg}))
        .collect()
}

fn serialize_categorized(pairs: &[(&str, &str)]) -> Vec<serde_json::Value> {
    pairs
        .iter()
        .map(|(pat, cat)| serde_json::json!({"pattern": pat, "category": cat}))
        .collect()
}
