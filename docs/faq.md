# FAQ

## General

### What is Warden?

Warden is the runtime control layer for AI coding agents. It sits between the agent and the codebase, enforcing policy, tracking session state, compressing noisy output, and steering long sessions toward productive outcomes.

### How is Warden different from CLAUDE.md rules?

CLAUDE.md rules are prompts — the agent can choose to ignore them, and they degrade as context fills up. Warden hooks into the tool-use layer. When Warden returns `"deny"`, the tool call is blocked deterministically. The agent cannot override it.

### Does Warden work with Gemini CLI?

Yes. Warden supports both Claude Code and Gemini CLI through assistant adapters. The same rules and pipeline apply to both. `warden init` detects your installed assistant and configures the correct hooks.

### Does Warden slow down my agent?

No. With the daemon running, hook latency is ~2ms. Without the daemon, cold start is ~12ms. Both are imperceptible during normal AI agent operation.

### Does Warden require an internet connection?

No. Warden is a local binary. All rules, analytics, and compression run entirely on your machine.

## Rules and Policy

### Why was my command blocked?

Run `warden debug-explain <rule-id>` to see the rule's pattern, message, and how to disable it. The denial message shown to the agent includes the rule category.

To see all rules: `warden debug-restrictions list`.

### How do I disable a specific rule?

```bash
warden debug-restrictions disable <rule-id>
```

Or in `rules.toml`:

```toml
[restrictions]
disable = ["substitution.grep_to_rg"]
```

### Can I add custom rules?

Yes. Add patterns to `~/.warden/rules.toml` (global) or `.warden/rules.toml` (per-project):

```toml
[safety]
patterns = [
    { match = "\\bmy-dangerous-cmd\\b", msg = "BLOCKED: not allowed." }
]
```

### Can a malicious repo disable safety rules?

No. Compiled safety patterns form an immutable floor. A project's `rules.toml` can add rules or replace user-added rules, but it can never disable the compiled safety patterns. Even with `replace = true`, the built-in protections remain active.

### What is shadow mode?

Rules tagged `shadow = true` log what they would deny without actually blocking. This lets you test new rules in production before enforcing them.

```toml
[safety]
patterns = [
    { match = "\\bmy-new-rule\\b", msg = "Would block this.", shadow = true }
]
```

## Session Intelligence

### What is the focus score?

A composite 0-100 score measuring how focused the current session is. It penalizes directory spread, subsystem switches without milestones, and excessive exploration without edits. When it drops below 40, Warden advises the agent to narrow scope.

### What is verification debt?

The number of edits since the last successful build or test. When this exceeds 4, Warden advises the agent to run verification before making more changes.

### How does loop detection work?

Warden tracks the sequence of agent actions (read, edit, bash_ok, bash_fail) and detects repeating patterns: 2-step ping-pong (A→B→A→B), 3-step cycles (A→B→C→A→B→C), and read spirals (5+ consecutive reads without edit).

### Can I disable analytics?

Yes. In `~/.warden/config.toml`:

```toml
[telemetry]
anomaly_detection = false
quality_predictor = false
focus_tracking = false
```

All analytics default to enabled.

## Daemon

### How do I start the daemon?

The daemon starts automatically during `warden init` or on the first session. To start manually:

```bash
warden daemon
```

### How do I stop the daemon?

```bash
warden debug-daemon-stop
```

Or it auto-stops after 1 hour of inactivity.

### What happens if the daemon crashes?

Warden falls back to CLI mode automatically. Each hook invocation tries the daemon first; if it fails, the handler runs directly. No session data is lost — the daemon cache is flushed to disk after each request.

### Does the daemon persist across sessions?

Yes. The daemon persists like a language server. It only restarts when:

- The binary is rebuilt (detected via mtime)
- The rules file changes
- It idles for 1 hour

## Output Compression

### How much output does Warden compress?

Depends on the command. `cargo test` with 200+ passing tests: 90-99% compression. `npm install`: 80-95%. `git diff` on large changes: 40-70%. Only noise is removed — errors, failures, and summaries are always preserved.

### Can I add custom filters?

Yes. In `rules.toml`:

```toml
[[command_filters]]
match = "my-command"
strategy = "keep_matching"
keep_patterns = ["^ERROR", "^PASS", "^FAIL"]
max_lines = 40
```

### Does compression affect error visibility?

No. All compression strategies prioritize keeping error lines, failure details, and summary information. Compression targets noise (progress bars, passing tests, download logs), not signal.

## Troubleshooting

### "Hook not firing"

1. Run `warden describe` — check that hooks are registered
2. Verify your assistant's settings file includes the Warden hook entries
3. Check that the `warden` binary is on your `PATH`

### "Permission denied" on daemon

- **Windows:** Named pipe uses owner-only DACL. Ensure you're running as the same user.
- **Unix:** Socket uses 0600 permissions. Check with `ls -la /tmp/warden-*.sock`.

### "Rules not loading from project"

Check `.warden/rules.toml` exists in your project root (the directory where you run the AI agent). Run `warden describe` to see which rules files are detected.

### High memory usage

The daemon holds compiled regex patterns and session state in memory. Typical usage is 5-15MB. If higher, check for very large custom rule sets in TOML.

## Next Steps

- [Troubleshooting](troubleshooting.md) — detailed error resolution guide
- [Commands Reference](commands.md) — all Warden commands
- [Configuration](configuration.md) — full configuration reference
