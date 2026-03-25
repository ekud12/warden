# Troubleshooting

## Common Issues

### Hook Not Firing

**Symptoms:** Warden rules are not being enforced. Commands that should be blocked pass through.

**Resolution:**

1. Verify hooks are registered:
   ```bash
   warden describe
   ```
   Look for "Hooks: registered" in the output.

2. Check your assistant's settings file:
   - **Claude Code:** `~/.claude/settings.json` should contain Warden hook entries
   - **Gemini CLI:** Check the Gemini hook configuration directory

3. Verify the binary is accessible:
   ```bash
   which warden
   warden --version
   ```

4. If using the daemon, check it's running:
   ```bash
   warden debug-daemon-status
   ```

### Command Incorrectly Blocked

**Symptoms:** A legitimate command is denied by Warden.

**Resolution:**

1. Check which rule fired:
   ```bash
   warden debug-explain <rule-id>
   ```
   The denial message includes the rule category.

2. Disable the specific rule:
   ```bash
   warden debug-restrictions disable <rule-id>
   ```

3. If it's a false positive in a compiled rule, add an exception in `rules.toml`:
   ```toml
   [auto_allow]
   patterns = ["your-specific-command-pattern"]
   ```

### Daemon Won't Start

**Symptoms:** `warden daemon` fails or the daemon doesn't respond to requests.

**Resolution:**

1. Check for stale PID file:
   ```bash
   warden debug-daemon-status
   ```

2. Check if the port/socket is in use:
   - **Windows:** Check for stale named pipe
   - **Unix:** `ls -la /tmp/warden-*.sock` — delete stale socket files

3. Check logs:
   ```
   ~/.warden/projects/*/logs/daemon.log
   ```

4. Try stopping and restarting:
   ```bash
   warden debug-daemon-stop
   warden daemon
   ```

### Rules Not Loading from Project

**Symptoms:** Project-specific rules in `.warden/rules.toml` are not being applied.

**Resolution:**

1. Verify the file location: `.warden/rules.toml` must be in the project root directory (the CWD when the AI agent runs).

2. Check for TOML syntax errors:
   ```bash
   warden describe
   ```
   Parse errors are logged.

3. Verify the rule format:
   ```toml
   [safety]
   patterns = [
       { match = "\\bpattern\\b", msg = "Message" }
   ]
   ```
   Note: `match` is the field name (not `regex`), and backslashes must be doubled in TOML.

### High Token Usage Despite Compression

**Symptoms:** Session token counts are higher than expected.

**Resolution:**

1. Check which commands are being compressed:
   ```bash
   warden debug-stats
   ```
   Look at the savings breakdown.

2. Verify compression is active. The `truncate-filter` stage must be in the hook pipeline.

3. For commands without built-in filters, add custom filters:
   ```toml
   [[command_filters]]
   match = "your-verbose-command"
   strategy = "keep_matching"
   keep_patterns = ["^ERROR", "^Summary"]
   max_lines = 30
   ```

### Session State Corruption

**Symptoms:** Warden behaves unexpectedly — wrong turn count, stale advisories, or missing analytics.

**Resolution:**

1. State resets on each new session automatically. If mid-session state is corrupted, restart the AI assistant session.

2. For persistent issues, clear the project's state:
   ```bash
   rm ~/.warden/projects/*/warden.db
   ```
   State will rebuild from defaults on next session.

3. If using the daemon, restart it after clearing state:
   ```bash
   warden debug-daemon-stop
   ```

## Error Messages

| Error | Meaning | Fix |
|-------|---------|-----|
| "BLOCKED: rm -rf on broad paths" | Safety rule prevented destructive deletion | Remove specific files by name instead |
| "BLOCKED: Use rg instead of grep" | Substitution rule redirecting to better tool | Use the suggested alternative |
| "Advisory: N edits since last build/test" | Verification debt accumulating | Run `cargo test` or equivalent |
| "Focus score N/100" | Session focus is degrading | Narrow scope to fewer directories |
| "Repeating pattern detected" | Behavioral loop detected | Try a fundamentally different approach |

## Diagnostic Commands

| Command | Purpose |
|---------|---------|
| `warden describe` | Show configuration, hooks, detected tools |
| `warden debug-stats` | Show accumulated session statistics |
| `warden debug-restrictions list` | Show all rules and their status |
| `warden debug-explain <id>` | Explain a specific rule |
| `warden replay` | Replay last session timeline |
| `warden debug-daemon-status` | Check daemon health |

## Getting Help

- **GitHub Issues:** [github.com/ekud12/warden/issues](https://github.com/ekud12/warden/issues)
- **Documentation:** [github.com/ekud12/warden/tree/main/docs](https://github.com/ekud12/warden/tree/main/docs)
