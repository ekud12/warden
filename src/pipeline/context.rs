// ─── pipeline::context — shared state flowing through the middleware pipeline ─

use std::time::Duration;

/// Shared mutable context passed through all pipeline stages
pub struct PipelineContext {
    /// Parsed hook input (tool name, tool input, session info)
    pub tool_name: String,
    pub tool_input: Option<serde_json::Value>,
    pub tool_output: Option<String>,
    pub command: Option<String>,
    pub exit_code: Option<i64>,

    /// Accumulated advisory messages (injected as additionalContext)
    pub advisories: Vec<String>,

    /// Final pipeline decision (set by deny/allow stages)
    pub decision: Option<super::Decision>,

    /// Per-stage timing for profiling
    pub timings: Vec<(&'static str, Duration)>,

    /// Error log (stage panics, config issues)
    pub errors: Vec<String>,
}

impl PipelineContext {
    pub fn new(tool_name: String) -> Self {
        Self {
            tool_name,
            tool_input: None,
            tool_output: None,
            command: None,
            exit_code: None,
            advisories: Vec::with_capacity(4),
            decision: None,
            timings: Vec::with_capacity(12),
            errors: Vec::new(),
        }
    }

    /// Log an error (stage panic, config issue). Collected but never blocks pipeline.
    pub fn log_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    /// Create a minimal context for testing
    #[cfg(test)]
    pub fn test_default() -> Self {
        Self::new("Bash".to_string())
    }
}
