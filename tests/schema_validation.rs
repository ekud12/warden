/// Schema validation tests — ensure shipped schemas are valid JSON and cover known config keys.

#[test]
fn config_schema_is_valid_json() {
    let schema = include_str!("../schemas/config.schema.json");
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(schema);
    assert!(parsed.is_ok(), "config.schema.json is not valid JSON: {:?}", parsed.err());
}

#[test]
fn rules_schema_is_valid_json() {
    let schema = include_str!("../schemas/rules.schema.json");
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(schema);
    assert!(parsed.is_ok(), "rules.schema.json is not valid JSON: {:?}", parsed.err());
}

#[test]
fn config_schema_covers_known_sections() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/config.schema.json")).unwrap();

    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("assistant"), "Missing 'assistant' section");
    assert!(props.contains_key("telemetry"), "Missing 'telemetry' section");
    assert!(props.contains_key("tools"), "Missing 'tools' section");
    assert!(props.contains_key("restrictions"), "Missing 'restrictions' section");
}

#[test]
fn rules_schema_covers_all_categories() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/rules.schema.json")).unwrap();

    let props = schema["properties"].as_object().unwrap();
    let required_categories = [
        "safety", "destructive", "substitutions", "advisories",
        "hallucination", "hallucination_advisory",
        "sensitive_paths_deny", "sensitive_paths_warn",
        "auto_allow", "zero_trace", "just", "thresholds",
        "restrictions", "command_filters",
    ];

    for category in required_categories {
        assert!(
            props.contains_key(category),
            "Rules schema missing category: {}",
            category
        );
    }
}

#[test]
fn config_schema_telemetry_fields_match_parser() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/config.schema.json")).unwrap();

    let telemetry_props = schema["properties"]["telemetry"]["properties"]
        .as_object()
        .unwrap();

    // These fields must exist in the schema (they exist in parser.rs TelemetryConfig)
    let known_fields = [
        "anomaly_detection", "quality_predictor", "cost_tracking",
        "error_prevention", "token_forecast", "smart_truncation",
        "project_dna", "rule_effectiveness", "drift_velocity",
        "compaction_optimizer", "command_recovery",
    ];

    for field in known_fields {
        assert!(
            telemetry_props.contains_key(field),
            "Telemetry schema missing field: {}",
            field
        );
    }
}

#[test]
fn default_config_toml_is_valid() {
    // The default config template should be valid TOML
    let default_config = r#"
[assistant]
type = "auto"

[tools]

[restrictions]

[telemetry]
"#;
    let result: Result<toml::Value, _> = toml::from_str(default_config);
    assert!(result.is_ok(), "Default config template is not valid TOML");
}
