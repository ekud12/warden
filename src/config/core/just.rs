// ─── config::just — Just recipe tables and truncation control ─────────────────

/// Just-first transform table: (raw prefix, just recipe)
/// Sorted longest prefix first — order is critical for matching.
pub const JUST_MAP: &[(&str, &str)] = &[
    // git (most specific first)
    ("git diff --cached --name-only", "just changed-files-staged"),
    ("git diff --cached --stat", "just diff-stat"),
    ("git diff --name-only", "just changed-files"),
    ("git diff --stat", "just diff-stat"),
    ("git diff --cached", "just diff-staged"),
    ("git diff", "just diff"),
    ("git show", "just show"),
    ("git log", "just log-compact"),
    ("git status", "just status"),
    ("git branch", "just branches"),
    ("git remote", "just remotes"),
    // frontend
    ("pnpm build", "just build"),
    ("pnpm test", "just test"),
    ("pnpm lint", "just lint"),
    ("pnpm install", "just install"),
    ("pnpm typecheck", "just typecheck"),
    ("pnpm align", "just align"),
    ("pnpm dev", "just dev"),
    ("pnpm preview", "just preview"),
    ("pnpm format", "just format"),
    // health
    ("npx knip", "just deadcode"),
    ("npx madge", "just circular"),
    ("npx tsc --noEmit", "just tsc-check"),
    ("npx depcheck", "just depcheck"),
    ("tokei", "just tokei"),
    // .NET
    ("dotnet build", "just dotnet-build"),
    ("dotnet test", "just dotnet-test"),
    ("dotnet clean", "just dotnet-clean"),
    ("dotnet publish", "just dotnet-publish"),
    // Rust toolchain
    ("cargo build", "just build"),
    ("cargo test", "just test"),
    ("cargo check", "just check"),
    ("cargo clippy", "just lint"),
    // docker
    ("docker compose", "just docker-*"),
    // listing
    ("ls ", "just ls"),
];

/// Just recipes that produce verbose output (need truncation wrapping)
pub const JUST_VERBOSE: &[&str] = &[
    "build", "test", "install", "lint", "lint-fix", "format", "format-fix",
    "align", "typecheck", "preview", "health", "health-ts", "health-dotnet",
    "circular", "deadcode", "depcheck", "tsc-check", "a11y",
    "dotnet-build", "dotnet-test", "dotnet-publish", "dotnet-clean",
    "dotnet-restore", "dotnet-format",
    "turbo-build", "turbo-lint",
    "content-build", "content-build-all",
];

/// Just recipes with short/formatted output (pass through)
pub const JUST_SHORT: &[&str] = &[
    "status", "branches", "remotes", "last-commit",
    "changed-files", "changed-files-staged",
    "diff-stat", "log-compact", "diff-compact", "show-compact",
    "diff", "diff-staged", "show",
    "difft", "difft-file", "difft-commit", "difft-branch",
    "docker-ps", "docker-ps-compact", "docker-up", "docker-up-build",
    "docker-down", "docker-down-volumes", "docker-rebuild", "docker-fresh",
    "docker-logs", "docker-logs-follow",
    "content-up", "content-up-build", "content-down", "content-fresh",
    "content-ps", "content-logs", "content-logs-api", "content-logs-cms",
    "content-build-compact", "content-settings", "content-page",
    "content-page-id", "content-pages", "content-media", "content-sitemap",
    "content-validate", "content-validate-guids", "content-validate-xrefs",
    "dotnet-build-compact", "dotnet-test-compact", "dotnet-run",
    "tokei", "outline", "rg", "fd", "ls", "tree", "ps",
    "dev", "dev-all", "dev-legacy",
];

/// Commands that already handle their own output (pass through)
pub const COMPACT_TOOLS: &[&str] = &[
    "build-compact", "test-compact", "log-compact", "diff-compact", "outline",
];

/// Verbose raw command patterns (regex) for truncation
pub const VERBOSE_PATTERNS: &[&str] = &[
    r"\b(dotnet\s+(build|test|run|publish|restore|pack|clean))\b",
    r"(?i)\b(npm\s+(install|ci|run|test|audit|outdated|ls))\b",
    r"(?i)\b(pnpm\s+(install|build|test|add|remove))\b",
    r"(?i)\b(yarn\s+(install|add|remove|test|build))\b",
    r"(?i)\b(pip3?\s+install)\b",
    r"\b(git\s+(log|diff|show|blame|shortlog))\b",
    r"(?i)\b(docker\s+(build|logs|ps|images))\b",
    r"(?i)\b(kubectl\s+(get|describe|logs))\b",
    r"(?i)\b(terraform\s+(plan|apply|init))\b",
    // NOTE: cargo intentionally excluded — piping through warden.exe truncate-filter
    // deadlocks on Windows when cargo is building warden itself (file lock on running .exe)
    r"(?i)\b(go\s+(build|test|vet))\b",
    r"(?i)\b(mvn|gradle)\b",
    r"(?i)\b(make|cmake)\b",
    r"(?i)\bfind\s+[/\\]",
    r"\brg\s+",
    r"(?i)\bnpx\s+(knip|madge|tsc|depcheck)\b",
];

/// Short command patterns (regex) — pass through, no truncation
pub const SHORT_COMMANDS: &[&str] = &[
    r"(?i)^\s*(echo|cd|mkdir|rmdir|del|copy|move|set|export|pwd|ls\s|dir\s|type\s|cat\s[^|]+$|head|tail|wc|which|where|hostname|whoami|date|cls|clear)\b",
    r"(?i)^\s*(git\s+(add|checkout|branch|switch|stash|init|clone|remote|fetch|pull|push|reset|revert|cherry-pick|rebase|merge|tag|config))\b",
    r"(?i)^\s*(npm\s+(init|cache|config|set|get|link|unlink|pack|publish|login|logout|whoami|token))\b",
    r"(?i)^\s*(dotnet\s+(new|add|remove|list|sln|nuget|tool|workload))\b",
    r"(?i)^\s*(bat|fd|rg\s.*-[lc]$|sd|tokei|lazygit|glow|jq|yq|xh)\b",
];
