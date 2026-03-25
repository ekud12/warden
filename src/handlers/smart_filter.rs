// ─── smart_filter — data-driven command output compression engine ─────────────
//
// Generic filter engine that applies per-command rules to compress output.
// Rules come from two sources:
//   1. Compiled defaults (config/core/filters.rs)
//   2. User TOML overrides ([[command_filters]] in personal.toml or project rules.toml)
//
// Strategies:
//   strip_matching  — remove lines matching strip_patterns, keep the rest
//   keep_matching   — only keep lines matching keep_patterns + first/last N
//   dedup           — collapse consecutive identical lines with count
//   head_tail       — keep first N + last N
//   passthrough     — no filtering
// ──────────────────────────────────────────────────────────────────────────────

use crate::config::core::filters;
use crate::rules::schema::CommandFilter;
use regex::Regex;
use std::sync::LazyLock;

/// Compiled filter rules (loaded once from defaults + TOML overrides)
static FILTER_RULES: LazyLock<Vec<CompiledFilter>> = LazyLock::new(|| {
    let mut rules = filters::defaults();
    // Append user TOML overrides (from merged rules)
    rules.extend(crate::rules::RULES.command_filters.clone());
    rules.into_iter().filter_map(compile_filter).collect()
});

struct CompiledFilter {
    cmd_match: Regex,
    keep: Vec<Regex>,
    strip: Vec<Regex>,
    keep_first: usize,
    keep_last: usize,
    summary_template: String,
    max_lines: usize,
    strategy: Strategy,
}

#[derive(Clone, Copy)]
enum Strategy {
    StripMatching,
    KeepMatching,
    Dedup,
    HeadTail,
    Passthrough,
}

fn compile_filter(rule: CommandFilter) -> Option<CompiledFilter> {
    let cmd_match = Regex::new(&rule.cmd_match).ok()?;
    let keep: Vec<Regex> = rule
        .keep_patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();
    let strip: Vec<Regex> = rule
        .strip_patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();
    let strategy = match rule.strategy.as_str() {
        "keep_matching" => Strategy::KeepMatching,
        "dedup" => Strategy::Dedup,
        "head_tail" => Strategy::HeadTail,
        "passthrough" => Strategy::Passthrough,
        _ => Strategy::StripMatching,
    };
    Some(CompiledFilter {
        cmd_match,
        keep,
        strip,
        keep_first: rule.keep_first,
        keep_last: rule.keep_last,
        summary_template: rule.summary_template,
        max_lines: rule.max_lines,
        strategy,
    })
}

/// Filter result: compressed output + metadata
pub struct FilterResult {
    pub output: String,
    pub original_lines: usize,
    pub kept_lines: usize,
    pub filter_name: &'static str,
}

/// Apply the best matching filter rule to command output.
/// Returns None if no filter matches or output is short enough.
pub fn filter_output(cmd: &str, output: &str, max_lines: usize) -> Option<FilterResult> {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        return None;
    }
    let original_lines = lines.len();

    // Find first matching filter rule
    let filter = FILTER_RULES.iter().find(|f| f.cmd_match.is_match(cmd));

    let (filtered, name) = if let Some(f) = filter {
        let max = f.max_lines.min(max_lines);
        let result = apply_filter(f, &lines, max);
        (result, "data-driven")
    } else {
        // Generic fallback: dedup + head/tail
        (filter_generic(&lines, max_lines), "generic")
    };

    let kept_lines = filtered.lines().count();
    Some(FilterResult {
        output: filtered,
        original_lines,
        kept_lines,
        filter_name: name,
    })
}

/// Apply a compiled filter rule to lines
fn apply_filter(f: &CompiledFilter, lines: &[&str], max: usize) -> String {
    match f.strategy {
        Strategy::StripMatching => apply_strip_matching(f, lines, max),
        Strategy::KeepMatching => apply_keep_matching(f, lines, max),
        Strategy::Dedup => apply_dedup(lines, max),
        Strategy::HeadTail => apply_head_tail(lines, f.keep_first, f.keep_last, max),
        Strategy::Passthrough => lines.join("\n"),
    }
}

/// Strip lines matching strip_patterns, keep everything else + keep_pattern matches
fn apply_strip_matching(f: &CompiledFilter, lines: &[&str], max: usize) -> String {
    let mut kept: Vec<&str> = Vec::with_capacity(max);
    let mut stripped = 0usize;

    // Always keep first N
    for &line in lines.iter().take(f.keep_first) {
        kept.push(line);
    }

    // Process middle lines
    for &line in lines
        .iter()
        .skip(f.keep_first)
        .take(lines.len().saturating_sub(f.keep_first + f.keep_last))
    {
        let should_strip = !line.trim().is_empty() && f.strip.iter().any(|re| re.is_match(line));
        let should_keep = f.keep.iter().any(|re| re.is_match(line));

        if should_keep || !should_strip {
            kept.push(line);
        } else {
            stripped += 1;
        }
        if kept.len() >= max {
            break;
        }
    }

    // Always keep last N
    for &line in lines.iter().skip(lines.len().saturating_sub(f.keep_last)) {
        if kept.len() < max {
            kept.push(line);
        }
    }

    format_output(&f.summary_template, &kept, lines.len(), stripped)
}

/// Only keep lines matching keep_patterns + first/last N
fn apply_keep_matching(f: &CompiledFilter, lines: &[&str], max: usize) -> String {
    let mut kept: Vec<&str> = Vec::with_capacity(max);

    // First N
    for &line in lines.iter().take(f.keep_first) {
        kept.push(line);
    }

    // Lines matching keep patterns
    for &line in lines.iter() {
        if f.keep.iter().any(|re| re.is_match(line)) {
            kept.push(line);
        }
        if kept.len() >= max {
            break;
        }
    }

    // Last N
    for &line in lines.iter().skip(lines.len().saturating_sub(f.keep_last)) {
        if kept.len() < max {
            kept.push(line);
        }
    }

    let stripped = lines.len() - kept.len();
    format_output(&f.summary_template, &kept, lines.len(), stripped)
}

/// Collapse consecutive identical lines
fn apply_dedup(lines: &[&str], max: usize) -> String {
    let mut kept: Vec<String> = Vec::with_capacity(max);
    let mut last = "";
    let mut count = 0u32;

    for &line in lines {
        if line == last {
            count += 1;
        } else {
            if count > 1 {
                kept.push(format!("  ... repeated {} times", count));
            }
            if !line.trim().is_empty() {
                kept.push(line.to_string());
            }
            last = line;
            count = 1;
        }
        if kept.len() >= max {
            break;
        }
    }
    if count > 1 {
        kept.push(format!("  ... repeated {} times", count));
    }
    kept.join("\n")
}

/// Keep first N + last N lines
fn apply_head_tail(lines: &[&str], head: usize, tail: usize, max: usize) -> String {
    let head = head.min(max / 2);
    let tail = tail.min(max / 2);
    let mut out = String::with_capacity(max * 60);

    for &line in lines.iter().take(head) {
        out.push_str(line);
        out.push('\n');
    }

    let omitted = lines.len().saturating_sub(head + tail);
    if omitted > 0 {
        out.push_str(&format!("  ... {} lines omitted ...\n", omitted));
    }

    for &line in lines.iter().skip(lines.len().saturating_sub(tail)) {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Generic fallback: dedup + head/tail
fn filter_generic(lines: &[&str], max: usize) -> String {
    apply_dedup(lines, max)
}

/// Format output with summary header
fn format_output(template: &str, kept: &[&str], total: usize, stripped: usize) -> String {
    let summary = template
        .replace("{kept}", &kept.len().to_string())
        .replace("{total}", &total.to_string())
        .replace("{stripped}", &stripped.to_string());

    let mut out = String::with_capacity(kept.len() * 60 + summary.len() + 10);
    if !summary.is_empty() {
        out.push_str(&format!("--- {} ---\n", summary));
    }
    for line in kept {
        out.push_str(line);
        out.push('\n');
    }
    out
}
