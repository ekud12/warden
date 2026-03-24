// ─── core::hallucination — agent hallucination patterns (DENY + ADVISORY) ────

/// Hard deny: unambiguously dangerous hallucinated commands
pub const HALLUCINATION: &[(&str, &str)] = &[
    // Path traversal / encoding attacks
    (r"%2e%2e(%2f|/)|%252e", "BLOCKED: URL-encoded path traversal."),
    (r"\\x00|%00", "BLOCKED: Null byte in command."),
    (r"\$\(.*base64\s+-d", "BLOCKED: Base64-decoded command execution."),
    // Reverse shells / network exfiltration
    (r"/dev/tcp/", "BLOCKED: Reverse shell pattern."),
    (r"\bsocat\b.*EXEC:", "BLOCKED: Socat exec — potential reverse shell."),
    (r"\btelnet\b.*\|\s*bash", "BLOCKED: Telnet piped to bash."),
    (r"\bncat\b.*-e\b", "BLOCKED: Ncat with execute flag."),
    // Credential theft
    (r"(>|tee\s+\S*)\s*~?/?\.ssh/", "BLOCKED: Writing to .ssh directory."),
    (r"(>|tee\s+\S*)\s*~?/?\.gnupg/", "BLOCKED: Writing to .gnupg directory."),
    (r"(>|tee\s+\S*)\s*~?/?\.(npmrc|git-credentials|netrc)\b", "BLOCKED: Writing to credential files."),
    (r"\.(ssh/id_|npmrc|git-credentials|netrc).*\|\s*(curl|wget|xh|nc|base64)", "BLOCKED: Piping credentials to network tool."),
    (r"cat\s+.*(passwd|shadow|credentials).*\|", "BLOCKED: Piping system credential files."),
    (r"cat\s+.*\.env\b.*\|\s*(curl|wget|xh|nc|base64)", "BLOCKED: Piping .env file to network."),
    // Command hijacking
    (r"\b(alias|function)\s+(sudo|su|ssh|rm|ls|cd)\b", "BLOCKED: Hijacking security-critical command."),
    (r"\bsource\s+/dev/stdin", "BLOCKED: Sourcing from stdin."),
    (r"\beval\s+\$\(curl", "BLOCKED: Eval of remote script."),
    (r"\bcurl\b.*\|\s*bash\b", "BLOCKED: Piping curl output to bash."),
    (r"\bwget\b.*\|\s*bash\b", "BLOCKED: Piping wget output to bash."),
    (r"\bcurl\b.*\|\s*sh\b", "BLOCKED: Piping curl output to sh."),
    (r"\bwget\b.*-O\s*-\s*\|\s*sh\b", "BLOCKED: Piping wget output to sh."),
    // History exfiltration
    (r"\bhistory\b.*\|\s*(curl|wget|xh|nc|base64)", "BLOCKED: Piping shell history to external tool."),
    // Cron/persistence
    (r"\bcrontab\s+-e\b", "BLOCKED: Interactive crontab edit."),
    (r"echo\s+.*>>\s*/etc/crontab", "BLOCKED: Appending to system crontab."),
    // Kernel module manipulation
    (r"\binsmod\b|\bmodprobe\b", "BLOCKED: Kernel module loading."),
    // Firewall manipulation
    (r"\biptables\b|\bnftables\b", "BLOCKED: Firewall rule modification."),
    (r"\bufw\s+(enable|disable|allow|deny)\b", "BLOCKED: UFW firewall modification."),
    // PowerShell obfuscation
    (r"(?i)powershell\s+.*-[Ee]ncodedCommand\b", "BLOCKED: PowerShell encoded command execution."),
    (r"(?i)Invoke-Expression\s+\(.*Download", "BLOCKED: PowerShell download + execute."),
];

/// Advisory: suspicious but could be legitimate
pub const HALLUCINATION_ADVISORY: &[(&str, &str)] = &[
    (r"\.\./\.\.", "Advisory: Deep path traversal (../../). Verify this is intentional."),
    (r"(>|tee\s+\S*)\s*~?/?\.(bashrc|zshrc|profile|bash_profile)\b", "Advisory: Writing to shell config file."),
    (r"(>|tee\s+\S*)\s*~?/?\.git/hooks/", "Advisory: Writing to git hooks directory."),
    (r"\.env.*\|\s*(curl|wget|xh|nc|base64)", "Advisory: Piping .env file to network tool."),
    (r"\bmkfifo\s+/tmp/", "Advisory: Named pipe creation in /tmp."),
    // Moved from hard deny — can appear in legitimate code/tutorials
    (r"\bnc\b.*\s-e\s", "Advisory: Netcat with execute flag. Verify this is not a reverse shell."),
    (r"bash\s+-i\s+>&", "Advisory: Interactive bash redirect. Verify this is intentional."),
    (r"\bpython[23]?\s+-c\s+.*socket", "Advisory: Python socket one-liner. Verify intent."),
    (r"\bperl\s+-e\s+.*socket", "Advisory: Perl socket one-liner. Verify intent."),
    (r"\bruby\s+-e\s+.*TCPSocket", "Advisory: Ruby TCPSocket one-liner. Verify intent."),
    (r"\bchmod\s+[0-7]*[67][0-7]{2}\b", "Advisory: Permissive chmod. Verify permissions are minimal."),
    (r"\bnohup\s+.*&$", "Advisory: Background process with nohup will persist after session."),
    (r"\bscreen\s+-dmS\b", "Advisory: Detached screen session will persist."),
    (r"\btmux\s+new-session\s+-d\b", "Advisory: Detached tmux session will persist."),
    (r"\bwget\s+-q\b.*-O\b", "Advisory: Quiet wget download. Verify the source URL."),
    (r">\s*/dev/null\s+2>&1", "Advisory: Silencing all output. Errors may be hidden."),
    (r"\bxargs\s+-I\s*{}\s+(rm|mv|cp)\b", "Advisory: xargs with destructive command. Verify the input."),
    (r"\bssh\s+-o\s+StrictHostKeyChecking=no\b", "Advisory: Disabling SSH host key checking."),
    (r"\bopenssl\s+(genrsa|genpkey|req)\b", "Advisory: OpenSSL key/cert generation. Verify parameters."),
    (r"\bsetfacl\b", "Advisory: ACL modification. Verify target path."),
];
