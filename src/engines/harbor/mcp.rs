// ─── Engine: Harbor — MCP ─────────────────────────────────────────────────────
//
// `warden mcp` runs as a stdio MCP server, making Warden bidirectional:
// the AI can ASK Warden for guidance, not just get passively filtered.
//
// Tools exposed:
//   - session_status: Current phase, quality, turn count, anomalies
//   - explain_denial: Why was the last command blocked?
//   - suggest_action: What should I do next based on session state?
//   - check_file: Is this file safe/advisable to edit?
//   - session_history: Last 20 events
//   - reset_context: Signal task pivot, clear goal
//
// Protocol: JSON-RPC 2.0 over stdio (MCP standard)
// ──────────────────────────────────────────────────────────────────────────────

use crate::analytics;
use crate::common;
use crate::constants;
use crate::engines::dream::imprint as anomaly;
use std::io::{self, BufRead, Write};

/// MCP server entry point — reads JSON-RPC from stdin, writes to stdout
pub fn run() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                write_error(&mut out, serde_json::Value::Null, -32700, "Parse error");
                continue;
            }
        };

        let id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        match method {
            "initialize" => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": { "listChanged": false }
                        },
                        "serverInfo": {
                            "name": constants::NAME,
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                });
                write_response(&mut out, &response);
            }

            "notifications/initialized" => {
                // No response needed for notifications
            }

            "tools/list" => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": tools_list()
                    }
                });
                write_response(&mut out, &response);
            }

            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or_default();
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or_default();

                let result = match tool_name {
                    "session_status" => tool_session_status(),
                    "explain_denial" => tool_explain_denial(),
                    "suggest_action" => tool_suggest_action(),
                    "check_file" => tool_check_file(&arguments),
                    "session_history" => tool_session_history(),
                    "reset_context" => tool_reset_context(),
                    _ => serde_json::json!({
                        "content": [{"type": "text", "text": format!("Unknown tool: {}", tool_name)}],
                        "isError": true
                    }),
                };

                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result
                });
                write_response(&mut out, &response);
            }

            _ => {
                write_error(
                    &mut out,
                    id,
                    -32601,
                    &format!("Method not found: {}", method),
                );
            }
        }
    }
}

fn tools_list() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "session_status",
            "description": "Get current session state: phase, quality score, turn count, anomaly alerts, token usage, and recent errors. Use this to understand the session health before making decisions.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "explain_denial",
            "description": "Explain why the most recent command was blocked by Warden. Shows the rule that fired, the category, and how to fix it.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "suggest_action",
            "description": "Get Warden's suggestion for what to do next based on session state, error patterns, and phase. Use when stuck or unsure of approach.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "check_file",
            "description": "Check if a file is safe and advisable to edit. Returns any known issues, co-change suggestions, and recent error history for that file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to check"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "session_history",
            "description": "Get recent session activity: last 20 events from session notes (edits, errors, milestones, denials).",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "reset_context",
            "description": "Signal a context pivot: clears session goal, action history, and working set. Use when the user has changed tasks mid-conversation.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }
    ])
}

fn tool_session_status() -> serde_json::Value {
    let state = common::read_session_state();
    let total_tokens = state.estimated_tokens_in + state.estimated_tokens_out;

    // Load project stats for anomaly context
    let project_dir = common::project_dir();
    let stats = anomaly::load_stats(&project_dir);
    let avg_quality = if stats.quality_score.n >= 3 {
        Some(stats.quality_score.mean as u32)
    } else {
        None
    };

    let phase = &state.adaptive.phase;
    let quality = analytics::quality::predict_quality(
        &state.turn_snapshots,
        state.turn,
        state.errors_unresolved,
        state.estimated_tokens_saved,
        total_tokens,
    );

    let mut status = format!(
        "Turn: {}\nPhase: {}\nErrors unresolved: {}\nFiles edited: {}\nFiles read: {}\nTokens in: ~{}K\nTokens out: ~{}K\nTokens saved: ~{}K",
        state.turn,
        phase,
        state.errors_unresolved,
        state.files_edited.len(),
        state.files_read.len(),
        state.estimated_tokens_in / 1000,
        state.estimated_tokens_out / 1000,
        state.estimated_tokens_saved / 1000,
    );

    if let Some(q) = quality {
        status.push_str(&format!("\nQuality score: {}/100", q.score));
        if let Some(avg) = avg_quality {
            status.push_str(&format!(" (project avg: {})", avg));
        }
    }

    // Anomaly alerts
    let last_snap = state.turn_snapshots.last();
    let tokens_this_turn = last_snap
        .map(|s| s.tokens_in_delta + s.tokens_out_delta)
        .unwrap_or(0);
    let anomalies = anomaly::check_anomalies(&stats, tokens_this_turn, 2.0);
    if !anomalies.is_empty() {
        status.push_str("\n\nAnomalies:\n");
        for a in &anomalies {
            status.push_str(&format!("- {}\n", a));
        }
    }

    // ─── Enriched fields ─────────────────────────────────────────────────

    // Trust-based advisory budget
    let trust = crate::engines::anchor::trust::compute_trust(&state);
    let advisory_budget = if trust > 85 {
        1
    } else if trust > 50 {
        3
    } else if trust > 25 {
        5
    } else {
        15
    };
    status.push_str(&format!(
        "\nTrust: {}\nAdvisory budget: {}",
        trust, advisory_budget
    ));

    // Session goal
    if !state.session_goal.is_empty() {
        status.push_str(&format!("\nSession goal: {}", state.session_goal));
    }

    // Last phase transition turn
    if let Some(last_transition) = state.adaptive.transitions.last() {
        status.push_str(&format!(
            "\nLast phase transition: turn {} ({} -> {}, {})",
            last_transition.turn, last_transition.from, last_transition.to, last_transition.reason
        ));
    }

    // Intervention effectiveness (top 3 categories from dream scores)
    let intervention_scores = crate::engines::dream::get_intervention_scores();
    if !intervention_scores.scores.is_empty() {
        let mut sorted: Vec<(&String, &f64)> = intervention_scores.scores.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        status.push_str("\nIntervention effectiveness (top 3):");
        for (cat, score) in sorted.iter().take(3) {
            status.push_str(&format!("\n  {}: {:.2}", cat, score));
        }
    }

    // Focus score (directories_touched vs subsystem_switches)
    if !state.directories_touched.is_empty() {
        let unique_dirs = state.directories_touched.len() as f64;
        let focus = if unique_dirs > 0.0 {
            (1.0 - (state.subsystem_switches as f64 / unique_dirs)).max(0.0)
        } else {
            1.0
        };
        status.push_str(&format!("\nFocus score: {:.2}", focus));
    }

    // Compaction forecast (if snapshots available)
    if state.turn_snapshots.len() >= 3 {
        let avg_tokens_per_turn = total_tokens / state.turn.max(1) as u64;
        if avg_tokens_per_turn > 0 {
            // Claude context ~ 200K tokens; estimate turns until ~80% fill
            let target = 160_000u64;
            if total_tokens < target {
                let remaining = target - total_tokens;
                let eta_turns = remaining / avg_tokens_per_turn;
                status.push_str(&format!(
                    "\nCompaction forecast: ~{} turns until 80% context",
                    eta_turns
                ));
            } else {
                status.push_str("\nCompaction forecast: context at/above 80%");
            }
        }
    }

    text_result(&status)
}

fn tool_explain_denial() -> serde_json::Value {
    let project_dir = common::project_dir();
    let log_path = project_dir.join("logs").join("pretool-bash.log");

    let content = match std::fs::read_to_string(&log_path) {
        Ok(c) => c,
        Err(_) => {
            return text_result("No denial log found. No commands have been blocked this session.");
        }
    };

    // Find the last DENY entry
    let last_deny = content.lines().rev().find(|line| line.contains("[DENY]"));

    match last_deny {
        Some(line) => text_result(&format!(
            "Last denial:\n{}\n\nTo avoid this denial, follow the suggestion in the message. Most denials are substitution rules (use rg instead of grep, fd instead of find) or safety rules (dangerous commands).",
            line
        )),
        None => text_result("No commands have been denied this session."),
    }
}

fn tool_suggest_action() -> serde_json::Value {
    let state = common::read_session_state();
    let mut suggestions = Vec::new();

    // Phase-based suggestions
    use crate::engines::anchor::compass::SessionPhase;
    match &state.adaptive.phase {
        SessionPhase::Warmup => suggestions.push("You're in warmup phase. Read relevant files and understand the codebase before editing."),
        SessionPhase::Exploring => suggestions.push("You've been exploring without editing. Consider committing to an approach and start implementing."),
        SessionPhase::Struggling => suggestions.push("Error rate is high. Step back and verify your approach. Run tests/build to check current state."),
        SessionPhase::Late => suggestions.push("Context is filling up. Minimize reads, use targeted edits, consider committing progress."),
        SessionPhase::Productive => {} // No special suggestion needed
    }

    // Error-based suggestions
    if state.errors_unresolved >= 3 {
        suggestions.push("Multiple unresolved errors. Fix existing errors before adding new code.");
    }

    // Build nudge
    let edits_since_build = state.turn.saturating_sub(state.last_build_turn);
    if edits_since_build >= 5 && !state.files_edited.is_empty() {
        suggestions.push("You've made several edits without building/testing. Run a build to catch issues early.");
    }

    // Uncommitted work
    if state.files_edited.len() >= 5 {
        suggestions.push("Consider a checkpoint commit — you have many edited files.");
    }

    if suggestions.is_empty() {
        suggestions.push("Session looks healthy. Continue with your current approach.");
    }

    text_result(&suggestions.join("\n\n"))
}

fn tool_check_file(arguments: &serde_json::Value) -> serde_json::Value {
    let path = arguments.get("path").and_then(|p| p.as_str()).unwrap_or("");
    if path.is_empty() {
        return text_result("Please provide a file path to check.");
    }

    let state = common::read_session_state();
    let mut info = Vec::new();

    // Check if already edited this session
    if state.files_edited.contains(&path.to_string()) {
        let edit_turn = state.last_edit_turn;
        info.push(format!("Edited this session (last edit: turn {})", edit_turn));
    }

    // Check if already read
    if let Some(entry) = state.files_read.get(path) {
        info.push(format!(
            "Read at turn {} (hash: {})",
            entry.turn, entry.hash
        ));
    }

    // Working set membership
    let dir = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if state.initial_working_set.contains(&dir) {
        info.push("In initial working set (core focus area)".to_string());
    } else if state.rolling_working_set.contains(&dir) {
        info.push("In rolling working set (recently active)".to_string());
    }

    // Syntax validation coverage
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "json" => info.push("Syntax: JSON — full parse validation on edit".to_string()),
        "toml" => info.push("Syntax: TOML — full parse validation on edit".to_string()),
        "yaml" | "yml" => {
            info.push("Syntax: YAML — lightweight structural checks on edit".to_string())
        }
        _ => {}
    }

    // Likely generated file heuristic
    let short = path
        .rsplit('/')
        .next()
        .or_else(|| path.rsplit('\\').next())
        .unwrap_or(path);
    let generated = [
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "Cargo.lock",
        "Gemfile.lock",
        "poetry.lock",
        "composer.lock",
    ];
    let generated_ext = [".min.js", ".min.css", ".map", ".d.ts"];
    if generated.contains(&short)
        || generated_ext.iter().any(|e| short.ends_with(e))
    {
        info.push(format!(
            "GENERATED: {} is likely a generated/lock file. Edits may be overwritten.",
            short
        ));
    }

    // Check sensitive paths
    let sensitive = [
        ".env",
        "credentials",
        "secrets",
        "id_rsa",
        "id_ed25519",
        ".pem",
        ".key",
    ];
    if sensitive.iter().any(|s| short.contains(s)) {
        info.push(format!(
            "SENSITIVE: {} matches a sensitive file pattern. Edit with caution.",
            short
        ));
    }

    // Recent errors mentioning this file — try redb first, fall back to JSONL
    {
        let events = if common::storage::is_available() {
            common::storage::read_last_events(100)
                .iter()
                .filter_map(|e| String::from_utf8(e.clone()).ok())
                .collect::<Vec<_>>()
        } else {
            let session_path = common::project_dir().join("session-notes.jsonl");
            std::fs::read_to_string(&session_path)
                .unwrap_or_default()
                .lines()
                .rev()
                .take(100)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        };
        let error_count = events
            .iter()
            .filter(|line| line.contains("\"error\"") && line.contains(short))
            .count();
        if error_count > 0 {
            info.push(format!(
                "Recent errors: {} error(s) in session notes mention this file",
                error_count
            ));
        }
    }

    // Check if file exists
    if !std::path::Path::new(path).exists() {
        info.push(format!(
            "File does not exist: {}. Will be created on write.",
            path
        ));
    }

    if info.is_empty() {
        info.push(format!("No known issues with {}. Safe to edit.", path));
    }

    text_result(&info.join("\n"))
}

fn tool_session_history() -> serde_json::Value {
    // Try redb events first, fall back to session-notes.jsonl
    let entries: Vec<serde_json::Value> = if common::storage::is_available() {
        let raw = common::storage::read_last_events(20);
        if !raw.is_empty() {
            raw.iter()
                .filter_map(|e| serde_json::from_slice(e).ok())
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let entries = if entries.is_empty() {
        // Fallback: session-notes.jsonl
        let session_path = common::project_dir().join("session-notes.jsonl");
        match std::fs::read_to_string(&session_path) {
            Ok(c) => {
                let lines: Vec<&str> = c.lines().collect();
                let recent = if lines.len() > 20 {
                    &lines[lines.len() - 20..]
                } else {
                    &lines
                };
                recent
                    .iter()
                    .filter_map(|line| serde_json::from_str(line).ok())
                    .collect()
            }
            Err(_) => return text_result("No session history available."),
        }
    } else {
        entries
    };

    if entries.is_empty() {
        return text_result("No session history available.");
    }

    let mut history = String::from("Recent session events:\n");
    for entry in &entries {
        let note_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("?");
        let detail = entry.get("detail").and_then(|v| v.as_str()).unwrap_or("");
        let ts = entry.get("ts").and_then(|v| v.as_str()).unwrap_or("");
        history.push_str(&format!("[{}] {} — {}\n", note_type, detail, ts));
    }

    text_result(&history)
}

fn tool_reset_context() -> serde_json::Value {
    let mut state = common::read_session_state();
    state.session_goal.clear();
    state.action_history.clear();
    state.action_transitions.clear();
    state.initial_working_set = state.rolling_working_set.clone();
    state.context_switch_detected = true;
    state.last_initial_set_touch_turn = state.turn;
    common::write_session_state(&state);
    common::log("mcp", "Context reset by AI");
    text_result("Context reset. Session goal cleared, working set updated to recent directories.")
}

fn text_result(text: &str) -> serde_json::Value {
    serde_json::json!({
        "content": [{"type": "text", "text": text}]
    })
}

fn write_response(out: &mut impl Write, response: &serde_json::Value) {
    let json = serde_json::to_string(response).unwrap_or_default();
    let _ = writeln!(out, "{}", json);
    let _ = out.flush();
}

fn write_error(out: &mut impl Write, id: serde_json::Value, code: i32, message: &str) {
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    });
    write_response(out, &response);
}
