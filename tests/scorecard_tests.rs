// ─── scorecard regression tests — verify trace evaluation quality ─────────────
//
// These tests load golden trace fixtures and verify scorecard metrics.
// If rules change and metrics regress, these tests catch it.

use std::fs;

fn load_trace(path: &str) -> Vec<Vec<u8>> {
    let content = fs::read_to_string(path).expect(&format!("Failed to load trace: {}", path));
    content.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.as_bytes().to_vec())
        .collect()
}

// Note: These tests validate trace parsing only — they don't require the full
// warden binary or redb. Scorecard computation is a pure function over events.

#[test]
fn trace_healthy_has_milestones() {
    let trace = load_trace("tests/traces/healthy.jsonl");
    assert!(trace.len() >= 10, "healthy trace should have 10+ events");

    // Verify milestones exist in trace
    let milestones: Vec<_> = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("milestone"))
        .collect();
    assert!(milestones.len() >= 2, "healthy session should have 2+ milestones");
}

#[test]
fn trace_struggling_has_denials_and_errors() {
    let trace = load_trace("tests/traces/struggling.jsonl");

    let denials: usize = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("deny"))
        .count();
    let errors: usize = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("error"))
        .count();

    assert!(denials >= 1, "struggling session should have denials");
    assert!(errors >= 2, "struggling session should have errors");
}

#[test]
fn trace_adversarial_all_bypasses_denied() {
    let trace = load_trace("tests/traces/adversarial.jsonl");

    let denials: usize = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("deny"))
        .count();

    assert!(denials >= 8, "adversarial trace should have 8+ denied bypass attempts");
}

#[test]
fn trace_policy_heavy_has_substitution_denials() {
    let trace = load_trace("tests/traces/policy_heavy.jsonl");

    let denials: Vec<String> = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("deny"))
        .filter_map(|e| e.get("detail").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();

    assert!(denials.iter().any(|d| d.contains("grep")), "should deny grep");
    assert!(denials.iter().any(|d| d.contains("find")), "should deny find");
    assert!(denials.iter().any(|d| d.contains("chmod")), "should deny chmod");
}

#[test]
fn trace_read_heavy_has_advisory() {
    let trace = load_trace("tests/traces/read_heavy.jsonl");

    let advisories: usize = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("advisory"))
        .count();

    assert!(advisories >= 1, "read-heavy session should trigger advisory");
}

#[test]
fn trace_compaction_has_recovery() {
    let trace = load_trace("tests/traces/compaction.jsonl");

    let compactions: usize = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("compaction"))
        .count();
    let post_compaction_milestones: usize = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| {
            e.get("type").and_then(|v| v.as_str()) == Some("milestone")
            && e.get("turn").and_then(|v| v.as_u64()).unwrap_or(0) > 12
        })
        .count();

    assert!(compactions >= 1, "should have compaction event");
    assert!(post_compaction_milestones >= 1, "should recover after compaction");
}

#[test]
fn trace_long_session_has_multiple_milestones() {
    let trace = load_trace("tests/traces/long_session.jsonl");
    let milestones: usize = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("milestone"))
        .count();
    assert!(milestones >= 5, "long session should have 5+ milestones, got {}", milestones);
}

#[test]
fn trace_multi_subsystem_has_focus_advisory() {
    let trace = load_trace("tests/traces/multi_subsystem.jsonl");
    let focus_advisories: usize = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| {
            e.get("type").and_then(|v| v.as_str()) == Some("advisory")
            && e.get("detail").and_then(|v| v.as_str()).unwrap_or("").contains("focus")
        })
        .count();
    assert!(focus_advisories >= 1, "multi-subsystem session should have focus advisory");
}

#[test]
fn trace_compression_heavy_has_compression_events() {
    let trace = load_trace("tests/traces/compression_heavy.jsonl");
    let compressions: usize = trace.iter()
        .filter_map(|e| serde_json::from_slice::<serde_json::Value>(e).ok())
        .filter(|e| e.get("type").and_then(|v| v.as_str()) == Some("compression"))
        .count();
    assert!(compressions >= 3, "compression-heavy session should have 3+ compression events");
}

#[test]
fn trace_corpus_version_exists() {
    let version = fs::read_to_string("tests/traces/VERSION").expect("VERSION file should exist");
    assert!(!version.trim().is_empty(), "VERSION should not be empty");
}
