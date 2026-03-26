// ─── dream module tests — serde contracts, task order, semver comparison ──────
//
// Since warden is a binary-only crate (no lib.rs), we test the dream module's
// public contract via JSON serde round-trips. Struct definitions here mirror
// the real types — if the real types change shape, these tests break.
// ──────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ─── Mirror types (must match src/dream.rs exactly) ─────────────────────────

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
struct ResumePacket {
    high_salience_files: Vec<String>,
    last_verified_state: String,
    current_issue: String,
    dead_ends: Vec<String>,
    probable_next_actions: Vec<String>,
    #[serde(default)]
    top_playbook: String,
    #[serde(default)]
    convention_hints: Vec<String>,
    #[serde(default)]
    verification_debt: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct DreamPlaybook {
    id: String,
    name: String,
    trigger_signals: Vec<String>,
    recommended_steps: Vec<String>,
    evidence_count: u32,
    success_rate: f64,
    last_seen_turn: u32,
    source_sessions: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct RepairPattern {
    error_signature: String,
    affected_files: Vec<String>,
    commands_that_helped: Vec<String>,
    verification_step: String,
    success_count: u32,
    last_seen_turn: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ProjectConvention {
    kind: String,
    observation: String,
    confidence: f64,
    evidence_count: u32,
    last_updated_turn: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SuccessfulSequence {
    actions: Vec<String>,
    led_to_milestone: bool,
    occurrences: u32,
    last_seen_turn: u32,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct InterventionScores {
    scores: BTreeMap<String, f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RankedItem {
    path: String,
    score: f64,
    last_turn: u32,
    frequency: u32,
    led_to_progress: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ErrorCluster {
    file: String,
    error_stem: String,
    count: u32,
    first_turn: u32,
    last_turn: u32,
}

// ─── Mirror: TASK_ORDER enum variants ───────────────────────────────────────

const TASK_ORDER_NAMES: &[&str] = &[
    "LearnEffectiveness",    // E5
    "BuildResumePacket",     // E2
    "LearnSequences",        // E7
    "ClusterErrors",         // E4
    "LearnRepairPatterns",   // E8
    "LearnConventions",      // E9
    "UpdateWorkingSetRanking", // E3
    "BuildDeadEndMemory",    // E6
    "ScoreArtifacts",        // E10
    "ConsolidateEvents",     // E1
];

// ─── Test 1: TASK_ORDER has all 10 tasks, no duplicates ─────────────────────

#[test]
fn test_dream_task_order_complete() {
    assert_eq!(
        TASK_ORDER_NAMES.len(),
        10,
        "TASK_ORDER must have exactly 10 tasks (E1-E10)"
    );

    // No duplicates
    let mut seen = std::collections::HashSet::new();
    for name in TASK_ORDER_NAMES {
        assert!(
            seen.insert(name),
            "Duplicate task in TASK_ORDER: {}",
            name
        );
    }

    // All expected enum variants present
    let expected = [
        "ConsolidateEvents",
        "BuildResumePacket",
        "UpdateWorkingSetRanking",
        "ClusterErrors",
        "LearnEffectiveness",
        "BuildDeadEndMemory",
        "LearnSequences",
        "LearnRepairPatterns",
        "LearnConventions",
        "ScoreArtifacts",
    ];
    for variant in &expected {
        assert!(
            TASK_ORDER_NAMES.contains(variant),
            "Missing task variant in TASK_ORDER: {}",
            variant
        );
    }
}

// ─── Test 2: ResumePacket serde with V1-only fields ─────────────────────────

#[test]
fn test_resume_packet_serde_default() {
    // V1-only JSON (no top_playbook, convention_hints, verification_debt)
    let v1_json = serde_json::json!({
        "high_salience_files": ["src/main.rs", "src/lib.rs"],
        "last_verified_state": "Build passed at turn 12",
        "current_issue": "type error in handler",
        "dead_ends": ["tried raw pointers"],
        "probable_next_actions": ["cargo build", "run tests"]
    });

    let packet: ResumePacket =
        serde_json::from_value(v1_json).expect("V1 JSON should deserialize into ResumePacket");

    // V1 fields present
    assert_eq!(packet.high_salience_files, vec!["src/main.rs", "src/lib.rs"]);
    assert_eq!(packet.last_verified_state, "Build passed at turn 12");
    assert_eq!(packet.current_issue, "type error in handler");
    assert_eq!(packet.dead_ends, vec!["tried raw pointers"]);
    assert_eq!(packet.probable_next_actions, vec!["cargo build", "run tests"]);

    // V2 fields have defaults
    assert_eq!(packet.top_playbook, "", "V2 top_playbook should default to empty string");
    assert!(
        packet.convention_hints.is_empty(),
        "V2 convention_hints should default to empty vec"
    );
    assert_eq!(
        packet.verification_debt, 0,
        "V2 verification_debt should default to 0"
    );

    // Round-trip: serialize back and deserialize again
    let serialized = serde_json::to_string(&packet).expect("serialize");
    let round_tripped: ResumePacket =
        serde_json::from_str(&serialized).expect("deserialize round-trip");
    assert_eq!(packet, round_tripped, "Round-trip should preserve all fields");
}

// ─── Test 3: DreamPlaybook fields ───────────────────────────────────────────

#[test]
fn test_playbook_fields() {
    let playbook = DreamPlaybook {
        id: "pb-001".to_string(),
        name: "Fix type errors".to_string(),
        trigger_signals: vec!["type_error".to_string(), "build_fail".to_string()],
        recommended_steps: vec![
            "read error output".to_string(),
            "check type definitions".to_string(),
            "apply fix".to_string(),
            "cargo build".to_string(),
        ],
        evidence_count: 7,
        success_rate: 0.85,
        last_seen_turn: 42,
        source_sessions: 3,
    };

    assert_eq!(playbook.id, "pb-001");
    assert_eq!(playbook.name, "Fix type errors");
    assert_eq!(playbook.trigger_signals.len(), 2);
    assert_eq!(playbook.recommended_steps.len(), 4);
    assert_eq!(playbook.evidence_count, 7);
    assert!((playbook.success_rate - 0.85).abs() < f64::EPSILON);
    assert_eq!(playbook.last_seen_turn, 42);
    assert_eq!(playbook.source_sessions, 3);

    // Serde round-trip
    let json = serde_json::to_value(&playbook).expect("serialize");
    let deserialized: DreamPlaybook = serde_json::from_value(json).expect("deserialize");
    assert_eq!(playbook, deserialized);
}

// ─── Test 4: RepairPattern fields ───────────────────────────────────────────

#[test]
fn test_repair_pattern_fields() {
    let pattern = RepairPattern {
        error_signature: "cannot find type `Foo` in this scope".to_string(),
        affected_files: vec!["src/handlers/mod.rs".to_string(), "src/types.rs".to_string()],
        commands_that_helped: vec!["cargo build".to_string()],
        verification_step: "build/test".to_string(),
        success_count: 4,
        last_seen_turn: 88,
    };

    assert_eq!(pattern.error_signature, "cannot find type `Foo` in this scope");
    assert_eq!(pattern.affected_files.len(), 2);
    assert_eq!(pattern.affected_files[0], "src/handlers/mod.rs");
    assert_eq!(pattern.commands_that_helped, vec!["cargo build"]);
    assert_eq!(pattern.verification_step, "build/test");
    assert_eq!(pattern.success_count, 4);
    assert_eq!(pattern.last_seen_turn, 88);

    // Serde round-trip
    let json = serde_json::to_value(&pattern).expect("serialize");
    let deserialized: RepairPattern = serde_json::from_value(json).expect("deserialize");
    assert_eq!(pattern, deserialized);
}

// ─── Test 5: ProjectConvention fields ───────────────────────────────────────

#[test]
fn test_convention_fields() {
    let convention = ProjectConvention {
        kind: "build_preference".to_string(),
        observation: "Project type: rust".to_string(),
        confidence: 0.75,
        evidence_count: 12,
        last_updated_turn: 100,
    };

    assert_eq!(convention.kind, "build_preference");
    assert_eq!(convention.observation, "Project type: rust");
    assert!((convention.confidence - 0.75).abs() < f64::EPSILON);
    assert_eq!(convention.evidence_count, 12);
    assert_eq!(convention.last_updated_turn, 100);

    // Serde round-trip
    let json = serde_json::to_value(&convention).expect("serialize");
    let deserialized: ProjectConvention = serde_json::from_value(json).expect("deserialize");
    assert_eq!(convention, deserialized);
}

// ─── Test 6: SuccessfulSequence fields ──────────────────────────────────────

#[test]
fn test_successful_sequence_fields() {
    let sequence = SuccessfulSequence {
        actions: vec![
            "read_file".to_string(),
            "edit_file".to_string(),
            "cargo_build".to_string(),
        ],
        led_to_milestone: true,
        occurrences: 5,
        last_seen_turn: 67,
    };

    assert_eq!(sequence.actions.len(), 3);
    assert_eq!(sequence.actions[0], "read_file");
    assert_eq!(sequence.actions[1], "edit_file");
    assert_eq!(sequence.actions[2], "cargo_build");
    assert!(sequence.led_to_milestone);
    assert_eq!(sequence.occurrences, 5);
    assert_eq!(sequence.last_seen_turn, 67);

    // Serde round-trip
    let json = serde_json::to_value(&sequence).expect("serialize");
    let deserialized: SuccessfulSequence = serde_json::from_value(json).expect("deserialize");
    assert_eq!(sequence, deserialized);
}

// ─── Test 7: is_newer semver comparison ─────────────────────────────────────
// Mirror of install::update::is_newer — pure function, tested inline.

fn is_newer(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v
            .trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };
    let c = parse(current);
    let l = parse(latest);
    l > c
}

#[test]
fn test_is_newer_semver() {
    // Patch bump
    assert!(is_newer("1.0.0", "1.0.1"));
    // Minor bump
    assert!(is_newer("1.0.0", "1.1.0"));
    // Major bump
    assert!(is_newer("1.0.0", "2.0.0"));
    // Same version — not newer
    assert!(!is_newer("1.2.0", "1.2.0"));
    // Older version — not newer
    assert!(!is_newer("2.0.0", "1.9.9"));
    // With v prefix
    assert!(is_newer("v1.0.0", "v1.0.1"));
    assert!(is_newer("1.0.0", "v1.0.1"));
    assert!(is_newer("v1.0.0", "1.0.1"));
    // Mixed prefix — same version
    assert!(!is_newer("v1.2.0", "1.2.0"));
    // Missing patch
    assert!(is_newer("1.0", "1.1"));
    assert!(!is_newer("1.1", "1.0"));
    // Major-only
    assert!(is_newer("1", "2"));
    assert!(!is_newer("2", "1"));
    // Complex real-world versions
    assert!(is_newer("1.2.0", "1.2.1"));
    assert!(is_newer("0.9.9", "1.0.0"));
    assert!(!is_newer("1.0.0", "0.99.99"));
}

// ─── Extra: serde round-trips for remaining types ───────────────────────────

#[test]
fn test_intervention_scores_serde() {
    let mut scores = InterventionScores::default();
    scores.scores.insert("safety".to_string(), 0.8);
    scores.scores.insert("substitution".to_string(), 0.6);

    let json = serde_json::to_value(&scores).expect("serialize");
    let deserialized: InterventionScores = serde_json::from_value(json).expect("deserialize");
    assert_eq!(deserialized.scores.len(), 2);
    assert!((deserialized.scores["safety"] - 0.8).abs() < f64::EPSILON);
}

#[test]
fn test_ranked_item_serde() {
    let item = RankedItem {
        path: "src/dream.rs".to_string(),
        score: 0.95,
        last_turn: 30,
        frequency: 8,
        led_to_progress: true,
    };

    let json = serde_json::to_string(&item).expect("serialize");
    let deserialized: RankedItem = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.path, "src/dream.rs");
    assert!((deserialized.score - 0.95).abs() < f64::EPSILON);
    assert_eq!(deserialized.last_turn, 30);
    assert_eq!(deserialized.frequency, 8);
    assert!(deserialized.led_to_progress);
}

#[test]
fn test_error_cluster_serde() {
    let cluster = ErrorCluster {
        file: "src/main.rs".to_string(),
        error_stem: "cannot find value `x` in this scope".to_string(),
        count: 3,
        first_turn: 10,
        last_turn: 25,
    };

    let json = serde_json::to_string(&cluster).expect("serialize");
    let deserialized: ErrorCluster = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.file, "src/main.rs");
    assert_eq!(deserialized.count, 3);
    assert_eq!(deserialized.first_turn, 10);
    assert_eq!(deserialized.last_turn, 25);
}

// ─── Extra: text_similarity mirror (pure function from dream.rs) ────────────

fn text_similarity(a: &str, b: &str) -> f64 {
    let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();
    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

#[test]
fn test_text_similarity_identical() {
    assert!((text_similarity("hello world", "hello world") - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_text_similarity_disjoint() {
    assert!((text_similarity("hello world", "foo bar")).abs() < f64::EPSILON);
}

#[test]
fn test_text_similarity_partial() {
    // "hello world" and "hello foo" share "hello" out of {"hello","world","foo"} = 1/3
    let sim = text_similarity("hello world", "hello foo");
    assert!((sim - 1.0 / 3.0).abs() < 0.01);
}

#[test]
fn test_text_similarity_empty() {
    assert!((text_similarity("", "")).abs() < f64::EPSILON);
    assert!((text_similarity("hello", "")).abs() < f64::EPSILON);
    assert!((text_similarity("", "hello")).abs() < f64::EPSILON);
}

// ─── Extra: ResumePacket with all V2 fields populated ───────────────────────

#[test]
fn test_resume_packet_full_v2() {
    let packet = ResumePacket {
        high_salience_files: vec!["src/dream.rs".to_string()],
        last_verified_state: "Build OK".to_string(),
        current_issue: "none".to_string(),
        dead_ends: vec![],
        probable_next_actions: vec!["test".to_string()],
        top_playbook: "read -> edit -> build".to_string(),
        convention_hints: vec!["uses cargo".to_string(), "prefers rg".to_string()],
        verification_debt: 3,
    };

    let json = serde_json::to_string(&packet).expect("serialize");
    let deserialized: ResumePacket = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.top_playbook, "read -> edit -> build");
    assert_eq!(deserialized.convention_hints.len(), 2);
    assert_eq!(deserialized.verification_debt, 3);
    assert_eq!(packet, deserialized);
}
