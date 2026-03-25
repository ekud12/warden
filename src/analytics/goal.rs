// ─── analytics::goal — session goal extraction + coherence ───────────────────
//
// Extracts user intent from the first message and re-injects as a grounding
// anchor. Tracks topic coherence by comparing current file working set
// against the initial working set.
// ──────────────────────────────────────────────────────────────────────────────

use regex::Regex;
use std::sync::LazyLock;

static GOAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(fix|add|refactor|implement|build|create|update|debug|test|write|remove|delete|move|rename|migrate|upgrade|downgrade|setup|configure|deploy|optimize|improve|clean)\s+(.{5,80})").unwrap()
});

/// Extract a session goal from user text (first message typically)
pub fn extract_goal(text: &str) -> Option<String> {
    // Try structured extraction first
    if let Some(caps) = GOAL_RE.captures(text) {
        let verb = caps.get(1)?.as_str().to_lowercase();
        let target = caps.get(2)?.as_str().trim();
        // Truncate at sentence boundary or 80 chars
        let target = target.split('.').next().unwrap_or(target);
        let target = target.split('\n').next().unwrap_or(target);
        return Some(format!("{} {}", verb, target.trim()));
    }

    // Fallback: first sentence if it's short enough
    let first_line = text.lines().next().unwrap_or(text).trim();
    if first_line.len() >= 10 && first_line.len() <= 100 {
        return Some(first_line.to_string());
    }

    None
}

/// Format the goal re-anchoring message
pub fn format_anchor(goal: &str, turn: u32) -> String {
    format!("Session goal: \"{}\". (Turn {})", goal, turn)
}

/// Compute topic coherence: Jaccard similarity between initial and current file sets
/// Returns (similarity 0.0-1.0, list of drifted dirs)
pub fn topic_coherence(initial_set: &[String], current_files: &[String]) -> (f64, Vec<String>) {
    if initial_set.is_empty() || current_files.is_empty() {
        return (1.0, Vec::new());
    }

    // Extract directory components from current files
    let current_dirs: std::collections::HashSet<String> = current_files
        .iter()
        .filter_map(|f| {
            let normalized = f.replace('\\', "/");
            normalized.rsplit('/').nth(1).map(|d| d.to_string())
        })
        .collect();

    let initial_dirs: std::collections::HashSet<&String> = initial_set.iter().collect();

    // Jaccard similarity
    let intersection = current_dirs
        .iter()
        .filter(|d| initial_dirs.contains(d))
        .count();
    let union = initial_dirs.len() + current_dirs.len() - intersection;

    let similarity = if union > 0 {
        intersection as f64 / union as f64
    } else {
        1.0
    };

    // Find drifted directories (in current but not in initial)
    let drifted: Vec<String> = current_dirs
        .into_iter()
        .filter(|d| !initial_dirs.contains(d))
        .take(3)
        .collect();

    (similarity, drifted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_fix_goal() {
        let goal = extract_goal("fix the authentication middleware in src/auth");
        assert_eq!(
            goal,
            Some("fix the authentication middleware in src/auth".to_string())
        );
    }

    #[test]
    fn extract_add_goal() {
        let goal = extract_goal("add retry logic to the API client");
        assert_eq!(goal, Some("add retry logic to the API client".to_string()));
    }

    #[test]
    fn no_goal_from_short_text() {
        let goal = extract_goal("hi");
        assert!(goal.is_none());
    }

    #[test]
    fn coherence_same_dirs() {
        // Initial set is directory names (e.g., "auth", "api"), extracted the same way
        let initial = vec!["auth".to_string(), "api".to_string()];
        let current = vec![
            "src/auth/middleware.rs".to_string(),
            "src/api/client.rs".to_string(),
        ];
        let (sim, _) = topic_coherence(&initial, &current);
        assert!(sim > 0.5, "same dirs should have high similarity: {}", sim);
    }

    #[test]
    fn coherence_different_dirs() {
        let initial = vec!["auth".to_string()];
        let current = vec![
            "tests/integration/test_db.rs".to_string(),
            "docs/readme.md".to_string(),
        ];
        let (sim, drifted) = topic_coherence(&initial, &current);
        assert!(
            sim < 0.5,
            "different dirs should have low similarity: {}",
            sim
        );
        assert!(!drifted.is_empty());
    }
}
