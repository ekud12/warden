// ─── userprompt_context — per-turn context grounding ────────────────────────
//
// UserPromptSubmit handler. Runs before Claude processes each user message.
// Responsibilities:
//   1. Increment turn counter in session-state.json
//   2. Inject lightweight context (recent errors, edited files)
//   3. Git-aware grounding (branch + working tree summary)
//   4. Files-in-context reminder (reduces redundant re-reads)
//   5. Exploration budget advisory (>= 8 explores without editing)
//   6. Token budget threshold advisory (configurable, default 700K)
//   7. Late-session compactness mode (configurable turn thresholds)
//   8. Periodic rule re-injection (every N turns, default 30)
//   9. Drift detection warning (deny density exceeds threshold)
//
// Performance target: < 5ms (git subprocess only on cache miss ~50ms)
// ──────────────────────────────────────────────────────────────────────────────

use crate::analytics;
use crate::common;
use crate::handlers::adaptation;
use crate::handlers::git_summary;
use crate::handlers::token_budget;
use crate::rules;

pub fn run(_raw: &str) {
    // Increment turn counter in session state
    let mut state = common::read_session_state();
    state.turn += 1;

    // Goal extraction: on turn 1, try to extract session goal from user message
    if state.turn == 1 && state.session_goal.is_empty() {
        // Extract user prompt text from the raw input
        if let Some(input) = common::parse_input(_raw) {
            let text = input.tool_input.as_ref()
                .and_then(|v| v.get("message").or_else(|| v.get("content")).or_else(|| v.get("prompt")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(goal) = analytics::goal::extract_goal(text) {
                state.session_goal = goal;
                common::log("userprompt-context", &format!("Goal extracted: {}", &state.session_goal));
            }
        }
    }

    // Record snapshot for the turn that just ended (captures previous turn's outcomes)
    if state.turn > 1 {
        let snap = common::TurnSnapshot {
            turn: state.turn - 1,
            errors_unresolved: state.errors_unresolved,
            explore_count: state.explore_count,
            files_edited_count: state.files_edited.len().min(u16::MAX as usize) as u16,
            files_read_count: state.files_read.len().min(u16::MAX as usize) as u16,
            tokens_in_delta: state.estimated_tokens_in.saturating_sub(state.prev_snapshot_tokens_in),
            tokens_out_delta: state.estimated_tokens_out.saturating_sub(state.prev_snapshot_tokens_out),
            milestones_hit: state.last_edit_turn == state.turn - 1 && !state.last_milestone.is_empty(),
            edits_this_turn: state.last_edit_turn == state.turn - 1,
            denials_this_turn: state.denial_rate(1) as u8,
        };
        state.prev_snapshot_tokens_in = state.estimated_tokens_in;
        state.prev_snapshot_tokens_out = state.estimated_tokens_out;
        state.turn_snapshots.push(snap);
    }

    let adaptation_msg = adaptation::adapt(&mut state);

    let explore_count = state.explore_count;
    let turn = state.turn;

    let explore_budget: u32 = state.adaptive.params.explore_budget;

    // ── Context switch detection ──
    if !state.context_switch_detected && detect_context_switch(&state) {
        state.context_switch_detected = true;
        state.session_goal.clear();
        state.action_history.clear();
        state.action_transitions.clear();
        state.initial_working_set = state.rolling_working_set.clone();
        state.last_initial_set_touch_turn = state.turn;
        common::log("userprompt-context", &format!("Context switch detected at turn {}", turn));
    }

    // ── Predictive intelligence ──

    // Goal anchoring: re-inject session goal every 5 turns (skip after context switch)
    let goal_anchor = if !state.session_goal.is_empty()
        && !state.context_switch_detected
        && turn >= 5 && turn.is_multiple_of(5) {
            Some(analytics::goal::format_anchor(&state.session_goal, turn))
        } else { None };

    // Action entropy drift detection
    let entropy_advisory = if state.advisory_ready("entropy") {
        let has_recent_edits = state.last_edit_turn + 3 >= turn;
        analytics::entropy::check_drift(&state.action_history, has_recent_edits)
    } else { None };

    // Markov action prediction
    let markov_advisory = if state.action_history.len() >= 3 && state.advisory_ready("markov") {
        let current = state.action_history.last().map(|s| s.as_str()).unwrap_or("");
        analytics::markov::check_patterns(&state.action_transitions, current, &state.action_history)
    } else { None };

    // Topic coherence check (every 10 turns after turn 10)
    let coherence_advisory = if turn >= 10 && turn.is_multiple_of(10)
        && !state.initial_working_set.is_empty()
        && state.advisory_ready("coherence") {
            let (sim, drifted) = analytics::goal::topic_coherence(
                &state.initial_working_set, &state.files_edited,
            );
            if sim < 0.3 && !drifted.is_empty() {
                Some(format!(
                    "Focus seems to have shifted to {} — if intentional, this is fine.",
                    drifted.join(", ")
                ))
            } else { None }
        } else { None };

    // Exploration budget: advisory at adaptive threshold
    let explore_ready = if explore_count >= explore_budget {
        state.explore_count = 0;
        state.advisory_ready("explore")
    } else {
        false
    };

    // Git-aware grounding (cached unless edits happened)
    let git_line = git_summary::get_or_refresh(&mut state);

    // Token budget check (rate-limited)
    let token_advisory = if state.advisory_ready("token_budget") {
        token_budget::check_threshold(&state)
    } else {
        None
    };

    // Heuristic advisories from turn snapshots
    let heuristic_parts = heuristic_advisories(&mut state);

    // ── Runtime analytics (all fire automatically, opt-out via config) ──

    // Anomaly detection: check current turn against project baselines
    let anomaly_alerts = {
        let project_dir = common::project_dir();
        let stats = analytics::anomaly::load_stats(&project_dir);
        let last_snap = state.turn_snapshots.last();
        let tokens_this_turn = last_snap
            .map(|s| s.tokens_in_delta + s.tokens_out_delta)
            .unwrap_or(0);
        analytics::anomaly::check_anomalies(&stats, tokens_this_turn, 2.0)
    };

    // Token budget forecasting: predict compaction ETA
    let forecast_msg = analytics::forecast::predict_compaction(
        &state.turn_snapshots,
        state.turn,
        state.estimated_tokens_in + state.estimated_tokens_out,
        rules::RULES.token_budget_advisory,
    ).map(|f| analytics::forecast::format_forecast(&f))
     .filter(|s| !s.is_empty());

    // Quality prediction: fires at turn 10, then every 5 turns
    let quality_msg = analytics::quality::predict_quality(
        &state.turn_snapshots,
        state.turn,
        state.errors_unresolved,
        state.estimated_tokens_saved,
        state.estimated_tokens_in + state.estimated_tokens_out,
    ).map(|q| {
        let project_dir = common::project_dir();
        let stats = analytics::anomaly::load_stats(&project_dir);
        let avg = if stats.quality_score.n >= 3 { Some(stats.quality_score.mean as u32) } else { None };
        q.format(avg)
    }).filter(|s| !s.is_empty());

    // Error prevention: check current edit patterns against Bayesian priors
    let error_prevention_msg = {
        let project_dir = common::project_dir();
        let priors = analytics::error_prevention::load_priors(&project_dir);
        let edits_since_build = state.turn.saturating_sub(state.last_build_turn);
        let edited_dirs = state.files_edited.iter()
            .filter_map(|f| f.rsplit('/').nth(1))
            .collect::<std::collections::HashSet<_>>().len() as u32;
        let turns_since_test = state.turn.saturating_sub(state.last_build_turn);
        analytics::error_prevention::check_patterns(&priors, edits_since_build, edited_dirs, turns_since_test, 0.6)
    };

    // Git guardian: uncommitted changes duration check
    let git_uncommitted = crate::handlers::git_guardian::check_uncommitted_duration(&state);

    // Files-in-context: last 5 files read, sorted by turn (most recent first)
    let files_in_context = build_files_in_context(&state);

    // Read session-notes.jsonl for recent context
    let project_dir = common::project_dir();
    let session_path = project_dir.join("session-notes.jsonl");

    let mut parts = Vec::with_capacity(10);
    parts.extend(heuristic_parts);

    // Inject analytics advisories
    for alert in &anomaly_alerts {
        parts.push(alert.to_string());
    }
    if let Some(msg) = forecast_msg {
        parts.push(msg);
    }
    if let Some(msg) = quality_msg {
        parts.push(msg);
    }
    if let Some(msg) = error_prevention_msg {
        parts.push(msg);
    }
    if let Some(msg) = git_uncommitted {
        parts.push(msg);
    }
    if let Some(msg) = goal_anchor {
        parts.push(msg);
    }
    if let Some(msg) = entropy_advisory {
        parts.push(msg);
    }
    if let Some(msg) = markov_advisory {
        parts.push(msg);
    }
    if let Some(msg) = coherence_advisory {
        parts.push(msg);
    }

    // Adaptation context injection (phase change notification)
    if let Some(msg) = adaptation_msg {
        parts.push(msg);
    }

    let reinject_interval: u32 = state.adaptive.params.rules_reinject_interval;
    if reinject_interval > 0 && turn > 0 && turn.is_multiple_of(reinject_interval) {
        let rules_path = common::assistant_rules_dir().join("tool-enforcement.md");
        if let Ok(content) = std::fs::read_to_string(&rules_path) {
            let condensed = common::truncate(&content, 800);
            parts.push(format!("## Rules Reminder (turn {})\n{}", turn, condensed));
            common::log_structured("userprompt-context", common::LogLevel::Info, "rules-reinject", &format!("turn {}", turn));
        }
    }

    let drift_threshold: u32 = state.adaptive.params.drift_threshold;
    if drift_threshold > 0 && state.denial_rate(10) >= drift_threshold {
        parts.push(format!(
            "DRIFT DETECTED: {} tool denials in last 10 turns. Re-read your rules. \
            Check substitutions: grep->rg, find->fd, cat->bat, curl->xh. \
            Use just <recipe> when Justfile exists. Run `{} explain <rule-id>` for details.",
            state.denial_rate(10), crate::constants::NAME
        ));
        common::log_structured("userprompt-context", common::LogLevel::Info, "drift-warning", &format!("turn {}", turn));
        state.recent_denial_turns.clear();
    }

    // Git summary first (most useful grounding)
    if let Some(ref git) = git_line {
        parts.push(git.clone());
    }

    if session_path.exists() {
        let tail = common::read_tail(&session_path, 1024);
        if !tail.is_empty() {
            let mut recent_errors: Vec<String> = Vec::new();
            let mut edited_files: Vec<String> = Vec::new();

            for line in tail.lines().rev() {
                let entry = match serde_json::from_str::<serde_json::Value>(line) {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                let note_type = entry
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let detail = entry
                    .get("detail")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                match note_type {
                    "error" if recent_errors.len() < 3 => {
                        let error_type = detail.split_whitespace().next().unwrap_or(detail);
                        if !recent_errors.iter().any(|e| e.starts_with(error_type)) {
                            recent_errors.push(detail.to_string());
                        }
                    }
                    "edit" if edited_files.len() < 5 => {
                        if !edited_files.contains(&detail.to_string()) {
                            edited_files.push(detail.to_string());
                        }
                    }
                    _ => {}
                }
            }

            if !recent_errors.is_empty() {
                parts.push(format!("Recent errors: {}", recent_errors.join(" | ")));
            }
            if !edited_files.is_empty() {
                parts.push(format!("Edited: {}", edited_files.join(", ")));
            }
        }
    }

    // Files-in-context reminder
    if !files_in_context.is_empty() {
        parts.push(format!("In context: {}", files_in_context));
    }

    // Exploration budget advisory (rate-limited)
    if explore_ready {
        parts.push(format!(
            "Note: {} exploration operations without editing. Consider committing to an approach.",
            explore_count
        ));
    }

    // Token budget advisory (rate-limited)
    if let Some(advisory) = token_advisory {
        parts.push(advisory);
    }

    // Cost budget check: warn when estimated cost exceeds threshold
    {
        let total_tokens = state.estimated_tokens_in + state.estimated_tokens_out;
        // Default: $3/1M input, $15/1M output → rough blended rate ~$9/1M
        let estimated_cost = total_tokens as f64 / 1_000_000.0 * 9.0;
        // TODO: make configurable via config.toml budget.max_session_cost
        let budget_limit = 5.0f64; // $5 default
        if estimated_cost > budget_limit && state.advisory_ready("cost_budget") {
            parts.push(format!(
                "Session cost estimate: ${:.2} (budget: ${:.2}). Consider wrapping up or focusing on high-impact changes.",
                estimated_cost, budget_limit
            ));
        }
    }

    // Parallelism hint: nudge when multiple independent items exist
    if state.errors_unresolved > 1 && state.advisory_ready("parallel") {
        parts.push(format!(
            "{} unresolved errors — consider parallel fix agents.",
            state.errors_unresolved
        ));
    }

    // Late-session compactness mode (rate-limited, scaled to 1M context)
    let deny_turn = rules::RULES.progressive_read_deny_turn;
    let advisory_turn = rules::RULES.progressive_read_advisory_turn;
    if turn >= deny_turn && state.advisory_ready("context_pressure") {
        parts.push("Context pressure: HIGH. Minimize reads — use targeted reads with offset+limit.".to_string());
    } else if turn >= advisory_turn && state.advisory_ready("context_pressure") {
        parts.push("Context pressure: moderate. Prefer targeted reads (offset+limit).".to_string());
    }

    if parts.is_empty() {
        common::write_session_state(&state);
        return;
    }

    // Context delta: hash the output, skip if identical to last turn's injection
    let combined = parts.join("\n");
    let context_hash = common::string_hash(&combined);
    if context_hash == state.last_context_hash {
        common::write_session_state(&state);
        return; // Nothing new — skip injection entirely
    }
    state.last_context_hash = context_hash;
    common::write_session_state(&state);

    let max_chars: usize = state.adaptive.params.context_chars_max;
    if combined.len() > max_chars {
        common::additional_context(&combined[..max_chars]);
    } else {
        common::additional_context(&combined);
    }
}

/// Build compact "files in context" string from recent reads.
/// Applies salience decay: files read >10 turns ago are dropped,
/// files read >5 turns ago are marked as stale.
fn build_files_in_context(state: &common::SessionState) -> String {
    if state.files_read.is_empty() {
        return String::new();
    }

    let turn = state.turn;

    // Sort by turn descending, filter by salience
    let mut entries: Vec<(&String, &common::FileReadEntry)> = state.files_read.iter()
        .filter(|(_, entry)| turn.saturating_sub(entry.turn) <= 10) // salience decay: drop after 10 turns
        .collect();
    entries.sort_by(|a, b| b.1.turn.cmp(&a.1.turn));

    let items: Vec<String> = entries
        .iter()
        .take(5)
        .map(|(path, entry)| {
            let short = shorten_path(path);
            let age = turn.saturating_sub(entry.turn);
            if age > 5 {
                format!("{} (t{}, stale)", short, entry.turn)
            } else {
                format!("{} (t{})", short, entry.turn)
            }
        })
        .collect();

    items.join(", ")
}

/// Detect context switch: >60% of rolling working set dirs are NOT in initial set
/// AND no initial-set files touched for 8+ turns.
fn detect_context_switch(state: &common::SessionState) -> bool {
    if state.initial_working_set.is_empty() || state.rolling_working_set.len() < 3 {
        return false;
    }
    let new_dirs = state.rolling_working_set.iter()
        .filter(|d| !state.initial_working_set.contains(d))
        .count();
    let ratio = new_dirs as f64 / state.rolling_working_set.len() as f64;
    let stale = state.turn.saturating_sub(state.last_initial_set_touch_turn) >= 8;
    ratio > 0.6 && stale
}

/// Shorten path to last 2 components
fn shorten_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.rsplit('/').take(2).collect();
    if parts.len() >= 2 {
        format!("{}/{}", parts[1], parts[0])
    } else {
        parts.first().unwrap_or(&path).to_string()
    }
}

// ─── Heuristic advisories from turn snapshots ──────────────────────────────

/// Compute heuristic advisories from turn snapshots. Each advisory is gated
/// by advisory_ready() cooldown to prevent spam.
///
/// Pattern: compute all signals from snapshots first (immutable borrow),
/// then check cooldowns and build messages (mutable borrow).
fn heuristic_advisories(state: &mut common::SessionState) -> Vec<String> {
    if state.turn_snapshots.len() < 3 {
        return Vec::new();
    }

    let r = &rules::RULES;

    // Phase 1: compute signals from snapshots (immutable access only)
    let sig_error_slope = if state.errors_unresolved >= 3 {
        error_slope(&state.turn_snapshots, 10) > r.error_slope_threshold
    } else {
        false
    };

    let stale_window = r.stale_milestone_turns as usize;
    let sig_stale = state.turn >= 15
        && state.turn_snapshots.len() >= stale_window
        && !state.turn_snapshots[state.turn_snapshots.len().saturating_sub(stale_window)..]
            .iter().any(|s| s.milestones_hit);

    let sig_token_burn = if state.turn_snapshots.len() >= 5 {
        let recent5 = &state.turn_snapshots[state.turn_snapshots.len() - 5..];
        let avg_tokens: u64 = recent5.iter().map(|s| s.tokens_in_delta + s.tokens_out_delta).sum::<u64>() / 5;
        avg_tokens > r.token_burn_threshold && !recent5.iter().any(|s| s.edits_this_turn)
    } else {
        false
    };

    let stag_window = r.stagnation_turns as usize;
    let sig_stagnation = if state.turn_snapshots.len() >= stag_window {
        let recent = &state.turn_snapshots[state.turn_snapshots.len() - stag_window..];
        let explore_growing = recent.windows(2).all(|w| w[1].explore_count >= w[0].explore_count);
        explore_growing && !recent.iter().any(|s| s.edits_this_turn) && !recent.iter().any(|s| s.milestones_hit)
    } else {
        false
    };

    let sig_ping_pong = if state.turn_snapshots.len() >= 6 {
        let recent6 = &state.turn_snapshots[state.turn_snapshots.len() - 6..];
        let alternations = recent6.windows(2)
            .filter(|w| w[0].edits_this_turn && w[1].errors_unresolved > w[0].errors_unresolved)
            .count();
        alternations >= 3
    } else {
        false
    };

    // Phase 2: emit advisories with cooldown gates (mutable access)
    let mut advisories = Vec::new();

    if sig_error_slope && state.advisory_ready("error_slope") {
        advisories.push("Errors rising. Run build/test to verify before continuing.".to_string());
    }
    if sig_stale && state.advisory_ready("stale_session") {
        advisories.push(format!("No milestones in {} turns. Consider a different approach.", stale_window));
    }
    if sig_token_burn && state.advisory_ready("token_burn") {
        advisories.push("High token burn without edits. Focus on implementation.".to_string());
    }
    if sig_stagnation && state.advisory_ready("stagnation") {
        advisories.push("Exploration without progress. Commit to an approach and start editing.".to_string());
    }
    if sig_ping_pong && state.advisory_ready("ping_pong") {
        advisories.push("Edit-error cycle detected. Verify your approach before more edits.".to_string());
    }

    advisories
}

/// Linear regression slope of errors_unresolved over last N snapshots.
/// Positive slope = errors trending up.
pub fn error_slope(snapshots: &[common::TurnSnapshot], window: usize) -> f64 {
    let snaps = if snapshots.len() > window {
        &snapshots[snapshots.len() - window..]
    } else {
        snapshots
    };
    let n = snaps.len() as f64;
    if n < 2.0 {
        return 0.0;
    }

    let mut sum_x = 0.0f64;
    let mut sum_y = 0.0f64;
    let mut sum_xy = 0.0f64;
    let mut sum_xx = 0.0f64;

    for (i, snap) in snaps.iter().enumerate() {
        let x = i as f64;
        let y = snap.errors_unresolved as f64;
        sum_x += x;
        sum_y += y;
        sum_xy += x * y;
        sum_xx += x * x;
    }

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < f64::EPSILON {
        return 0.0;
    }
    (n * sum_xy - sum_x * sum_y) / denom
}
