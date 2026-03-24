// ─── pretool_bash::just — Just-first transform logic ─────────────────────────

use crate::common;
use crate::common::shell_parse;
use crate::rules;
use std::path::Path;
use std::sync::LazyLock;

pub enum JustResult {
    Transform(String),
    Deny(String),
    Advisory(String),
}

/// Try to transform command segments using JUST_MAP.
/// Uses shell_parse for quote-aware segment splitting.
/// Returns None if no segments matched any prefix.
pub fn try_just_transform(cmd: &str) -> Option<JustResult> {
    let mut segments = shell_parse::parse(cmd);

    let mut any_transform = false;

    for seg in &mut segments {
        let trimmed = seg.text.trim();
        if trimmed.is_empty() {
            continue;
        }

        for (prefix, recipe) in &rules::RULES.just_map {
            let prefix: &str = prefix;
            let recipe: &str = recipe;
            if !trimmed.starts_with(prefix) {
                continue;
            }

            // Docker compose — always deny (ambiguous mapping)
            if prefix == "docker compose" {
                return Some(JustResult::Deny(format!(
                    "Use a specific just docker-* recipe instead of `{}`",
                    common::truncate(trimmed, 60)
                )));
            }

            let rest = &trimmed[prefix.len()..];

            // Exact match (rest is empty or only whitespace)
            if rest.is_empty() || rest.trim().is_empty() {
                seg.text = recipe.to_string();
                any_transform = true;
                break;
            }

            // Has extra args — git commands passthrough, others get advisory (not deny)
            if trimmed.starts_with("git ") {
                break; // Passthrough
            }

            // Advisory instead of deny — extra args may be intentional
            return Some(JustResult::Advisory(format!(
                "Advisory: Consider using `{}` instead of `{}`. The just recipe may handle this better.",
                recipe,
                common::truncate(trimmed, 60)
            )));
        }
    }

    if any_transform {
        Some(JustResult::Transform(shell_parse::rejoin(&segments)))
    } else {
        None
    }
}

/// Check if a Justfile exists by searching from cwd upward (mirrors `just` lookup behavior)
pub fn justfile_exists() -> bool {
    static EXISTS: LazyLock<bool> = LazyLock::new(|| {
        let mut dir = std::env::current_dir().ok();
        while let Some(d) = dir {
            if d.join("Justfile").exists() || d.join("justfile").exists() {
                return true;
            }
            dir = d.parent().map(Path::to_path_buf);
        }
        false
    });
    *EXISTS
}
