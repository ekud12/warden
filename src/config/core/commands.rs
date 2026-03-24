// ─── core::commands — command classification for milestone/error tracking ────

/// Build commands (milestone detection)
pub const BUILD_CMDS: &[&str] = &[
    "build", "just build", "just dotnet-build", "dotnet build", "pnpm build", "npm run build",
];

/// Test commands
pub const TEST_CMDS: &[&str] = &[
    "test", "just test", "just dotnet-test", "dotnet test", "pnpm test", "npm run test",
];

/// Lint commands
pub const LINT_CMDS: &[&str] = &[
    "lint", "just lint", "eslint", "pnpm lint", "npm run lint",
];

/// TSC commands
pub const TSC_CMDS: &[&str] = &[
    "tsc --noEmit", "just tsc-check", "just typecheck",
];

/// Knip commands (dead code detection)
pub const KNIP_CMDS: &[&str] = &[
    "knip", "just deadcode",
];

/// Circular dependency commands
pub const CIRCULAR_CMDS: &[&str] = &[
    "madge --circular", "just circular",
];

/// Health check commands
pub const HEALTH_CMDS: &[&str] = &[
    "just health", "just health-ts",
];

/// Deploy commands
pub const DEPLOY_CMDS: &[&str] = &[
    "deploy", "publish", "release", "just dotnet-publish",
];
