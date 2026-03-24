// ─── pretool_read — PreToolUse handler for Read tool governance ────────────────
//
// Four responsibilities:
//   1. Post-edit read suppression: DENY re-reading a file just edited (within 2 turns)
//   2. Read dedup: DENY re-reading unchanged files still in context (within 10 turns)
//   3. Progressive ranged read: ADVISORY/DENY full reads of medium files in late sessions
//   4. Large file governance: prevent full reads of code files >50KB
//
// Allows unconditionally:
//   - Ranged reads (offset, limit, start_line, end_line, line_range present)
//   - Non-code file extensions (configs, markdown, JSON, etc.) — for governance only
//   - Files that can't be stat'd (new/inaccessible — fail open)
//   - Code files under progressive threshold (turn-dependent)
//
// The dedup DENY has a compaction guard: reads are allowed freely for 2 turns
// after compaction (files_read is cleared on PreCompact).
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::config;
use crate::rules;
use std::fs;
use std::path::Path;

/// PreToolUse handler for Read — dedup deny + post-edit suppression + large file governance
pub fn run(raw: &str) {
    let input = common::parse_input_or_return!(raw);

    let ti = match input.tool_input.as_ref() {
        Some(v) => v,
        None => return,
    };

    let file_path = ti
        .get("file_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if file_path.is_empty() {
        return;
    }

    // If range params present, allow (targeted read — already efficient)
    let has_range = ti.get("offset").is_some()
        || ti.get("limit").is_some()
        || ti.get("start_line").is_some()
        || ti.get("end_line").is_some()
        || ti.get("line_range").is_some();

    if has_range {
        common::log_structured("pretool-read", common::LogLevel::Allow, "ranged-read", truncate_path(file_path));
        return;
    }

    let state = common::read_session_state();

    // ── 0. Edit-intent exemption ──
    // If this file was previously edited, allow the read unconditionally.
    // The Edit tool requires a prior Read — blocking it creates a deadlock
    // where Claude can't re-read files it needs to edit further.
    let norm_path = common::normalize_path(file_path);
    let is_edited_file = state.files_edited.iter().any(|f| common::normalize_path(f) == norm_path);
    if is_edited_file {
        let size = fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
        track_read(file_path, size);
        common::log_structured("pretool-read", common::LogLevel::Allow, "edit-intent", truncate_path(file_path));
        return;
    }

    // ── 1. Post-edit read advisory ──
    // If this file was edited within the last 2 turns, advise but allow.
    // The Edit tool output already confirms the changes, but blocking reads
    // can cause confusion when the AI needs to re-read for further edits.
    if !state.last_edited_file.is_empty()
        && norm_path == common::normalize_path(&state.last_edited_file)
        && state.turn.saturating_sub(state.last_edit_turn) <= 2
    {
        track_read(file_path, fs::metadata(file_path).map(|m| m.len()).unwrap_or(0));
        common::log_structured("pretool-read", common::LogLevel::Advisory, "post-edit", truncate_path(file_path));
        common::allow_with_advisory(
            "PreToolUse",
            &format!(
                "You edited this file at turn {}. The Edit tool confirmed your changes — this re-read may be unnecessary.",
                state.last_edit_turn
            ),
        );
        return;
    }

    // ── 2. Read dedup — advisory for unchanged files still in context ──
    // Downgraded from deny to advisory: false denies (post-compaction, edit-intent)
    // cause more harm than the ~200 tokens saved per blocked read.
    if let Some(advisory_or_deny) = check_read_dedup(file_path, &state) {
        let msg = match advisory_or_deny {
            DedupAction::Deny(msg) | DedupAction::Advisory(msg) => msg,
        };
        track_read(file_path, fs::metadata(file_path).map(|m| m.len()).unwrap_or(0));
        common::log_structured("pretool-read", common::LogLevel::Advisory, "read-dedup", truncate_path(file_path));
        common::allow_with_advisory("PreToolUse", &msg);
        return;
    }

    // ── 3. Size + extension checks ──
    let ext = file_path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();

    if !config::CODE_EXTS.contains(&ext.as_str()) {
        let size = fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
        track_read(file_path, size);
        common::log_structured("pretool-read", common::LogLevel::Allow, "non-code-ext", truncate_path(file_path));
        return;
    }

    let size = match fs::metadata(file_path) {
        Ok(m) => m.len(),
        Err(_) => {
            common::log_structured("pretool-read", common::LogLevel::Allow, "no-stat", truncate_path(file_path));
            return;
        }
    };

    // ── 3a. Large files >50KB — advisory instead of deny ──
    if size > rules::RULES.max_read_size {
        let kb = size / 1024;
        let hint = large_file_hint(&ext);
        common::log_structured("pretool-read", common::LogLevel::Allow, "large-file-advisory",
            &format!("~{}KB {}", kb, truncate_path(file_path)));
        common::allow_with_advisory(
            "PreToolUse",
            &format!("~{}KB file. {} Consider using offset+limit for targeted reads.", kb, hint),
        );
        return;
    }

    // ── 3b. Progressive ranged read enforcement ──
    // As session progresses, push toward targeted reads for medium files.
    // Thresholds tighten with turn count to reduce context waste.
    let turn = state.turn;
    if let Some(action) = check_progressive_read(size, turn) {
        match action {
            ProgressiveAction::Deny(msg) => {
                record_deny_savings(size / 3); // estimate ~1/3 of file size in tokens
                common::log_structured("pretool-read", common::LogLevel::Deny, "progressive", truncate_path(file_path));
                common::deny("PreToolUse", &msg);
                return;
            }
            ProgressiveAction::Advisory(msg) => {
                track_read(file_path, size);
                common::log_structured("pretool-read", common::LogLevel::Advisory, "progressive", truncate_path(file_path));
                common::allow_with_advisory("PreToolUse", &msg);
                return;
            }
        }
    }

    // Track and allow
    track_read(file_path, size);
    common::log_structured("pretool-read", common::LogLevel::Allow, "pass", truncate_path(file_path));
}

/// Track a file read in session state — called from PreToolUse since PostToolUse
/// doesn't fire for built-in Read tool.
fn track_read(file_path: &str, size: u64) {
    if let Some(hash) = common::content_hash(Path::new(file_path)) {
        let mtime = common::file_mtime(Path::new(file_path)).unwrap_or(0);
        let mut state = common::read_session_state();
        state.explore_count += 1;
        state.files_read.insert(
            file_path.to_string(),
            common::FileReadEntry {
                hash,
                turn: state.turn,
                size,
                mtime,
            },
        );
        state.enforce_bounds();
        common::write_session_state(&state);
    }
}

enum DedupAction {
    Deny(String),
    Advisory(String),
}

/// Check read dedup — returns Deny if within context window, Advisory if borderline
fn check_read_dedup(file_path: &str, state: &common::SessionState) -> Option<DedupAction> {
    if state.files_read.is_empty() {
        return None;
    }

    let prev = state.files_read.get(file_path)?;

    // Check if content changed
    let current_hash = common::content_hash(Path::new(file_path))?;
    if current_hash != prev.hash {
        return None; // Content changed — allow the read
    }

    let turn_gap = state.turn.saturating_sub(prev.turn);
    let post_compaction = state.turn.saturating_sub(state.last_compaction_turn) <= 2;

    // Very recent (≤2 turns): advisory only — model may need content for editing
    if turn_gap <= 2 && !post_compaction {
        return Some(DedupAction::Advisory(format!(
            "You read this file at turn {} ({} turns ago). Content unchanged (~{} bytes). It's likely still in your context — skip re-reading if you already have what you need.",
            prev.turn, turn_gap, prev.size
        )));
    }

    // Within dedup window and not right after compaction → DENY
    // (file content is still in Claude's context window)
    // TODO: Re-enable adaptive dedup window when adaptation module is ported.
    // Was: state.adaptive.params.read_dedup_window
    let dedup_window: u32 = 0;
    let dedup_deny = if dedup_window > 0 { dedup_window } else { 10 };
    let dedup_advisory = dedup_deny * 2;

    if turn_gap <= dedup_deny && !post_compaction {
        return Some(DedupAction::Deny(format!(
            "You read this file at turn {} ({} turns ago). Content unchanged (~{} bytes). It's still in your context.",
            prev.turn, turn_gap, prev.size
        )));
    }

    // Beyond deny window but within advisory zone → advisory
    if turn_gap <= dedup_advisory {
        return Some(DedupAction::Advisory(format!(
            "You read this file at turn {} ({} turns ago). Content unchanged (~{} bytes). It may still be in your context.",
            prev.turn, turn_gap, prev.size
        )));
    }

    // Beyond advisory zone → allow (likely compacted out)
    None
}

enum ProgressiveAction {
    Deny(String),
    Advisory(String),
}

/// Progressive ranged read enforcement — tightens with session turn.
/// Returns None if file is small enough to pass at current turn.
fn check_progressive_read(size: u64, turn: u32) -> Option<ProgressiveAction> {
    let kb = size / 1024;
    let deny_turn = rules::RULES.progressive_read_deny_turn;
    let advisory_turn = rules::RULES.progressive_read_advisory_turn;

    // Late session (turn >= deny threshold): deny full reads >10KB, advisory >5KB
    if turn >= deny_turn {
        if size > 10_000 {
            return Some(ProgressiveAction::Deny(format!(
                "~{}KB file at turn {} (high context pressure). Use start_line + end_line to read only the section you need.",
                kb, turn
            )));
        }
        if size > 5_000 {
            return Some(ProgressiveAction::Advisory(format!(
                "~{}KB file at turn {} — consider using start_line + end_line to reduce context usage.",
                kb, turn
            )));
        }
    }
    // Mid-late session (turn >= advisory threshold): advisory for >15KB
    else if turn >= advisory_turn && size > 15_000 {
        return Some(ProgressiveAction::Advisory(format!(
            "~{}KB file at turn {}. Prefer start_line + end_line for targeted reads to conserve context.",
            kb, turn
        )));
    }

    None
}

/// Record token savings from a deny intervention
fn record_deny_savings(tokens: u64) {
    let mut state = common::read_session_state();
    state.estimated_tokens_saved += tokens;
    state.savings_deny += 1;
    common::write_session_state(&state);
}

/// Build contextual hint for large file denial
fn large_file_hint(ext: &str) -> &'static str {
    let has_aidex = Path::new(".aidex").exists();
    let aidex_supports = config::AIDEX_EXTS.contains(&ext);

    if has_aidex && aidex_supports {
        "use aidex_signature first, then targeted Read with offset+limit"
    } else {
        "use outline first, then targeted Read with offset+limit"
    }
}

fn truncate_path(p: &str) -> &str {
    if p.len() > 80 {
        &p[p.len() - 80..]
    } else {
        p
    }
}
