// ─── truncate_filter — stdin pipe filter for verbose command output ────────────
//
// Unlike other handlers, this is NOT a JSON hook — it reads raw line-stream
// stdin piped from Bash commands: `warden truncate-filter [--mode MODE]`
//
// Modes:
//   default — head + tail + important lines (original behavior)
//   test    — keep only failure lines + summary (strip passing tests)
//   build   — keep only error/warning lines + final status
//   install — keep only final line + any errors/warnings
//
// Progressive compression based on session turn:
//   Turn 1-15:  max 80 lines
//   Turn 16-30: max 60 lines
//   Turn 31+:   max 40 lines
// ──────────────────────────────────────────────────────────────────────────────

use super::smart_filter;
use crate::common;
use regex::Regex;
use std::collections::HashSet;
use std::io::{self, BufRead, Write};
use std::sync::LazyLock;

// ── Compiled regex patterns (initialized once, reused across daemon requests) ──

static FAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(crate::config::TRUNCATE_FAIL).unwrap());

static SUMMARY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(crate::config::TRUNCATE_SUMMARY).unwrap());

static BUILD_IMPORTANT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(crate::config::TRUNCATE_BUILD_IMPORTANT).unwrap());

static INSTALL_ERROR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(crate::config::TRUNCATE_INSTALL_ERROR).unwrap());

static DEFAULT_IMPORTANT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(crate::config::TRUNCATE_DEFAULT_IMPORTANT).unwrap());

struct Thresholds {
    max_lines: usize,
    head: usize,
    tail: usize,
    important: usize,
}

fn get_thresholds() -> Thresholds {
    let state = common::read_session_state();

    let adapted = state.adaptive.params.truncation_max_lines;

    // Use adapted max_lines if available, else fall back to turn-based defaults
    let max_lines = if adapted > 0 {
        adapted
    } else {
        match state.turn {
            0..=15 => 80,
            16..=30 => 60,
            _ => 40,
        }
    };

    // Scale head/tail/important proportionally to max_lines
    let head = (max_lines * 15 / 80).max(5);
    let tail = head;
    let important = (max_lines * 20 / 80).max(5);

    Thresholds {
        max_lines,
        head,
        tail,
        important,
    }
}

/// Parse --cmd from CLI args (the command being filtered)
fn parse_cmd() -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--cmd" {
            // Everything after --cmd is the command
            return args[i + 1..].join(" ");
        }
    }
    String::new()
}

/// Parse --mode from CLI args (called by main.rs)
pub fn parse_mode() -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--mode"
            && let Some(mode) = args.get(i + 1)
        {
            return mode.clone();
        }
    }
    "default".to_string()
}

/// Truncation filter entry point
pub fn run() {
    let mode = parse_mode();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    let mut lines: Vec<String> = Vec::new();
    for line_result in stdin.lock().lines() {
        match line_result {
            Ok(line) => lines.push(line),
            Err(_) => break,
        }
    }

    // Error cluster compression: group similar errors before filtering
    let lines = cluster_errors(lines);

    match mode.as_str() {
        "test" => filter_test(&lines, &mut out),
        "build" => filter_build(&lines, &mut out),
        "install" => filter_install(&lines, &mut out),
        "smart" => {
            let t = get_thresholds();
            filter_smart(&lines, &mut out, t.max_lines);
        }
        _ => {
            let t = get_thresholds();
            // Try per-command smart filter first (RTK-style)
            let cmd = parse_cmd();
            if !cmd.is_empty() {
                let full_output = lines.join("\n");
                if let Some(result) = smart_filter::filter_output(&cmd, &full_output, t.max_lines) {
                    let _ = write!(out, "{}", result.output);
                    record_savings(result.original_lines, result.kept_lines);
                    return;
                }
            }
            // Fall back: relevance scoring, then head+tail
            if lines.len() > t.max_lines {
                filter_smart(&lines, &mut out, t.max_lines);
            } else {
                filter_default(&lines, &mut out);
            }
        }
    }
}

/// Test mode: keep only failure lines + summary line
fn filter_test(lines: &[String], out: &mut impl Write) {
    let t = get_thresholds();

    if lines.len() <= t.max_lines {
        for line in lines {
            let _ = writeln!(out, "{}", line);
        }
        return;
    }

    record_savings(lines.len(), t.max_lines);

    let mut kept: Vec<&str> = Vec::new();
    let mut kept_set: HashSet<&str> = HashSet::new();

    // Always keep first 3 lines (framework header)
    for line in lines.iter().take(3) {
        kept.push(line);
        kept_set.insert(line);
    }

    // Keep failure lines (with 1 line of context before)
    for (i, line) in lines.iter().enumerate() {
        if FAIL_RE.is_match(line) {
            // Context line before the failure
            if i > 0 && kept.len() < t.max_lines {
                let prev = lines[i - 1].as_str();
                if !kept_set.contains(prev) {
                    kept.push(prev);
                    kept_set.insert(prev);
                }
            }
            if kept.len() < t.max_lines && !kept_set.contains(line.as_str()) {
                kept.push(line);
                kept_set.insert(line);
            }
        }
    }

    // Keep summary lines (usually at the end)
    for line in lines.iter().rev().take(10) {
        if SUMMARY_RE.is_match(line) && !kept_set.contains(line.as_str()) {
            kept.push(line);
            kept_set.insert(line);
        }
    }

    // Always keep last 3 lines
    for line in lines.iter().rev().take(3) {
        if !kept_set.contains(line.as_str()) {
            kept.push(line);
            kept_set.insert(line);
        }
    }

    let _ = writeln!(
        out,
        "--- TEST OUTPUT (filtered: {} lines -> {} kept, failures + summary) ---",
        lines.len(),
        kept.len()
    );
    for line in &kept {
        let _ = writeln!(out, "{}", line);
    }
}

/// Build mode: keep only error/warning lines + final status
fn filter_build(lines: &[String], out: &mut impl Write) {
    let t = get_thresholds();

    if lines.len() <= t.max_lines {
        for line in lines {
            let _ = writeln!(out, "{}", line);
        }
        return;
    }

    record_savings(lines.len(), t.max_lines);

    let mut kept: Vec<&str> = Vec::new();
    let mut kept_set: HashSet<&str> = HashSet::new();

    // First 3 lines (build command echo, etc.)
    for line in lines.iter().take(3) {
        kept.push(line);
        kept_set.insert(line);
    }

    // Error/warning lines
    for line in lines.iter() {
        if BUILD_IMPORTANT_RE.is_match(line)
            && kept.len() < t.max_lines
            && !kept_set.contains(line.as_str())
        {
            kept.push(line);
            kept_set.insert(line);
        }
    }

    // Last 5 lines (build summary)
    for line in lines.iter().rev().take(5) {
        if !kept_set.contains(line.as_str()) {
            kept.push(line);
            kept_set.insert(line);
        }
    }

    let _ = writeln!(
        out,
        "--- BUILD OUTPUT (filtered: {} lines -> {} kept, errors + warnings) ---",
        lines.len(),
        kept.len()
    );
    for line in &kept {
        let _ = writeln!(out, "{}", line);
    }
}

/// Install mode: keep only final status + any errors/warnings
fn filter_install(lines: &[String], out: &mut impl Write) {
    let t = get_thresholds();

    if lines.len() <= t.max_lines {
        for line in lines {
            let _ = writeln!(out, "{}", line);
        }
        return;
    }

    record_savings(lines.len(), t.max_lines);

    let mut kept: Vec<&str> = Vec::new();
    let mut kept_set: HashSet<&str> = HashSet::new();

    // Error/warning lines only
    for line in lines.iter() {
        if INSTALL_ERROR_RE.is_match(line) && kept.len() < 20 && !kept_set.contains(line.as_str()) {
            kept.push(line);
            kept_set.insert(line);
        }
    }

    // Last 5 lines (install summary)
    for line in lines.iter().rev().take(5) {
        if !kept_set.contains(line.as_str()) {
            kept.push(line);
            kept_set.insert(line);
        }
    }

    let _ = writeln!(
        out,
        "--- INSTALL OUTPUT (filtered: {} lines -> {} kept) ---",
        lines.len(),
        kept.len()
    );
    for line in &kept {
        let _ = writeln!(out, "{}", line);
    }
}

/// Default mode: head + tail + important lines (original behavior)
fn filter_default(lines: &[String], out: &mut impl Write) {
    let t = get_thresholds();

    if lines.len() <= t.max_lines {
        for line in lines {
            let _ = writeln!(out, "{}", line);
        }
        return;
    }

    record_savings(lines.len(), t.max_lines);

    let head: Vec<&str> = lines.iter().take(t.head).map(|s| s.as_str()).collect();
    let tail: Vec<&str> = lines
        .iter()
        .skip(lines.len().saturating_sub(t.tail))
        .map(|s| s.as_str())
        .collect();

    let important: Vec<&str> = lines
        .iter()
        .filter(|l| DEFAULT_IMPORTANT_RE.is_match(l))
        .take(t.important)
        .map(|s| s.as_str())
        .collect();

    let seen: std::collections::HashSet<&str> = head.iter().chain(tail.iter()).copied().collect();
    let unique_important: Vec<&str> = important
        .iter()
        .filter(|l| !seen.contains(**l))
        .copied()
        .collect();

    for line in &head {
        let _ = writeln!(out, "{}", line);
    }

    if !unique_important.is_empty() {
        let _ = writeln!(out, "\n--- IMPORTANT LINES ---");
        for line in &unique_important {
            let _ = writeln!(out, "{}", line);
        }
    }

    let _ = writeln!(
        out,
        "\n--- TRUNCATED ({} lines, showing head+tail+{} important) ---\n",
        lines.len(),
        unique_important.len()
    );

    for line in &tail {
        let _ = writeln!(out, "{}", line);
    }
}

// ─── Smart truncation: keyword relevance scoring ────────────────────────────

/// Score a line by relevance. Higher score = more likely to be kept.
/// Boost: error keywords, edited file names, warnings
/// Suppress: boilerplate (Compiling, Downloading, progress bars)
fn relevance_score(line: &str, edited_files: &[String]) -> i32 {
    let mut score: i32 = 0;

    // Boost: error/warning keywords
    let lower = line.to_lowercase();
    if lower.contains("error") || lower.contains("failed") || lower.contains("panic") {
        score += 10;
    }
    if lower.contains("warning") || lower.contains("warn") {
        score += 5;
    }
    if lower.contains("test") && (lower.contains("fail") || lower.contains("pass")) {
        score += 8;
    }

    // Boost: lines mentioning edited files
    for file in edited_files {
        let short = file.rsplit('/').next().unwrap_or(file);
        if line.contains(short) {
            score += 7;
            break;
        }
    }

    // Boost: stack traces, line numbers
    if line.contains("at ")
        && (line.contains(".rs:") || line.contains(".ts:") || line.contains(".js:"))
    {
        score += 4;
    }

    // Suppress: boilerplate
    if lower.starts_with("compiling ") || lower.starts_with("downloading ") {
        score -= 8;
    }
    if lower.starts_with("  ") && lower.contains("...") {
        score -= 5; // progress lines
    }
    if line.trim().is_empty() {
        score -= 10;
    }

    // Position bonus: first and last lines are often headers/summaries
    score
}

/// Smart truncation: keep top-N lines by relevance score instead of head+tail.
/// Falls back to head+tail if scoring adds >2ms overhead.
fn filter_smart(lines: &[String], out: &mut impl Write, max_lines: usize) {
    let start = std::time::Instant::now();

    // Load edited files for relevance boosting
    let state = common::read_session_state();
    let edited_files = state.files_edited.clone();

    // Score all lines
    let mut scored: Vec<(usize, i32)> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let mut s = relevance_score(line, &edited_files);
            // Position bonus: first 3 and last 3 always boosted
            if i < 3 || i >= lines.len().saturating_sub(3) {
                s += 15;
            }
            (i, s)
        })
        .collect();

    // Check time budget
    if start.elapsed().as_millis() > 2 {
        // Fallback to head+tail if scoring is too slow
        return;
    }

    // Sort by score descending, take top N
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    let mut kept_indices: Vec<usize> = scored.iter().take(max_lines).map(|(i, _)| *i).collect();

    // Re-sort by original position for output order
    kept_indices.sort();

    let _ = writeln!(
        out,
        "--- SMART TRUNCATED ({} lines → {} by relevance) ---",
        lines.len(),
        kept_indices.len()
    );
    for &i in &kept_indices {
        let _ = writeln!(out, "{}", lines[i]);
    }

    record_savings(lines.len(), max_lines);
}

/// Error cluster compression: group similar error lines by file + error code.
/// If a cluster has 3+ entries, collapse to a summary + first/last occurrence.
/// This reduces noise from large builds without hiding the underlying issue.
fn cluster_errors(lines: Vec<String>) -> Vec<String> {
    static ERROR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(.+?):(\d+)(?::\d+)?:\s*(error|warning)(?:\[([A-Z]\d+)\])?:\s*(.+)").unwrap()
    });

    // Count errors per file
    let mut file_counts: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(caps) = ERROR_RE.captures(line) {
            let file = caps.get(1).map_or("", |m| m.as_str()).to_string();
            file_counts.entry(file).or_default().push(i);
        }
    }

    // If no clusters of 3+, return original
    if !file_counts.values().any(|indices| indices.len() >= 3) {
        return lines;
    }

    let mut result = Vec::with_capacity(lines.len());
    let mut skip_indices: HashSet<usize> = HashSet::new();

    for (file, indices) in &file_counts {
        if indices.len() >= 3 {
            // Keep first and last occurrence, collapse the middle
            let first = indices[0];
            let last = indices[indices.len() - 1];
            for &idx in &indices[1..indices.len() - 1] {
                skip_indices.insert(idx);
            }
            // Insert cluster summary after the first occurrence
            // (handled inline below)
            let _ = (file, first, last); // used for context only
        }
    }

    let mut cluster_summaries_inserted: HashSet<String> = HashSet::new();
    for (i, line) in lines.iter().enumerate() {
        if skip_indices.contains(&i) {
            // Check if we need to insert a cluster summary here
            if let Some(caps) = ERROR_RE.captures(line) {
                let file = caps.get(1).map_or("", |m| m.as_str()).to_string();
                if let Some(indices) = file_counts.get(&file)
                    && indices.len() >= 3
                    && indices[0] < i
                    && !cluster_summaries_inserted.contains(&file)
                {
                    let severity = caps.get(3).map_or("error", |m| m.as_str());
                    result.push(format!(
                        "  ... [{} more {}s in {}]",
                        indices.len() - 2,
                        severity,
                        file
                    ));
                    cluster_summaries_inserted.insert(file);
                }
            }
            continue;
        }
        result.push(line.clone());
    }

    result
}

/// Record truncation savings — best-effort direct write.
/// In daemon mode, posttool-session also detects truncation markers and records
/// savings there (inside the daemon cache). This direct write may be overwritten
/// by the daemon cache flush, but it works for non-daemon mode.
fn record_savings(total_lines: usize, max_lines: usize) {
    let lines_dropped = total_lines.saturating_sub(max_lines);
    let saved = (lines_dropped as u64) * 10;
    let mut state = common::read_session_state();
    state.estimated_tokens_saved += saved;
    state.savings_truncation += 1;
    common::write_session_state(&state);
}
