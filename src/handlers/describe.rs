// ─── describe — show active user overrides or full JSON dump ──────────────────
//
// Default mode: styled terminal output showing non-empty user overrides.
// Verbose mode (--all): full JSON dump of all compiled rules + overrides.
// Not daemon-eligible — runs directly without stdin JSON.
// ──────────────────────────────────────────────────────────────────────────────

use crate::config;
use crate::handlers::config_override::OVERRIDES;
use crate::install::term;

pub fn run(args: &[String]) {
    let verbose = args.iter().any(|a| a == "--all");

    if verbose {
        run_verbose();
    } else {
        run_default();
    }
}

/// Default mode: styled terminal output of non-empty user overrides.
fn run_default() {
    let ver = env!("CARGO_PKG_VERSION");

    // Version header
    eprintln!();
    term::print_bold(term::BRAND, "  W A R D E N");
    term::print_colored(term::DIM, &format!("  v{}\n", ver));
    eprintln!();

    let mut any_override = false;

    // Pair-based override categories: (label, user overrides, compiled rule count)
    let pair_categories: &[(&str, &[(String, String)], usize)] = &[
        ("safety", &OVERRIDES.safety, config::SAFETY.len()),
        (
            "substitutions",
            &OVERRIDES.substitutions,
            config::SUBSTITUTIONS.len(),
        ),
        ("advisories", &OVERRIDES.advisories, config::ADVISORIES.len()),
        (
            "hallucination",
            &OVERRIDES.hallucination,
            config::HALLUCINATION.len(),
        ),
        (
            "hallucination_advisory",
            &OVERRIDES.hallucination_advisory,
            config::HALLUCINATION_ADVISORY.len(),
        ),
    ];

    for &(name, overrides, compiled_count) in pair_categories {
        if !overrides.is_empty() {
            any_override = true;
            term::section(name);
            term::hint(&format!(
                "{} compiled rules ({} user overrides)",
                compiled_count,
                overrides.len()
            ));
            for (pattern, message) in overrides {
                term::print_colored(term::TEXT, "    ");
                term::print_bold(term::TEXT, pattern);
                term::print_colored(term::DIM, &format!("  {}\n", message));
            }
        }
    }

    // auto_allow (Vec<String>, not pairs)
    if !OVERRIDES.auto_allow.is_empty() {
        any_override = true;
        term::section("auto_allow");
        term::hint(&format!(
            "{} compiled rules ({} user overrides)",
            config::AUTO_ALLOW.len(),
            OVERRIDES.auto_allow.len()
        ));
        for pattern in &OVERRIDES.auto_allow {
            term::print_colored(term::TEXT, "    ");
            term::println_bold(term::TEXT, pattern);
        }
    }

    if !any_override {
        term::print_colored(term::DIM, "  No user overrides. Using all compiled defaults.\n");
    }

    eprintln!();
}

/// Verbose mode (--all): full JSON dump to stdout.
fn run_verbose() {
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
    println!(
        "{}",
        serde_json::to_string_pretty(&output).unwrap_or_default()
    );
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
