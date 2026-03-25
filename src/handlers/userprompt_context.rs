// ─── userprompt_context — per-turn context grounding ────────────────────────
//
// UserPromptSubmit handler. Runs before Claude processes each user message.
//
// 7 consolidated advisory signals (candidates for injection):
//   1. Safety  (1.0) — drift detection
//   2. Loop    (0.9) — loop pattern detection
//   3. Verify  (0.8) — verification debt, read drift, checkpoint
//   4. Phase   (0.7) — adaptation phase change
//   5. Recovery(0.6) — error prevention
//   6. Focus   (0.5) — focus score, explore hint
//   7. Pressure(0.4) — context pressure, token/cost budget
//
// All other analytics compute + write redb silently (never candidates).
// Budget: trust > 85 → top 1, > 50 → top 3, > 25 → top 5, else → 15.
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
    let _git_line = git_summary::get_or_refresh(&mut state);

    // Token budget check (rate-limited)
    let token_advisory = if state.advisory_ready("token_budget") {
        token_budget::check_threshold(&state)
    } else {
        None
    };

    // Heuristic advisories from turn snapshots
    let heuristic_parts = heuristic_advisories(&mut state);

    // ── Runtime analytics (gated by config.toml telemetry flags) ──
    let tel = &crate::config::CONFIG.telemetry;

    // Anomaly detection: check current turn against project baselines
    let anomaly_alerts = if tel.anomaly_detection {
        let project_dir = common::project_dir();
        let stats = analytics::anomaly::load_stats(&project_dir);
        let last_snap = state.turn_snapshots.last();
        let tokens_this_turn = last_snap
            .map(|s| s.tokens_in_delta + s.tokens_out_delta)
            .unwrap_or(0);
        analytics::anomaly::check_anomalies(&stats, tokens_this_turn, 2.0)
    } else {
        Vec::new()
    };

    // Token budget forecasting: predict compaction ETA
    let forecast_msg = if tel.token_forecast {
        analytics::forecast::predict_compaction(
            &state.turn_snapshots,
            state.turn,
            state.estimated_tokens_in + state.estimated_tokens_out,
            rules::RULES.token_budget_advisory,
        ).map(|f| analytics::forecast::format_forecast(&f))
         .filter(|s| !s.is_empty())
    } else {
        None
    };

    // Quality prediction: fires at turn 10, then every 5 turns
    let quality_msg = if tel.quality_predictor {
        analytics::quality::predict_quality(
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
        }).filter(|s| !s.is_empty())
    } else {
        None
    };

    // Error prevention: check current edit patterns against Bayesian priors
    let error_prevention_msg = if tel.error_prevention {
        let project_dir = common::project_dir();
        let priors = analytics::error_prevention::load_priors(&project_dir);
        let edits_since_build = state.turn.saturating_sub(state.last_build_turn);
        let edited_dirs = state.files_edited.iter()
            .filter_map(|f| f.rsplit('/').nth(1))
            .collect::<std::collections::HashSet<_>>().len() as u32;
        let turns_since_test = state.turn.saturating_sub(state.last_build_turn);
        analytics::error_prevention::check_patterns(&priors, edits_since_build, edited_dirs, turns_since_test, 0.6)
    } else {
        None
    };

    // Git guardian: uncommitted changes duration check
    let git_uncommitted = crate::handlers::git_guardian::check_uncommitted_duration(&state);

    // ── New intelligence modules ──

    // Verification debt: edits since last build/test
    let verification_msg = analytics::verification::check_debt(&state);
    let read_drift_msg = analytics::verification::check_read_drift(&state);

    // Focus score: composite metric
    let focus_report = analytics::focus::compute_focus(&state);

    // Loop pattern detection: 2-gram, 3-gram, read spirals
    let loop_msg = analytics::loop_patterns::check_loop_patterns(&state.action_history);

    // Checkpoint enforcement: turns since last milestone/verification
    state.turns_since_checkpoint += 1;
    let checkpoint_msg = if state.turns_since_checkpoint >= 8 {
        Some(format!(
            "{} turns without a milestone or verification. Run a build/test.",
            state.turns_since_checkpoint
        ))
    } else {
        None
    };

    // Files-in-context: last 5 files read, sorted by turn (most recent first)
    let _files_in_context = build_files_in_context(&state);

    // Read session-notes.jsonl for recent context
    let project_dir = common::project_dir();
    let _session_path = project_dir.join("session-notes.jsonl");

    // ── Injection Budget System ──
    // All analytics still compute and write state. The budget controls what gets
    // injected into the agent's context. Signals below the budget cutoff are
    // logged silently to redb — nothing is lost, just quieted.

    let trust = crate::analytics::trust::compute_trust(&state);

    // Budget: how many advisories to inject based on trust level
    let advisory_budget = if trust > crate::config::TRUST_BUDGET_HIGH { 1 }
        else if trust > crate::config::TRUST_BUDGET_NORMAL { 3 }
        else if trust > crate::config::TRUST_BUDGET_DEGRADED { 5 }
        else { 15 };

    // Collect ALL candidates with utility scores
    let mut candidates: Vec<(f32, &str, String)> = Vec::new(); // (utility, category, message)

    // ── 7 Consolidated Signals (candidates for injection) ──
    // All analytics still compute + write state. Only 7 categories compete for budget.

    // 1. Safety (1.0): drift detection
    let drift_threshold: u32 = state.adaptive.params.drift_threshold;
    if drift_threshold > 0 && state.denial_rate(10) >= drift_threshold {
        candidates.push((1.0, "safety", format!(
            "DRIFT: {} denials in 10 turns. Check: grep→rg, find→fd, cat→bat, curl→xh.",
            state.denial_rate(10)
        )));
        common::log_structured("userprompt-context", common::LogLevel::Info, "drift-warning", &format!("turn {}", turn));
        state.recent_denial_turns.clear();
    }

    // 2. Loop (0.9): loop patterns
    if let Some(msg) = loop_msg {
        candidates.push((0.9, "loop", msg));
    }

    // 3. Verification (0.8): verification debt, read drift, checkpoint
    if let Some(msg) = verification_msg {
        candidates.push((0.8, "verification", msg));
    } else if let Some(msg) = read_drift_msg {
        candidates.push((0.8, "verification", msg));
    } else if let Some(msg) = checkpoint_msg {
        candidates.push((0.8, "verification", msg));
    }

    // 4. Phase (0.7): adaptation
    if let Some(msg) = adaptation_msg {
        candidates.push((0.7, "phase", msg));
    }

    // 5. Recovery (0.6): error prevention
    if let Some(msg) = error_prevention_msg {
        candidates.push((0.6, "recovery", msg));
    }

    // 6. Focus (0.5): focus score + explore hint
    if let Some(msg) = focus_report.advisory {
        candidates.push((0.5, "focus", msg));
    } else if explore_ready {
        candidates.push((0.5, "focus", format!(
            "{} exploration ops without editing. Commit to an approach.", explore_count
        )));
    }

    // 7. Pressure (0.4): context pressure + token budget + cost
    {
        let deny_turn = rules::RULES.progressive_read_deny_turn;
        let advisory_turn = rules::RULES.progressive_read_advisory_turn;
        if turn >= deny_turn && state.advisory_ready("context_pressure") {
            candidates.push((0.4, "pressure", "Context pressure: HIGH. Use targeted reads with offset+limit.".to_string()));
        } else if let Some(advisory) = token_advisory {
            candidates.push((0.4, "pressure", advisory));
        } else if turn >= advisory_turn && state.advisory_ready("context_pressure") {
            candidates.push((0.4, "pressure", "Context pressure: moderate. Prefer targeted reads.".to_string()));
        } else {
            let total_tokens = state.estimated_tokens_in + state.estimated_tokens_out;
            let estimated_cost = total_tokens as f64 / 1_000_000.0 * 9.0;
            if estimated_cost > 5.0 && state.advisory_ready("cost_budget") {
                candidates.push((0.4, "pressure", format!("Cost: ${:.2}/$5.00. Focus on high-impact changes.", estimated_cost)));
            }
        }
    }

    // ── Silent signals (logged to redb, never candidates) ──
    for (cat, msg) in [
        ("anomaly", anomaly_alerts.first().map(|s| s.to_string())),
        ("forecast", forecast_msg),
        ("quality", quality_msg),
        ("entropy", entropy_advisory),
        ("markov", markov_advisory),
        ("coherence", coherence_advisory),
        ("goal", goal_anchor),
        ("git", git_uncommitted),
    ] {
        if let Some(m) = msg {
            common::log_structured("userprompt-context", common::LogLevel::Info, cat,
                &format!("silent {}", common::truncate(&m, 80)));
        }
    }
    for msg in &heuristic_parts {
        common::log_structured("userprompt-context", common::LogLevel::Info, "session_health",
            &format!("silent {}", common::truncate(msg, 80)));
    }

    // E.1: Intent-based dedup — keep only highest-utility candidate per category
    {
        let mut seen_categories: std::collections::HashSet<&str> = std::collections::HashSet::new();
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        candidates.retain(|(_, cat, _)| seen_categories.insert(cat));
    }

    // Phase 5.1: String similarity dedup — collapse candidates saying the same thing
    {
        let mut i = 0;
        while i < candidates.len() {
            let mut j = i + 1;
            while j < candidates.len() {
                if jaccard_similarity(&candidates[i].2, &candidates[j].2) > 0.6 {
                    if candidates[i].0 >= candidates[j].0 { candidates.remove(j); }
                    else { candidates.remove(i); continue; }
                } else { j += 1; }
            }
            i += 1;
        }
    }

    // Phase 5.2: Dream-informed utility adjustment
    {
        let dream_scores = crate::dream::get_intervention_scores();
        for (utility, cat, _) in &mut candidates {
            let effectiveness = dream_scores.scores.get(*cat).copied().unwrap_or(0.5);
            *utility *= effectiveness as f32;
        }
    }

    // Sort by utility descending, apply budget
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Log ALL candidates silently to redb (nothing is lost)
    for (utility, category, msg) in &candidates {
        common::log_structured("userprompt-context", common::LogLevel::Info, category,
            &format!("u={:.1} trust={} {}", utility, trust, common::truncate(msg, 80)));
    }

    // Take top N by budget
    let selected: Vec<String> = candidates.into_iter()
        .take(advisory_budget)
        .map(|(_, _, msg)| msg)
        .collect();

    // Event-based rule reinjection (replaces periodic)
    let mut parts = selected;
    if should_reinject_rules(&state) {
        let rules_path = common::assistant_rules_dir().join("tool-enforcement.md");
        if let Ok(content) = std::fs::read_to_string(&rules_path) {
            let condensed = common::truncate(&content, 800);
            parts.push(format!("## Rules Reminder (turn {})\n{}", turn, condensed));
            common::log_structured("userprompt-context", common::LogLevel::Info, "rules-reinject", &format!("turn {}", turn));
        }
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
        return;
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

/// Jaccard similarity between two strings (word-level)
fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();
    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}

/// Event-based rule reinjection: only reinject when there's evidence the agent
/// is ignoring rules, not on a periodic timer. Reduces context waste.
fn should_reinject_rules(state: &common::SessionState) -> bool {
    // 3+ denials in last 5 turns = agent is fighting rules
    let recent_denials = state.recent_denial_turns.iter()
        .filter(|&&t| t + 5 >= state.turn)
        .count();
    if recent_denials >= 3 { return true; }

    // Just after compaction = rules may have been lost from context
    if state.last_compaction_turn > 0 && state.turn == state.last_compaction_turn + 1 {
        return true;
    }

    false
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
