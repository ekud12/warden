// ─── smart_filter — per-command intelligent output compression ─────────────
//
// Inspired by RTK (Rust Token Killer). Instead of generic head+tail truncation,
// applies command-specific filters that understand output structure.
//
// Architecture:
//   1. Detect command type from the command string
//   2. Apply specialized filter that preserves semantically important lines
//   3. Deduplicate, group, and compress remaining output
//   4. Fall back to generic truncation for unknown commands
//
// Strategies (applied per filter):
//   - Deduplication: collapse repeated lines with count
//   - Grouping: aggregate similar entries (e.g., "3 files changed")
//   - Structural preservation: keep headers, errors, summaries
//   - Whitespace/noise removal: strip blank lines, progress bars, spinners
//
// Integration: called from pretool_bash truncation setup when wrapping commands.
// ──────────────────────────────────────────────────────────────────────────────

use regex::Regex;
use std::sync::LazyLock;

/// Filter result: compressed output + metadata
pub struct FilterResult {
    pub output: String,
    pub original_lines: usize,
    pub kept_lines: usize,
    pub filter_name: &'static str,
}

impl FilterResult {
    pub fn compression_pct(&self) -> u32 {
        if self.original_lines == 0 { return 0; }
        ((self.original_lines - self.kept_lines) * 100 / self.original_lines) as u32
    }
}

/// Detect command type and apply the best filter. Returns None if no
/// specialized filter matches (caller should use generic truncation).
pub fn filter_output(cmd: &str, output: &str, max_lines: usize) -> Option<FilterResult> {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        return None; // No filtering needed
    }

    let original_lines = lines.len();

    // Match command to specialized filter
    let (filtered, filter_name) = if is_cargo_test(cmd) {
        (filter_cargo_test(&lines, max_lines), "cargo-test")
    } else if is_cargo_build(cmd) {
        (filter_cargo_build(&lines, max_lines), "cargo-build")
    } else if is_git_diff(cmd) {
        (filter_git_diff(&lines, max_lines), "git-diff")
    } else if is_git_log(cmd) {
        (filter_git_log(&lines, max_lines), "git-log")
    } else if is_npm_install(cmd) {
        (filter_npm_install(&lines, max_lines), "npm-install")
    } else if is_test_runner(cmd) {
        (filter_test_output(&lines, max_lines), "test-runner")
    } else if is_lint(cmd) {
        (filter_lint(&lines, max_lines), "lint")
    } else if is_ls_tree(cmd) {
        (filter_listing(&lines, max_lines), "listing")
    } else {
        // Generic: dedup + noise removal + head/tail
        (filter_generic(&lines, max_lines), "generic")
    };

    let kept_lines = filtered.lines().count();
    Some(FilterResult {
        output: filtered,
        original_lines,
        kept_lines,
        filter_name,
    })
}

// ── Command detection ────────────────────────────────────────────────────────

fn is_cargo_test(cmd: &str) -> bool { cmd.contains("cargo test") || cmd.contains("cargo nextest") }
fn is_cargo_build(cmd: &str) -> bool {
    cmd.contains("cargo build") || cmd.contains("cargo check") || cmd.contains("cargo clippy")
}
fn is_git_diff(cmd: &str) -> bool { cmd.contains("git diff") }
fn is_git_log(cmd: &str) -> bool { cmd.contains("git log") }
fn is_npm_install(cmd: &str) -> bool {
    cmd.contains("npm install") || cmd.contains("npm ci") || cmd.contains("pnpm install") || cmd.contains("yarn install")
}
fn is_test_runner(cmd: &str) -> bool {
    cmd.contains("pytest") || cmd.contains("vitest") || cmd.contains("jest")
        || cmd.contains("npm test") || cmd.contains("pnpm test")
        || cmd.contains("go test") || cmd.contains("dotnet test")
}
fn is_lint(cmd: &str) -> bool {
    cmd.contains("eslint") || cmd.contains("biome") || cmd.contains("ruff")
        || cmd.contains("pylint") || cmd.contains("mypy") || cmd.contains("clippy")
}
fn is_ls_tree(cmd: &str) -> bool {
    cmd.starts_with("ls ") || cmd.starts_with("eza ") || cmd.starts_with("tree ")
        || cmd.starts_with("fd ") || cmd.starts_with("find ")
}

// ── Specialized filters ──────────────────────────────────────────────────────

/// cargo test: strip passing tests, keep failures + summary
fn filter_cargo_test(lines: &[&str], max: usize) -> String {
    let mut kept: Vec<&str> = Vec::with_capacity(max);
    let mut pass_count = 0u32;
    let mut fail_count = 0u32;

    for &line in lines {
        let trimmed = line.trim();
        // Always keep: failure lines, summary, warnings, errors
        if trimmed.contains("FAILED") || trimmed.contains("failures:")
            || trimmed.contains("panicked") || trimmed.contains("error[")
            || trimmed.starts_with("test result:")
            || trimmed.starts_with("failures:")
            || trimmed.starts_with("---- ") // failure section headers
        {
            if trimmed.contains("FAILED") { fail_count += 1; }
            kept.push(line);
        } else if trimmed.ends_with("... ok") || trimmed.ends_with("... ignored") {
            pass_count += 1;
            // Skip — passing tests are noise
        } else if !trimmed.is_empty() {
            kept.push(line);
        }

        if kept.len() >= max { break; }
    }

    let mut out = String::with_capacity(kept.len() * 60);
    out.push_str(&format!("--- cargo test ({} passed, {} failed, showing failures + summary) ---\n",
        pass_count, fail_count));
    for line in &kept {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// cargo build/check/clippy: strip "Compiling" lines, keep errors + warnings + final
fn filter_cargo_build(lines: &[&str], max: usize) -> String {
    let mut kept: Vec<&str> = Vec::with_capacity(max);
    let mut compiling_count = 0u32;
    let mut warning_count = 0u32;
    let mut error_count = 0u32;

    for &line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("Compiling ") || trimmed.starts_with("Downloading ") || trimmed.starts_with("Downloaded ") {
            compiling_count += 1;
        } else if trimmed.contains("error[") || trimmed.starts_with("error:") || trimmed.starts_with("error ->") {
            error_count += 1;
            kept.push(line);
        } else if trimmed.contains("warning:") || trimmed.starts_with("warning[") {
            warning_count += 1;
            if warning_count <= 20 { kept.push(line); } // Cap warnings
        } else if trimmed.starts_with("Finished") || trimmed.starts_with("error: could not compile")
            || trimmed.contains("generated") || trimmed.starts_with("For more information")
        {
            kept.push(line);
        } else if trimmed.starts_with("-->") || trimmed.starts_with(" |") {
            // Source location context for errors/warnings
            kept.push(line);
        } else if !trimmed.is_empty() && kept.len() < max {
            kept.push(line);
        }

        if kept.len() >= max { break; }
    }

    let mut out = String::with_capacity(kept.len() * 60);
    out.push_str(&format!("--- cargo build ({} compiled, {} errors, {} warnings) ---\n",
        compiling_count, error_count, warning_count));
    for line in &kept {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// git diff: keep file headers + changed lines, collapse large hunks
fn filter_git_diff(lines: &[&str], max: usize) -> String {
    let mut kept: Vec<&str> = Vec::with_capacity(max);
    let mut files_changed = 0u32;
    let mut in_large_hunk = false;
    let mut hunk_lines_skipped = 0u32;

    for &line in lines {
        if line.starts_with("diff --git") {
            files_changed += 1;
            in_large_hunk = false;
            if hunk_lines_skipped > 0 {
                kept.push(""); // placeholder, we'll format it
                hunk_lines_skipped = 0;
            }
            kept.push(line);
        } else if line.starts_with("@@") || line.starts_with("---") || line.starts_with("+++") {
            in_large_hunk = false;
            kept.push(line);
        } else if (line.starts_with('+') || line.starts_with('-')) && !in_large_hunk {
            kept.push(line);
            if kept.len() > max / 2 {
                in_large_hunk = true; // start skipping context in remaining files
            }
        } else if in_large_hunk && (line.starts_with('+') || line.starts_with('-')) {
            hunk_lines_skipped += 1;
        } else if line.starts_with(' ') {
            // Context lines — keep first 2 per hunk, skip rest
            if kept.last().map(|l| l.starts_with(' ')).unwrap_or(false) {
                // Already have context, skip
            } else {
                kept.push(line);
            }
        }

        if kept.len() >= max { break; }
    }

    let mut out = String::with_capacity(kept.len() * 80);
    out.push_str(&format!("--- git diff ({} files, compressed) ---\n", files_changed));
    for line in &kept {
        out.push_str(line);
        out.push('\n');
    }
    if hunk_lines_skipped > 0 {
        out.push_str(&format!("  ... {} more change lines in remaining files\n", hunk_lines_skipped));
    }
    out
}

/// git log: keep commit headers + first line of message, collapse details
fn filter_git_log(lines: &[&str], max: usize) -> String {
    let mut kept: Vec<&str> = Vec::with_capacity(max);
    let mut commit_count = 0u32;

    for &line in lines {
        let trimmed = line.trim();
        if line.starts_with("commit ") || line.starts_with("Author:") || line.starts_with("Date:") {
            kept.push(line);
            if line.starts_with("commit ") { commit_count += 1; }
        } else if !trimmed.is_empty() && kept.last().map(|l| l.starts_with("Date:")).unwrap_or(false) {
            // First non-empty line after Date = commit message subject
            kept.push(line);
        } else if line.starts_with("Merge:") {
            kept.push(line);
        }
        // Skip: full commit bodies, empty lines between commits

        if kept.len() >= max { break; }
    }

    let mut out = String::with_capacity(kept.len() * 80);
    out.push_str(&format!("--- git log ({} commits, headers + subjects) ---\n", commit_count));
    for line in &kept {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// npm/pnpm/yarn install: keep warnings + final summary, skip progress
fn filter_npm_install(lines: &[&str], max: usize) -> String {
    static PROGRESS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(npm http|fetch |GET |reify|idealTree|timing|added \d+ packages in)").unwrap()
    });

    let mut kept: Vec<&str> = Vec::with_capacity(max);
    let mut skipped = 0u32;

    for &line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || PROGRESS_RE.is_match(trimmed) {
            skipped += 1;
        } else {
            kept.push(line);
        }

        if kept.len() >= max { break; }
    }

    let mut out = String::with_capacity(kept.len() * 60);
    out.push_str(&format!("--- install ({} progress lines skipped) ---\n", skipped));
    for line in &kept {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Generic test runner (pytest, vitest, jest, etc.): keep failures + summary
fn filter_test_output(lines: &[&str], max: usize) -> String {
    let mut kept: Vec<&str> = Vec::with_capacity(max);
    let mut pass_count = 0u32;

    for &line in lines {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.contains("fail") || lower.contains("error") || lower.contains("✗")
            || lower.contains("✘") || lower.contains("assert")
            || lower.contains("expected") || lower.contains("actual")
        {
            kept.push(line);
        } else if lower.contains("pass") || lower.contains("✓") || lower.contains("✔") {
            pass_count += 1;
            // Skip passing tests
        } else if lower.contains("test") && (lower.contains("suite") || lower.contains("total")
            || lower.contains("result") || lower.contains("ran"))
        {
            kept.push(line); // Summary lines
        } else if !trimmed.is_empty() && kept.len() < max {
            kept.push(line);
        }

        if kept.len() >= max { break; }
    }

    let mut out = String::with_capacity(kept.len() * 60);
    out.push_str(&format!("--- tests ({} passed, showing failures + summary) ---\n", pass_count));
    for line in &kept {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Lint output (eslint, biome, ruff, clippy): group by file, cap per file
fn filter_lint(lines: &[&str], max: usize) -> String {
    let mut kept: Vec<&str> = Vec::with_capacity(max);
    let mut current_file = "";
    let mut file_line_count = 0u32;
    let max_per_file = 10;

    for &line in lines {
        // Detect file headers (eslint: "/path/file.ts", biome: "path/file.ts lint")
        if (line.starts_with('/') || line.starts_with("./") || line.contains(":\\"))
            && !line.contains("error") && !line.contains("warning")
        {
            current_file = line;
            file_line_count = 0;
            kept.push(line);
        } else if file_line_count < max_per_file {
            kept.push(line);
            file_line_count += 1;
        }
        // Skip lint lines beyond cap per file

        if kept.len() >= max { break; }
    }

    let mut out = String::with_capacity(kept.len() * 80);
    out.push_str(&format!("--- lint (capped to {} lines/file) ---\n", max_per_file));
    for line in &kept {
        out.push_str(line);
        out.push('\n');
    }
    let _ = current_file; // suppress unused warning
    out
}

/// Directory listing: dedup similar entries, collapse deep paths
fn filter_listing(lines: &[&str], max: usize) -> String {
    let mut kept: Vec<&str> = Vec::with_capacity(max);

    // Keep first `max` lines, skip empty
    for &line in lines {
        if !line.trim().is_empty() {
            kept.push(line);
        }
        if kept.len() >= max { break; }
    }

    let remaining = lines.len().saturating_sub(max);
    let mut out = String::with_capacity(kept.len() * 60);
    for line in &kept {
        out.push_str(line);
        out.push('\n');
    }
    if remaining > 0 {
        out.push_str(&format!("  ... {} more entries\n", remaining));
    }
    out
}

/// Generic filter: dedup repeated lines + noise removal + head/tail
fn filter_generic(lines: &[&str], max: usize) -> String {
    // Phase 1: Dedup consecutive repeated lines
    let mut deduped: Vec<String> = Vec::with_capacity(lines.len());
    let mut last_line = "";
    let mut repeat_count = 0u32;

    for &line in lines {
        if line == last_line {
            repeat_count += 1;
        } else {
            if repeat_count > 1 {
                deduped.push(format!("  ... repeated {} times", repeat_count));
            }
            last_line = line;
            repeat_count = 1;

            // Phase 2: Skip noise lines
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("npm http")
                || trimmed.starts_with("  ")
                    && trimmed.contains("...")
            {
                continue;
            }
            deduped.push(line.to_string());
        }
    }
    if repeat_count > 1 {
        deduped.push(format!("  ... repeated {} times", repeat_count));
    }

    // Phase 3: If still too long, head + tail
    if deduped.len() <= max {
        return deduped.join("\n");
    }

    let head = max / 3;
    let tail = max / 3;
    let mut out = String::with_capacity(max * 60);

    for line in deduped.iter().take(head) {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&format!("\n  ... {} lines omitted ...\n\n",
        deduped.len() - head - tail));
    for line in deduped.iter().skip(deduped.len().saturating_sub(tail)) {
        out.push_str(line);
        out.push('\n');
    }
    out
}
