// ─── replay — session timeline reconstruction + comparative diff ─────────────
//
// `warden replay [project-hash]` — reconstruct timeline from session-notes.jsonl
// `warden diff <hash-a> <hash-b>` — side-by-side session comparison
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use crate::constants;
use std::fs;

/// Run replay or diff subcommand
pub fn run(args: &[String]) {
    if args.is_empty() {
        // Replay current project's last session
        let project_dir = common::project_dir();
        replay_session(&project_dir);
        return;
    }

    let subcmd = args[0].as_str();
    match subcmd {
        "diff" if args.len() >= 3 => {
            diff_sessions(&args[1], &args[2]);
        }
        _ => {
            // Treat as project hash
            let projects_dir = common::hooks_dir().join("projects");
            let project_dir = projects_dir.join(subcmd);
            if project_dir.exists() {
                replay_session(&project_dir);
            } else {
                eprintln!("Project not found: {}", subcmd);
                eprintln!("Available projects:");
                list_projects(&projects_dir);
            }
        }
    }
}

/// Replay a session from session-notes.jsonl
fn replay_session(project_dir: &std::path::Path) {
    let notes_path = project_dir.join(constants::SESSION_NOTES_FILE);
    let content = match fs::read_to_string(&notes_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No session notes found at {}", notes_path.display());
            return;
        }
    };

    let project_name = fs::read_to_string(project_dir.join("project.txt"))
        .unwrap_or_else(|_| "unknown".to_string())
        .trim().to_string();

    println!("=== Session Replay: {} ===\n", project_name);

    // Find the last session-end to determine current session boundaries
    let lines: Vec<&str> = content.lines().collect();
    let session_start = lines.iter().rposition(|line| {
        serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|e| e.get("type")?.as_str().map(|s| s == "session-end"))
            .unwrap_or(false)
    }).map(|i| i + 1).unwrap_or(0);

    let session_lines = &lines[session_start..];

    let mut edits = 0u32;
    let mut errors = 0u32;
    let mut milestones = 0u32;
    let mut adaptations = Vec::new();
    let mut timeline = Vec::new();

    for line in session_lines {
        let entry = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let ts = entry.get("ts").and_then(|v| v.as_str()).unwrap_or("");
        let note_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let detail = entry.get("detail").and_then(|v| v.as_str()).unwrap_or("");

        match note_type {
            "edit" => {
                edits += 1;
                timeline.push(format!("  [{}] Edit: {}", short_ts(ts), detail));
            }
            "error" => {
                errors += 1;
                timeline.push(format!("  [{}] Error: {}", short_ts(ts), truncate(detail, 80)));
            }
            "milestone" => {
                milestones += 1;
                timeline.push(format!("  [{}] Milestone: {}", short_ts(ts), detail));
            }
            "adaptation" => {
                adaptations.push(detail.to_string());
                timeline.push(format!("  [{}] Phase: {}", short_ts(ts), detail));
            }
            "session-summary" => {
                if let Some(data) = entry.get("data") {
                    let quality = data.get("quality_score").and_then(|v| v.as_u64()).unwrap_or(0);
                    let turns = data.get("duration_turns").and_then(|v| v.as_u64()).unwrap_or(0);
                    println!("Quality: {}/100 | Turns: {} | Edits: {} | Errors: {} | Milestones: {}",
                        quality, turns, edits, errors, milestones);
                    println!();
                }
            }
            "session-end" => {
                timeline.push(format!("  [{}] Session ended: {}", short_ts(ts), detail));
            }
            _ => {}
        }
    }

    // Print timeline
    println!("Timeline ({} events):", timeline.len());
    for event in &timeline {
        println!("{}", event);
    }

    if !adaptations.is_empty() {
        println!("\nPhase transitions:");
        for a in &adaptations {
            println!("  {}", a);
        }
    }

    println!("\nSummary: {} edits, {} errors, {} milestones", edits, errors, milestones);
}

/// Diff two sessions side by side
fn diff_sessions(hash_a: &str, hash_b: &str) {
    let projects_dir = common::hooks_dir().join("projects");
    let dir_a = projects_dir.join(hash_a);
    let dir_b = projects_dir.join(hash_b);

    if !dir_a.exists() || !dir_b.exists() {
        eprintln!("One or both project hashes not found.");
        list_projects(&projects_dir);
        return;
    }

    println!("=== Session Diff ===\n");
    println!("{:<30} | {}", hash_a, hash_b);
    println!("{}", "-".repeat(65));

    let stats_a = load_session_summary(&dir_a);
    let stats_b = load_session_summary(&dir_b);

    let fields = [
        ("Quality", stats_a.get("quality_score"), stats_b.get("quality_score")),
        ("Turns", stats_a.get("duration_turns"), stats_b.get("duration_turns")),
        ("Edits", stats_a.get("edits"), stats_b.get("edits")),
        ("Errors", stats_a.get("errors"), stats_b.get("errors")),
        ("Milestones", stats_a.get("milestones"), stats_b.get("milestones")),
        ("Tokens saved", stats_a.get("tokens_saved"), stats_b.get("tokens_saved")),
    ];

    for (name, a, b) in &fields {
        let va = a.and_then(|v| v.as_u64()).unwrap_or(0);
        let vb = b.and_then(|v| v.as_u64()).unwrap_or(0);
        let indicator = if va > vb { ">" } else if va < vb { "<" } else { "=" };
        println!("{:<15} {:>10} {} {:<10}", name, va, indicator, vb);
    }
}

fn load_session_summary(project_dir: &std::path::Path) -> serde_json::Value {
    let notes_path = project_dir.join(constants::SESSION_NOTES_FILE);
    let content = fs::read_to_string(&notes_path).unwrap_or_default();
    for line in content.lines().rev() {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line)
            && entry.get("type").and_then(|v| v.as_str()) == Some("session-summary")
                && let Some(data) = entry.get("data") {
                    return data.clone();
                }
    }
    serde_json::Value::Object(serde_json::Map::new())
}

fn list_projects(projects_dir: &std::path::Path) {
    if let Ok(entries) = fs::read_dir(projects_dir) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if dir.is_dir() {
                let hash = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
                let name = fs::read_to_string(dir.join("project.txt"))
                    .unwrap_or_else(|_| "unknown".to_string())
                    .trim().to_string();
                eprintln!("  {} — {}", hash, name);
            }
        }
    }
}

fn short_ts(ts: &str) -> &str {
    // "2026-03-24T10:30:00Z" → "10:30:00"
    if ts.len() >= 19 { &ts[11..19] } else { ts }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}...", &s[..max]) }
}
