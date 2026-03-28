// ─── Engine: Harbor — CLI: export-sessions ────────────────────────────────────
//
// Subcommand: warden export-sessions [--format json|csv] [--last N]
//
// Scans ~/.warden/projects/*/session-notes.jsonl for session-summary records.
// Falls back to basic counting for pre-upgrade sessions without structured data.
// ──────────────────────────────────────────────────────────────────────────────

use crate::common;
use std::fs;

pub fn run(args: &[String]) {
    let mut format = "json";
    let mut last_n: usize = 0; // 0 = all

    // Parse args: --format json|csv --last N
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" if i + 1 < args.len() => {
                format = if args[i + 1] == "csv" { "csv" } else { "json" };
                i += 2;
            }
            "--last" if i + 1 < args.len() => {
                last_n = args[i + 1].parse().unwrap_or(0);
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    let projects_dir = common::hooks_dir().join("projects");

    let mut summaries: Vec<serde_json::Value> = Vec::new();

    // Scan all project directories (read_dir handles non-existent dir gracefully)
    if let Ok(entries) = fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }

            let hash8 = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Read project name from breadcrumb
            let project_name = fs::read_to_string(dir.join("project.txt"))
                .unwrap_or_else(|_| hash8.clone())
                .trim()
                .to_string();

            // Read events — try redb first, fall back to JSONL
            let event_entries: Vec<serde_json::Value> = {
                common::storage::close();
                common::storage::open_db(&dir);
                let from_redb = if common::storage::is_available() {
                    common::storage::read_last_events(1000)
                        .iter()
                        .filter_map(|e| serde_json::from_slice(e).ok())
                        .collect::<Vec<serde_json::Value>>()
                } else {
                    Vec::new()
                };
                common::storage::close();
                if !from_redb.is_empty() {
                    from_redb
                } else {
                    let session_path = dir.join(crate::constants::SESSION_NOTES_FILE);
                    fs::read_to_string(&session_path)
                        .unwrap_or_default()
                        .lines()
                        .filter_map(|line| serde_json::from_str(line).ok())
                        .collect()
                }
            };

            if event_entries.is_empty() {
                continue;
            }

            let has_summary = event_entries
                .iter()
                .any(|e| e.get("type").and_then(|v| v.as_str()) == Some("session-summary"));

            // Extract session-summary records (structured data field)
            for entry in &event_entries {
                let note_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");

                if note_type == "session-summary" {
                    if let Some(data) = entry.get("data") {
                        let mut summary = data.clone();
                        if let Some(obj) = summary.as_object_mut() {
                            obj.insert("project".to_string(), serde_json::json!(project_name));
                            obj.insert("hash8".to_string(), serde_json::json!(hash8));
                            if let Some(ts) = entry.get("ts") {
                                obj.insert("ts".to_string(), ts.clone());
                            }
                        }
                        summaries.push(summary);
                    }
                } else if note_type == "session-end" {
                    // Fallback for pre-upgrade sessions: count basic stats from detail string
                    if !has_summary {
                        let detail = entry.get("detail").and_then(|v| v.as_str()).unwrap_or("");
                        let mut fallback = serde_json::json!({
                            "project": project_name,
                            "hash8": hash8,
                            "legacy": true,
                        });
                        if let Some(obj) = fallback.as_object_mut() {
                            if let Some(ts) = entry.get("ts") {
                                obj.insert("ts".to_string(), ts.clone());
                            }
                            // Parse "edits=N errors=N milestones=N" from detail
                            for part in detail.split_whitespace() {
                                if let Some((key, val)) = part.split_once('=') {
                                    match key {
                                        "edits" | "errors" | "milestones" => {
                                            if let Ok(n) = val.parse::<u32>() {
                                                obj.insert(key.to_string(), serde_json::json!(n));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        summaries.push(fallback);
                    }
                }
            }
        }
    }

    // Sort by timestamp descending
    summaries.sort_by(|a, b| {
        let ts_a = a.get("ts").and_then(|v| v.as_str()).unwrap_or("");
        let ts_b = b.get("ts").and_then(|v| v.as_str()).unwrap_or("");
        ts_b.cmp(ts_a)
    });

    // Apply --last N limit
    if last_n > 0 && summaries.len() > last_n {
        summaries.truncate(last_n);
    }

    match format {
        "csv" => print_csv(&summaries),
        _ => {
            println!(
                "{}",
                serde_json::to_string_pretty(&summaries).unwrap_or_else(|_| "[]".to_string())
            );
        }
    }
}

fn print_csv(summaries: &[serde_json::Value]) {
    println!(
        "ts,project,quality_score,turns,edits,errors,milestones,tokens_in,tokens_out,tokens_saved,savings_pct"
    );
    for s in summaries {
        println!(
            "{},{},{},{},{},{},{},{},{},{},{}",
            s.get("ts").and_then(|v| v.as_str()).unwrap_or(""),
            s.get("project").and_then(|v| v.as_str()).unwrap_or(""),
            s.get("quality_score").and_then(|v| v.as_u64()).unwrap_or(0),
            s.get("duration_turns")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            s.get("edits").and_then(|v| v.as_u64()).unwrap_or(0),
            s.get("errors").and_then(|v| v.as_u64()).unwrap_or(0),
            s.get("milestones").and_then(|v| v.as_u64()).unwrap_or(0),
            s.get("tokens_in").and_then(|v| v.as_u64()).unwrap_or(0),
            s.get("tokens_out").and_then(|v| v.as_u64()).unwrap_or(0),
            s.get("tokens_saved").and_then(|v| v.as_u64()).unwrap_or(0),
            s.get("savings_pct").and_then(|v| v.as_u64()).unwrap_or(0),
        );
    }
}
