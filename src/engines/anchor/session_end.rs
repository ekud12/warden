// ─── Engine: Anchor — Session End ────────────────────────────────────────────
//
// SessionEnd handler. Logs session summary when Claude exits. Cannot inject
// context (session is over), but writes final stats for next session's
// SessionStart to pick up.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::engines::dream::lore as learning;
use crate::engines::dream::{dna, imprint as anomaly, pruner as effectiveness};
use crate::engines::harbor::proc_mgmt;
use crate::handlers::userprompt_context;

pub fn run(raw: &str) {
    let input = common::parse_input(raw);
    let reason = input
        .as_ref()
        .and_then(|i| i.reason.as_deref())
        .unwrap_or("unknown");

    let project_dir = common::project_dir();

    // Count session stats — prefer redb events, fall back to session-notes.jsonl
    let mut edits = 0u32;
    let mut errors = 0u32;
    let mut milestones = 0u32;

    let event_lines = read_session_events(&project_dir);
    // Find the last "session-end" entry — only count entries after it
    let start_idx = event_lines
        .iter()
        .rposition(|e| e.get("type").and_then(|v| v.as_str()) == Some("session-end"))
        .map(|i| i + 1)
        .unwrap_or(0);

    for entry in &event_lines[start_idx..] {
        let note_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match note_type {
            "edit" => edits += 1,
            "error" => errors += 1,
            "milestone" => milestones += 1,
            _ => {}
        }
    }

    // Read final session state for savings breakdown
    let state = common::read_session_state();
    let total_tokens = state.estimated_tokens_in + state.estimated_tokens_out;
    let savings_pct = if total_tokens > 0 {
        (state.estimated_tokens_saved * 100) / (total_tokens + state.estimated_tokens_saved)
    } else {
        0
    };

    // Build savings breakdown
    let mut savings_parts: Vec<String> = Vec::new();
    if state.savings_dedup > 0 {
        savings_parts.push(format!("{} dedup", state.savings_dedup));
    }
    if state.savings_deny > 0 {
        savings_parts.push(format!("{} deny", state.savings_deny));
    }
    if state.savings_build_skip > 0 {
        savings_parts.push(format!("{} build-skip", state.savings_build_skip));
    }
    if state.savings_truncation > 0 {
        savings_parts.push(format!("{} truncation", state.savings_truncation));
    }

    // Write session-end entry to session-notes.jsonl
    let mut detail = format!(
        "reason={} edits={} errors={} milestones={}",
        reason, edits, errors, milestones
    );
    if state.estimated_tokens_saved > 0 {
        detail.push_str(&format!(
            " saved=~{}K ({}%) via {}",
            state.estimated_tokens_saved / 1000,
            savings_pct,
            savings_parts.join(", ")
        ));
    }
    // Write structured session summary (for export/offline analysis)
    let quality_score = compute_session_summary(edits, errors, milestones, &state);

    common::add_session_note("session-end", &detail);

    // Log summary (include savings if any)
    let log_msg = if state.estimated_tokens_saved > 0 {
        format!(
            "Session ended: reason={}, edits={}, errors={}, milestones={}, saved=~{}K ({}%) via {}",
            reason,
            edits,
            errors,
            milestones,
            state.estimated_tokens_saved / 1000,
            savings_pct,
            savings_parts.join(", ")
        )
    } else {
        format!(
            "Session ended: reason={}, edits={}, errors={}, milestones={}",
            reason, edits, errors, milestones
        )
    };
    common::log("session-end", &log_msg);

    // Record cross-project learning stats
    let project_name = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    learning::record_session(&project_name);

    // Update rule effectiveness scores
    {
        let project_dir = common::project_dir();
        let mut eff = effectiveness::load(&project_dir);
        effectiveness::update_session_end(&mut eff, &state.rules_fired, quality_score);
        effectiveness::save(&project_dir, &eff);
    }

    // Update Dream intervention effectiveness at session end (normally async)
    crate::engines::dream::pruner::learn_effectiveness();

    // Update project DNA (anomaly baselines, quality scores)
    {
        let project_dir = common::project_dir();
        let mut stats = anomaly::load_stats(&project_dir);
        let avg_tokens = if state.turn > 0 {
            (state.estimated_tokens_in + state.estimated_tokens_out) / state.turn as u64
        } else {
            0
        };
        let edit_velocity = if state.turn > 0 {
            edits as f64 / state.turn as f64
        } else {
            0.0
        };
        let total_actions = (edits + state.files_read.len() as u32).max(1);
        let explore_ratio = state.explore_count as f64 / total_actions as f64;
        let denial_rate = state.recent_denial_turns.len() as f64 / state.turn.max(1) as f64;
        dna::update_stats(
            &mut stats,
            &dna::SessionMetrics {
                turns: state.turn,
                quality: quality_score,
                avg_tokens_per_turn: avg_tokens,
                errors,
                edit_velocity,
                explore_ratio,
                denial_rate,
            },
        );
        anomaly::save_stats(&project_dir, &stats);
    }

    // Generate auto-changelog for the session
    crate::handlers::auto_changelog::generate(&state);

    // ── Auto-scorecard (A.1) ──
    let sc = crate::scorecard::compute_from_redb();
    common::storage::write_json("stats", "last_scorecard", &sc);
    common::add_session_note_ext("scorecard", &format!("score={}", sc.overall_score), None);

    // ── Auto-archive (A.3) ──
    let summary = serde_json::json!({
        "ts": common::now_iso(),
        "turns": state.turn,
        "edits": edits,
        "errors": errors,
        "milestones": milestones,
        "quality": quality_score,
        "scorecard": sc.overall_score,
    });
    common::storage::write_json("stats", &format!("session_{}", common::now_iso()), &summary);

    // ── Auto-replay (Phase 2) — always run, store report for dream state ──
    {
        let events = common::storage::read_last_events(200);
        if events.len() >= 5 {
            let replay_report = crate::engines::harbor::replay::replay_through_rules(&events);
            common::storage::write_json("stats", "last_replay", &replay_report);
            common::log(
                "session-end",
                &crate::engines::harbor::replay::format_replay_report(&replay_report),
            );
        }
    }

    // Scorecard regression check (Warden repo gets logged warning)
    if is_warden_repo()
        && let Some(prev) =
            common::storage::read_json::<crate::scorecard::Scorecard>("stats", "prev_scorecard")
    {
        let delta = sc.overall_score as i32 - prev.overall_score as i32;
        if delta < -5 {
            common::log(
                "session-end",
                &format!(
                    "REGRESSION: scorecard dropped {} points ({} → {})",
                    -delta, prev.overall_score, sc.overall_score
                ),
            );
        }
    }
    common::storage::write_json("stats", "prev_scorecard", &sc);

    // Log session-end diagnostics (flight recorder)
    {
        let state = common::read_session_state();
        let trust = crate::engines::anchor::trust::compute_trust(&state);
        let focus = crate::engines::anchor::focus::compute_focus(&state);
        common::storage::append_diagnostic(
            "session_end",
            &format!(
                "turns={} errors={} trust={} focus={} milestone='{}'",
                state.turn,
                state.errors_unresolved,
                trust,
                focus.score,
                common::truncate(&state.last_milestone, 40)
            ),
        );
    }

    // Kill all managed processes
    proc_mgmt::kill_all();

    // Close redb storage (flush pending writes)
    common::storage::close();

    // Daemon stays alive across sessions (like Docker Desktop).
    // It only restarts on binary rebuild (mtime mismatch detection).
}

/// Compute and write a structured session summary to session-notes.jsonl.
/// Quality score: productivity(30%) + milestone_rate(30%) + (1-error_rate)(20%) + efficiency(20%)
fn compute_session_summary(
    edits: u32,
    errors: u32,
    milestones: u32,
    state: &common::SessionState,
) -> u32 {
    let turns = state.turn;
    if turns == 0 {
        return 0;
    }

    let total_tokens_in = state.estimated_tokens_in;
    let total_tokens_out = state.estimated_tokens_out;
    let total_tokens = total_tokens_in + total_tokens_out;
    let tokens_saved = state.estimated_tokens_saved;
    let savings_pct = if total_tokens + tokens_saved > 0 {
        (tokens_saved * 100) / (total_tokens + tokens_saved)
    } else {
        0
    };

    // Unique files from session state
    let unique_files_edited = state.files_edited.len() as u32;
    let unique_files_read = state.files_read.len() as u32;

    // Explore ratio: explore_count relative to total actions
    let total_actions = (edits + unique_files_read + milestones).max(1);
    let explore_ratio = state.explore_count as f64 / total_actions as f64;

    // Error slope from snapshots
    let slope = userprompt_context::error_slope(&state.turn_snapshots, state.turn_snapshots.len());

    // Avg tokens per turn
    let avg_tokens_per_turn = if turns > 0 {
        total_tokens / turns as u64
    } else {
        0
    };

    // Max errors unresolved (scan snapshots)
    let max_errors = state
        .turn_snapshots
        .iter()
        .map(|s| s.errors_unresolved)
        .max()
        .unwrap_or(state.errors_unresolved);

    // Total denials
    let total_denials: u32 = state
        .turn_snapshots
        .iter()
        .map(|s| s.denials_this_turn as u32)
        .sum();

    // Compactions (count from session state)
    let compactions = if state.last_compaction_turn > 0 {
        1u32
    } else {
        0
    };

    // Quality score (0-100)
    let productivity = if turns > 0 {
        (edits as f64 / turns as f64).min(1.0)
    } else {
        0.0
    };
    let milestone_rate = if turns > 0 {
        (milestones as f64 / turns as f64 * 5.0).min(1.0)
    } else {
        0.0
    };
    let error_rate = if turns > 0 {
        (errors as f64 / turns as f64).min(1.0)
    } else {
        0.0
    };
    let efficiency = (savings_pct as f64 / 100.0).min(1.0);

    let quality_score = (productivity * 30.0
        + milestone_rate * 30.0
        + (1.0 - error_rate) * 20.0
        + efficiency * 20.0)
        .round() as u32;

    let data = serde_json::json!({
        "duration_turns": turns,
        "edits": edits,
        "errors": errors,
        "milestones": milestones,
        "tokens_in": total_tokens_in,
        "tokens_out": total_tokens_out,
        "tokens_saved": tokens_saved,
        "savings_pct": savings_pct,
        "unique_files_edited": unique_files_edited,
        "unique_files_read": unique_files_read,
        "explore_ratio": (explore_ratio * 100.0).round() / 100.0,
        "error_slope": (slope * 100.0).round() / 100.0,
        "avg_tokens_per_turn": avg_tokens_per_turn,
        "max_errors_unresolved": max_errors,
        "total_denials": total_denials,
        "compactions": compactions,
        "quality_score": quality_score,
    });

    let detail = format!("quality={} turns={} edits={}", quality_score, turns, edits);
    common::add_session_note_ext("session-summary", &detail, Some(&data));
    quality_score
}

/// Read session events: redb primary, session-notes.jsonl fallback.
fn read_session_events(project_dir: &std::path::Path) -> Vec<serde_json::Value> {
    // Try redb first
    if common::storage::is_available() {
        let raw_events = common::storage::read_last_events(500);
        if !raw_events.is_empty() {
            return raw_events
                .iter()
                .filter_map(|e| serde_json::from_slice(e).ok())
                .collect();
        }
    }
    // Fallback: session-notes.jsonl
    let session_path = project_dir.join("session-notes.jsonl");
    if session_path.exists() {
        let tail = common::read_tail(&session_path, 10_240);
        return tail
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
    }
    Vec::new()
}

/// Detect if current project is the Warden repo itself (for auto-replay regression checks)
fn is_warden_repo() -> bool {
    std::fs::read_to_string("Cargo.toml")
        .map(|c| c.contains("name = \"warden\""))
        .unwrap_or(false)
}
