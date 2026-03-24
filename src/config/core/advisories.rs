// ─── core::advisories — non-blocking advisory patterns ───────────────────────

/// Advisories: (regex, message) — command runs, but hint is injected
pub const ADVISORIES: &[(&str, &str)] = &[
    // Tool substitution advisories (softer than deny — for borderline cases)
    (r"^\s*docker\s+(ps|logs|inspect|images|container\s+ls)\b",
     "Advisory: Use docker MCP (list_containers, fetch_container_logs) instead of docker CLI."),
    (r#"\brg\s+["']?(class|interface|struct|enum|function|fn|def|type|trait)\s+\w+"#,
     "Advisory: Use aidex_query for symbol lookups instead of rg."),
    (r#"\brg\s+["']?(import|from|extends|implements|export\s+default)\b"#,
     "Advisory: Use ast-grep (sg) for structural patterns instead of rg."),
    (r"\|\s*(awk|sed|cut)\b",
     "Advisory: Consider jc for structured JSON output instead of awk/sed/cut."),
    (r#"\brg\s+["']?#"#,
     "Advisory: For markdown structure queries, use mdq instead of rg."),
    (r"\bpnpm\s+--filter\b",
     "Advisory: Consider using the project's just recipe which handles workspace filtering."),
    // Build/test advisories
    (r"\bnpm\s+install\b",
     "Advisory: npm install modifies node_modules and lock file. Ensure this is intentional."),
    (r"\bcargo\s+add\b",
     "Advisory: cargo add modifies Cargo.toml. Verify the dependency is correct."),
    (r"\bpip\s+install\b",
     "Advisory: pip install modifies the environment. Consider using a virtual environment."),
    // Large operation advisories
    (r"\bgit\s+clone\b",
     "Advisory: git clone downloads a full repository. This may be slow and use disk space."),
    (r"\bnpm\s+audit\s+fix\b",
     "Advisory: npm audit fix modifies dependencies. Review changes before committing."),
    // Performance advisories
    (r"\bnpx\s",
     "Advisory: npx downloads and runs a package. Ensure you trust the package source."),
    (r"\bcargo\s+install\b",
     "Advisory: cargo install compiles from source. This may take several minutes."),
    // Global install advisories
    (r"\bnpm\s+install\s+-g\b",
     "Advisory: Global npm install modifies system state. Consider local install."),
    (r"\bpip\s+install\s+--user\b",
     "Advisory: pip --user modifies user-level Python packages."),
    // Modification advisories
    (r"\bcargo\s+fmt\b(?!\s+--check)",
     "Advisory: cargo fmt modifies files in-place. Use --check to preview."),
    (r"\bdotnet\s+ef\s+migrations\s+add\b",
     "Advisory: Creates migration files. Verify migration name and model state."),
    (r"\bnpm\s+update\b",
     "Advisory: npm update modifies package-lock.json. Review changes."),
];
