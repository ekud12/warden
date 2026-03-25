// ─── config_override — JSON-based config hot-reload ──────────────────────────

use crate::common;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize, Default, Debug)]
pub struct ConfigOverrides {
    #[serde(default)]
    pub safety: Vec<(String, String)>,
    #[serde(default)]
    pub substitutions: Vec<(String, String)>,
    #[serde(default)]
    pub advisories: Vec<(String, String)>,
    #[serde(default)]
    pub hallucination: Vec<(String, String)>,
    #[serde(default)]
    pub hallucination_advisory: Vec<(String, String)>,
    #[serde(default)]
    pub auto_allow: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub just_map: Vec<(String, String)>,
}

pub static OVERRIDES: LazyLock<ConfigOverrides> = LazyLock::new(|| {
    // Check both new name (overrides.json) and legacy name (warden-overrides.json)
    let dir = common::hooks_dir();
    let path = {
        let new_path = dir.join("overrides.json");
        if new_path.exists() {
            new_path
        } else {
            dir.join(format!("{}-overrides.json", crate::constants::NAME))
        }
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ConfigOverrides::default(), // missing file -> defaults
    }
});
