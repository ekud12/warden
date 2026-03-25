// ─── config::parser — runtime config.toml parser ─────────────────────────────
//
// Parses ~/.warden/config.toml at startup. All fields have sensible defaults
// so missing file or missing fields are handled gracefully.

use serde::Deserialize;
use std::sync::LazyLock;

fn default_true() -> bool {
    true
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct WardenConfig {
    pub assistant: AssistantConfig,
    pub telemetry: TelemetryConfig,
    pub tools: toml::Table,
    pub restrictions: RestrictionsConfig,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct AssistantConfig {
    #[serde(rename = "type", default = "default_auto")]
    pub assistant_type: String,
}

fn default_auto() -> String {
    "auto".to_string()
}

#[derive(Deserialize)]
#[serde(default)]
pub struct TelemetryConfig {
    #[serde(default = "default_true")]
    pub anomaly_detection: bool,
    #[serde(default = "default_true")]
    pub quality_predictor: bool,
    #[serde(default = "default_true")]
    pub cost_tracking: bool,
    #[serde(default = "default_true")]
    pub error_prevention: bool,
    #[serde(default = "default_true")]
    pub token_forecast: bool,
    #[serde(default = "default_true")]
    pub smart_truncation: bool,
    #[serde(default = "default_true")]
    pub project_dna: bool,
    #[serde(default = "default_true")]
    pub rule_effectiveness: bool,
    #[serde(default = "default_true")]
    pub drift_velocity: bool,
    #[serde(default = "default_true")]
    pub compaction_optimizer: bool,
    #[serde(default = "default_true")]
    pub command_recovery: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            anomaly_detection: true,
            quality_predictor: true,
            cost_tracking: true,
            error_prevention: true,
            token_forecast: true,
            smart_truncation: true,
            project_dna: true,
            rule_effectiveness: true,
            drift_velocity: true,
            compaction_optimizer: true,
            command_recovery: true,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct RestrictionsConfig {
    pub disabled: Vec<String>,
}

/// Global parsed config — loaded once from ~/.warden/config.toml
pub static CONFIG: LazyLock<WardenConfig> = LazyLock::new(|| {
    let path = crate::install::home_dir().join(crate::constants::CONFIG_FILE);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
});
