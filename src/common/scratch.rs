// ─── common::scratch — large output offloading to scratch files ──────────────
//
// When tool output exceeds the offload threshold, the full content is written
// to a scratch file under .warden/scratch/. A short preview + file path is
// returned for injection into context, keeping the main context window clean.
// ──────────────────────────────────────────────────────────────────────────────

use super::io::project_dir;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Preview length: first N chars kept in context
const PREVIEW_LEN: usize = 500;

/// Max age for scratch files before cleanup (1 hour)
const MAX_AGE_SECS: u64 = 3600;

/// Scratch directory path (under project warden dir)
fn scratch_dir() -> PathBuf {
    project_dir().join("scratch")
}

/// Offload content to a scratch file. Returns (preview_with_path, scratch_path).
/// Creates the scratch directory if it doesn't exist.
pub fn offload(content: &str, tool_name: &str) -> Option<(String, PathBuf)> {
    let dir = scratch_dir();
    if fs::create_dir_all(&dir).is_err() {
        return None;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Sanitize tool name for filename
    let safe_name: String = tool_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .take(30)
        .collect();

    let filename = format!("{}_{}.txt", timestamp, safe_name);
    let path = dir.join(&filename);

    if fs::write(&path, content).is_err() {
        return None;
    }

    let size = content.len();
    let preview = if content.len() > PREVIEW_LEN {
        &content[..PREVIEW_LEN]
    } else {
        content
    };

    let message = format!(
        "{}\n\n[Full output offloaded: {} ({} bytes)]\n\
         Use Read tool with offset+limit to access specific sections.",
        preview,
        path.display(),
        size
    );

    Some((message, path))
}

/// Clean up scratch files older than MAX_AGE_SECS.
/// Fails silently — cleanup is best-effort.
pub fn cleanup_old() {
    let dir = scratch_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for entry in entries.flatten() {
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if now.saturating_sub(modified) > MAX_AGE_SECS {
            let _ = fs::remove_file(entry.path());
        }
    }
}
