// ─── common::events — typed session event log ───────────────────────────────
//
// Structured event types for all significant session occurrences.
// Events are written to redb (when wired) and session-notes.jsonl.
// Enables replay engine, analytics, and audit trail.

use serde::{Deserialize, Serialize};

/// All possible session events with typed payloads
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum SessionEvent {
    RuleFired {
        rule_id: String,
        decision: String, // "deny", "advisory", "shadow"
        cmd: String,
    },
    ToolRedirected {
        from: String,
        to: String,
    },
    OutputTrimmed {
        cmd: String,
        before_tokens: u64,
        after_tokens: u64,
    },
    AdvisoryEmitted {
        source: String,
        message: String,
        confidence: f32,
    },
    PhaseChanged {
        from: String,
        to: String,
        reason: String,
    },
    MilestoneReached {
        kind: String,
        detail: String,
    },
    ErrorDetected {
        kind: String,
        detail: String,
    },
    DeadEndRecorded {
        path: String,
        reason: String,
    },
    VerificationRun {
        cmd: String,
        success: bool,
    },
    GoalChanged {
        old: String,
        new: String,
    },
}

/// Write a typed event to the session log (JSONL format)
pub fn log_event(event: &SessionEvent) {
    if let Ok(json) = serde_json::to_string(event) {
        let event_type = match event {
            SessionEvent::RuleFired { .. } => "rule_fired",
            SessionEvent::ToolRedirected { .. } => "tool_redirected",
            SessionEvent::OutputTrimmed { .. } => "output_trimmed",
            SessionEvent::AdvisoryEmitted { .. } => "advisory_emitted",
            SessionEvent::PhaseChanged { .. } => "phase_changed",
            SessionEvent::MilestoneReached { .. } => "milestone_reached",
            SessionEvent::ErrorDetected { .. } => "error_detected",
            SessionEvent::DeadEndRecorded { .. } => "dead_end",
            SessionEvent::VerificationRun { .. } => "verification_run",
            SessionEvent::GoalChanged { .. } => "goal_changed",
        };
        super::add_session_note_ext(event_type, &json, None);
    }
}
