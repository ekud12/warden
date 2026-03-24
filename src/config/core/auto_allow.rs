// ─── core::auto_allow — safe read-only commands (bypass permission prompt) ───

/// Commands matching these patterns are auto-approved without user confirmation.
/// Only genuinely read-only, side-effect-free commands should be here.
pub const AUTO_ALLOW: &[&str] = &[
    // Modern CLI tools (read-only by default)
    r"^\s*bat\s",
    r"^\s*rg\s",
    r"^\s*fd\s",
    r"^\s*eza\s",
    r"^\s*dust\s",
    r"^\s*tokei\b",
    r"^\s*procs\b",
    r"^\s*huniq\b",
    r"^\s*mdq\s",
    r"^\s*jq\s",
    r"^\s*yq\s",
    r"^\s*glow\s",
    r"^\s*tldr\s",
    r"^\s*choose\s",
    r"^\s*grex\s",
    r"^\s*lychee\s",
    r"^\s*typos\s",
    r"^\s*shellcheck\s",
    r"^\s*jc\s",
    r"^\s*ouch\s+l", // ouch list (read-only)
    // Standard read-only
    r"^\s*wc\s",
    r"^\s*head\s",
    r"^\s*tail\s",
    r"^\s*diff\s",
    r"^\s*env\b",
    r"^\s*which\s",
    r"^\s*where\s",
    r"^\s*type\s",
    r"^\s*file\s",
    r"^\s*stat\s",
    r"^\s*echo\s",
    r"^\s*printf\s",
    r"^\s*date\b",
    r"^\s*whoami\b",
    r"^\s*hostname\b",
    r"^\s*uname\b",
    r"^\s*pwd\b",
    r"^\s*id\b",
    r"^\s*uptime\b",
    r"^\s*free\b",
    r"^\s*df\s",
    // Version checks
    r"--version$",
    r"-[vV]$",
    // Git read-only
    r"^\s*git\s+(status|log|diff|show|branch|remote|blame|shortlog|stash\s+list|stash\s+show|rev-parse|describe|tag\s*$)\b",
    r"^\s*git\s+log\b",
    // Build/test (auto-approve — they don't modify source)
    r"^\s*cargo\s+(build|check|test|clippy|fmt\s+--check|doc|bench)\b",
    r"^\s*npm\s+(test|run\s+(build|lint|check|test|dev|start))\b",
    r"^\s*pnpm\s+(test|run|exec)\b",
    r"^\s*yarn\s+(test|run|build)\b",
    r"^\s*bun\s+(test|run|build)\b",
    r"^\s*dotnet\s+(build|test|publish|restore|run)\b",
    r"^\s*python[23]?\s+-m\s+(pytest|unittest)\b",
    r"^\s*pytest\b",
    r"^\s*just\s",
    r"^\s*make\s+(test|build|check|lint)\b",
    // Package info
    r"^\s*npm\s+(ls|list|outdated|info|view|why)\b",
    r"^\s*cargo\s+(tree|metadata|search)\b",
    // Go toolchain
    r"^\s*go\s+(build|test|vet|fmt|run|mod\s+(tidy|download|verify))\b",
    // Java/Kotlin toolchain
    r"^\s*mvn\s+(test|compile|verify|package|clean)\b",
    r"^\s*gradle\s+(build|test|check|clean|assemble)\b",
    // Deno
    r"^\s*deno\s+(test|check|lint|fmt\s+--check|bench|compile)\b",
    // Ruby
    r"^\s*bundle\s+(exec|install|check)\b",
    r"^\s*rake\s+(test|spec|lint)\b",
    // PHP
    r"^\s*composer\s+(install|update|dump-autoload)\b",
    r"^\s*phpunit\b",
    // Swift
    r"^\s*swift\s+(build|test|package)\b",
    // Warden self-calls
    r"^\s*warden\s",
];
