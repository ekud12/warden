// ─── core::safety — destructive system operation patterns (DENY) ─────────────

/// Safety: (regex, deny_message) — universally dangerous, always ON
pub const SAFETY: &[(&str, &str)] = &[
    // Filesystem destruction
    (r"\brm\s+-rf?\s+[~*/.]", "BLOCKED: rm -rf on broad paths. Remove specific files by name."),
    (r"\brm\s+-rf?\s+/\b", "BLOCKED: rm -rf on root path."),
    (r"\brm\s+-rf?\s+\$HOME\b", "BLOCKED: rm -rf on home directory."),
    (r"\brmdir\s+/s\b", "BLOCKED: rmdir /s is destructive. Remove specific directories."),
    // Privilege escalation
    (r"\bsudo\s", "BLOCKED: sudo is not allowed. Run without sudo."),
    (r"\bsu\s+-\b", "BLOCKED: su - switches user. Not allowed."),
    (r"\bdoas\s", "BLOCKED: doas is not allowed. Run without privilege escalation."),
    (r"\brunas\s", "BLOCKED: runas is not allowed on Windows."),
    // Permissions
    (r"chmod\s+777", "BLOCKED: chmod 777. Use more restrictive permissions (755 or 644)."),
    (r"chmod\s+-R\s+777", "BLOCKED: Recursive chmod 777 is extremely dangerous."),
    (r"chmod\s+a\+w", "BLOCKED: chmod a+w makes files world-writable."),
    // System damage
    (r"\bmkfs\b", "BLOCKED: mkfs formats filesystems. Extremely dangerous."),
    (r"\bdd\s+if=", "BLOCKED: dd can overwrite disks. Too dangerous for AI."),
    (r"\bformat\s+[A-Z]:", "BLOCKED: format on Windows drive. Extremely dangerous."),
    (r"\bdiskpart\b", "BLOCKED: diskpart can destroy partitions."),
    (r"\bshutdown\b", "BLOCKED: System shutdown is not allowed."),
    (r"\breboot\b", "BLOCKED: System reboot is not allowed."),
    (r"\bpoweroff\b", "BLOCKED: System poweroff is not allowed."),
    (r"\bhalt\b", "BLOCKED: System halt is not allowed."),
    // Process manipulation
    (r"\bkill\s+-9\s+1\b", "BLOCKED: Killing PID 1 (init) crashes the system."),
    (r"\bkillall\s", "BLOCKED: killall kills all matching processes. Too broad."),
    (r"\bpkill\s+-9\b", "BLOCKED: pkill -9 force-kills processes. Too aggressive."),
    // Environment pollution
    (r"\bexport\s+PATH=\s*$", "BLOCKED: Clearing PATH breaks the shell."),
    (r"\bunset\s+PATH\b", "BLOCKED: Unsetting PATH breaks the shell."),
    (r"\bexport\s+LD_PRELOAD=", "BLOCKED: LD_PRELOAD injection."),
    (r"\bexport\s+LD_LIBRARY_PATH=", "BLOCKED: LD_LIBRARY_PATH hijack."),
    // Permissions escalation
    (r"\bchmod\s+[ug]\+s\b", "BLOCKED: Setting SUID/SGID bit."),
    (r"\bchown\s+-R\s+.*\s+/\b", "BLOCKED: Recursive chown on root path."),
    // Pipe to interpreter
    (r"\|\s*python[23]?\s+-c\b", "BLOCKED: Piping to python -c eval."),
    (r"\|\s*node\s+-e\b", "BLOCKED: Piping to node -e eval."),
    (r"\|\s*ruby\s+-e\b", "BLOCKED: Piping to ruby -e eval."),
    // PowerShell encoded commands
    (r"(?i)powershell\s+-[eE]nc", "BLOCKED: PowerShell encoded command — potential obfuscation."),
];

/// Git mutation rules — OFF by default, opt-in via personal.toml or config
/// Most users want Claude to commit/push. Power users enable this.
pub const GIT_SAFETY: &[(&str, &str)] = &[
    (r"\bgit\s+(push|pull|merge|rebase|cherry-pick|stash|reset|clean|checkout|restore|revert|bisect|am|apply)\b",
     "BLOCKED: Mutating git operation. Only read-only git commands allowed. Ask the user."),
    (r"\bgit\s+branch\s+-[dD]\b", "BLOCKED: Branch deletion. Ask the user."),
    (r"push\s+--force\b", "BLOCKED: force push. Ask the user."),
    (r"\bgit\s+(add|commit|tag)\b", "BLOCKED: git add/commit/tag modify the repository. Ask the user."),
];

/// Destructive: tools that auto-modify code or state
pub const DESTRUCTIVE: &[(&str, &str)] = &[
    (r"knip --fix", "BLOCKED: knip --fix auto-deletes code. Run without --fix first."),
    (r"\bsg\s.*\s-r\s", "BLOCKED: sg rewrite modifies in-place. Add --dry-run first."),
    (r"madge --image", "BLOCKED: madge --image writes a file. Use --circular instead."),
    (r"\bnpm\s+prune\b", "BLOCKED: npm prune removes packages. Show what would be removed first."),
    (r"\bcargo\s+clean\b", "BLOCKED: cargo clean deletes all build artifacts. Ask user first."),
    (r"\bdocker\s+system\s+prune\b", "BLOCKED: docker system prune removes unused data. Ask user first."),
    (r"\bdocker\s+volume\s+rm\b", "BLOCKED: docker volume rm destroys data. Ask user first."),
    (r"\btruncate\s", "BLOCKED: truncate can destroy file contents. Use specific operations."),
    (r"\bshred\s", "BLOCKED: shred irreversibly destroys files. Ask user first."),
    (r"\bnpm\s+run\s+eject\b", "BLOCKED: eject is irreversible. Ask user first."),
    (r"\byarn\s+eject\b", "BLOCKED: eject is irreversible. Ask user first."),
];
