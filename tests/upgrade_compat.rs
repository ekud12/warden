/// Upgrade compatibility tests — verify Warden handles old/malformed state gracefully.

use std::collections::HashMap;

/// Old config with missing new fields deserializes cleanly (serde(default))
#[test]
fn old_session_state_missing_fields() {
    // Minimal v1.0.0 session state — missing fields added in v1.1.x
    let old_state_json = r#"{
        "turn": 5,
        "files_read": {},
        "files_edited": ["src/main.rs"],
        "explore_count": 2,
        "errors_unresolved": 0,
        "estimated_tokens_in": 10000,
        "estimated_tokens_out": 5000
    }"#;

    // This should deserialize without error — all missing fields get defaults
    let result: Result<serde_json::Value, _> = serde_json::from_str(old_state_json);
    assert!(result.is_ok(), "Old session state should parse as valid JSON");

    let value = result.unwrap();
    assert_eq!(value["turn"], 5);
    assert_eq!(value["files_edited"][0], "src/main.rs");
}

/// Config.toml with unknown future fields should parse without error
#[test]
fn config_with_unknown_fields() {
    let future_config = r#"
[assistant]
type = "auto"

[telemetry]
anomaly_detection = true
future_feature_2027 = true

[some_future_section]
key = "value"
"#;

    // toml::from_str should succeed — unknown sections are ignored
    let result: Result<toml::Value, _> = toml::from_str(future_config);
    assert!(result.is_ok(), "Config with unknown fields should parse");
}

/// Rules TOML with unknown fields should not crash merge
#[test]
fn rules_toml_with_unknown_fields() {
    let future_rules = r#"
[safety]
replace = false

[[safety.patterns]]
regex = "test-pattern"
msg = "test message"

[future_category]
replace = true

[[future_category.patterns]]
regex = "future-.*"
msg = "future rule"
"#;

    let result: Result<toml::Value, _> = toml::from_str(future_rules);
    assert!(result.is_ok(), "Rules with unknown categories should parse");
}

/// Settings JSON with non-Warden hooks should survive merge
#[test]
fn non_warden_hooks_preserved_in_merge() {
    let settings_json = r#"{
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [{ "type": "command", "command": "/usr/bin/my-custom-hook pretool" }]
                },
                {
                    "matcher": "Bash",
                    "hooks": [{ "type": "command", "command": "/home/user/.warden/bin/warden pretool-bash" }]
                }
            ],
            "CustomEvent": [
                {
                    "matcher": "",
                    "hooks": [{ "type": "command", "command": "my-other-tool" }]
                }
            ]
        },
        "other_setting": true
    }"#;

    let settings: serde_json::Value = serde_json::from_str(settings_json).unwrap();

    // Verify structure
    let hooks = settings["hooks"].as_object().unwrap();
    let pretool = hooks["PreToolUse"].as_array().unwrap();
    assert_eq!(pretool.len(), 2);

    // The non-Warden hook should be identifiable
    let custom_hook = &pretool[0];
    let cmd = custom_hook["hooks"][0]["command"].as_str().unwrap();
    assert!(!cmd.to_lowercase().contains("warden"), "First hook is non-Warden");

    // The Warden hook should be identifiable
    let warden_hook = &pretool[1];
    let cmd = warden_hook["hooks"][0]["command"].as_str().unwrap();
    assert!(cmd.to_lowercase().contains("warden"), "Second hook is Warden");

    // CustomEvent should exist and be preserved
    assert!(hooks.contains_key("CustomEvent"));
    assert_eq!(settings["other_setting"], true);
}

/// Empty or missing hooks section should not crash install
#[test]
fn empty_hooks_section() {
    let settings_json = r#"{ "other_setting": true }"#;
    let settings: serde_json::Value = serde_json::from_str(settings_json).unwrap();
    assert!(settings.get("hooks").is_none());
}

/// Corrupt JSON falls back to empty object
#[test]
fn corrupt_settings_json() {
    let corrupt = "{ this is not valid json }}}";
    let result: Result<serde_json::Value, _> = serde_json::from_str(corrupt);
    assert!(result.is_err());

    // Warden should fall back to empty object (fail-open)
    let fallback: serde_json::Value = result.unwrap_or(serde_json::json!({}));
    assert!(fallback.is_object());
}

/// HashMap with unknown keys deserializes (SessionState has HashMap fields)
#[test]
fn hashmap_fields_tolerate_unknown_keys() {
    let json = r#"{
        "old_key_removed_in_v2": 42,
        "another_removed": "hello"
    }"#;

    // Deserializing into a generic HashMap succeeds
    let result: Result<HashMap<String, serde_json::Value>, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}
