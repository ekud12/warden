// ─── Bridge — External tool integrations ─────────────────────────────────────
//
// Bridges connect Warden to external systems. Currently implemented:
//   - Webhook: HTTP POST on deny/milestone/phase-change events
//
// Planned (config schema defined, implementation pending):
//   - LangChain / LangGraph callback handlers
//   - CrewAI agent hooks
//   - AutoGen integration
// ──────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

/// Webhook bridge configuration (from rules.toml)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WebhookConfig {
    /// URL to POST events to (empty = disabled)
    pub url: String,
    /// Event types to send: "deny", "milestone", "phase_change", "error"
    pub events: Vec<String>,
    /// Optional auth header value (e.g., "Bearer token123")
    pub auth_header: String,
    /// Timeout in milliseconds for HTTP POST (default: 2000)
    pub timeout_ms: u64,
}

/// Bridge configuration for planned integrations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BridgeConfig {
    pub webhook: WebhookConfig,
    // Planned — config schema only, implementation pending
    pub langchain: PlannedBridge,
    pub crewai: PlannedBridge,
    pub autogen: PlannedBridge,
}

/// Placeholder config for planned bridges
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PlannedBridge {
    pub enabled: bool,
}

/// Fire a webhook event (non-blocking, fire-and-forget).
/// Spawns a thread so the hook pipeline is never blocked by HTTP latency.
pub fn fire_webhook(event_type: &str, payload: &serde_json::Value) {
    let config = &crate::rules::RULES.bridge.webhook;
    if config.url.is_empty() {
        return;
    }
    if !config.events.iter().any(|e| e == event_type || e == "*") {
        return;
    }

    let url = config.url.clone();
    let auth = config.auth_header.clone();
    let timeout_ms = if config.timeout_ms > 0 {
        config.timeout_ms
    } else {
        2000
    };
    let body = serde_json::json!({
        "event": event_type,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "data": payload,
    })
    .to_string();

    // Fire-and-forget: spawn thread for HTTP POST
    std::thread::spawn(move || {
        post_webhook(&url, &auth, &body, timeout_ms);
    });
}

/// HTTP POST using raw TCP (no external HTTP crate needed).
/// Best-effort: failures are logged silently.
fn post_webhook(url: &str, auth: &str, body: &str, _timeout_ms: u64) {
    // Parse URL to extract host and path
    let url_trimmed = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let (host, path) = match url_trimmed.find('/') {
        Some(i) => (&url_trimmed[..i], &url_trimmed[i..]),
        None => (url_trimmed, "/"),
    };

    // Use subprocess for HTTP (xh or curl) since we don't have an HTTP crate
    let mut args = vec![
        "POST".to_string(),
        url.to_string(),
        format!("Content-Type:application/json"),
    ];
    if !auth.is_empty() {
        args.push(format!("Authorization:{}", auth));
    }

    // Try xh first (warden's preferred HTTP tool), fall back to curl
    let result = std::process::Command::new("xh")
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(body.as_bytes());
            }
            child.wait()
        });

    if result.is_err() {
        // Fallback to curl
        let _ = std::process::Command::new("curl")
            .args([
                "-s",
                "-X",
                "POST",
                "-H",
                "Content-Type: application/json",
                "-d",
                body,
                url,
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    crate::common::log("bridge", &format!("Webhook fired: {} → {}", host, path));
}

/// Check if a planned bridge is configured but not yet available.
/// Returns a user-friendly message if so.
pub fn check_planned_bridges(config: &BridgeConfig) -> Option<String> {
    let mut planned = Vec::new();
    if config.langchain.enabled {
        planned.push("LangChain");
    }
    if config.crewai.enabled {
        planned.push("CrewAI");
    }
    if config.autogen.enabled {
        planned.push("AutoGen");
    }
    if planned.is_empty() {
        return None;
    }
    Some(format!(
        "Bridge(s) configured but not yet available: {}. Coming in a future release.",
        planned.join(", ")
    ))
}
