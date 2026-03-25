# Runtime Control

Warden's Policy Engine intercepts every tool call before it reaches the environment. This page explains what Warden controls, how decisions are made, and how rules are organized.

## Decision Types

Every tool call receives one of three decisions:

| Decision | Effect | Example |
|----------|--------|---------|
| **Deny** | Tool call is blocked. Agent sees the denial reason. | `rm -rf /` → "BLOCKED: rm -rf on broad paths." |
| **Allow with Advisory** | Tool call proceeds. Agent receives guidance in context. | Large file read → "Consider using outline for structure first." |
| **Allow** | Tool call proceeds silently. | `cargo build` → passes through unchanged. |

Denials are deterministic. When a rule matches, the deny is unconditional — the agent cannot override it.

## Rule Categories

Warden ships with 298 compiled rules across 9 categories:

### Safety (Hard Deny)

Blocks universally dangerous operations that should never execute in an AI agent session.

- Filesystem destruction (`rm -rf`, `mkfs`, `dd`)
- Privilege escalation (`sudo`, `doas`, `runas`)
- Dangerous permissions (`chmod 777`, `chmod -R a+w`)
- System control (`shutdown`, `reboot`, `halt`)
- Process killing (`killall`, `kill -9 1`)

### Destructive (Hard Deny)

Blocks operations that destroy data or are difficult to reverse.

- Git force operations (`push --force`, `reset --hard`)
- Database destruction (`DROP DATABASE`, `DROP TABLE`)
- Package removal (`pip uninstall -y`, `npm uninstall -g`)
- Container cleanup (`docker system prune`)

### Hallucination Detection (Hard Deny)

Catches commands with nonexistent flags, fabricated tool names, or invalid paths that indicate the agent is hallucinating.

- Nonexistent cargo/npm/git subcommands
- Fabricated CLI flags
- Made-up executable names
- Invalid configuration file references

### Substitutions (Redirect)

Redirects the agent to better tools when alternatives are available.

- `grep` → `rg` (ripgrep — faster, respects .gitignore)
- `find` → `fd` (simpler syntax, faster)
- `curl` → `xh` (better HTTP client)
- `cat` for viewing → `bat` (syntax highlighting)

Substitutions only fire when the target tool is installed (auto-detected).

### Advisories (Soft Guidance)

Non-blocking guidance injected into the agent's context.

- Suggesting better approaches
- Warning about potential issues
- Recommending verification steps

### Sensitive Path Protection

Warns or blocks operations on sensitive file paths:

- Configuration files (`.env`, `credentials.json`)
- System paths (`/etc/`, `C:\Windows\`)
- Build artifacts that should not be modified directly
- CI/CD configuration files

### Auto-Allow

Patterns that skip all checks for known-safe operations. Reduces latency for common commands like `ls`, `echo`, `pwd`.

## Rule Precedence

Rules are evaluated in order:

1. **Auto-allow** — if matched, skip all checks (fast path)
2. **Safety** — highest priority deny
3. **Expansion risk** — variable/subshell bypass detection
4. **Hallucination** — fabricated commands/flags
5. **Destructive** — data-destroying operations
6. **Substitutions** — tool redirects
7. **Advisories** — non-blocking guidance

The first match wins. A safety deny takes priority over a substitution.

## Three-Tier Rule Merge

Rules come from three sources, merged at startup:

| Tier | Source | Purpose |
|------|--------|---------|
| **Compiled** | Built into the binary | Safety floor. Cannot be disabled. |
| **Global** | `~/.warden/rules.toml` | Personal preferences across all projects. |
| **Project** | `.warden/rules.toml` | Project-specific rules. |

Compiled safety patterns form an **immutable floor**. Even if a project rules file sets `replace = true`, the compiled safety patterns remain active. This prevents a malicious repository from disabling critical protections.

User-added patterns from global and project TOML files extend the compiled defaults. Project rules can replace global user additions but never compiled defaults.

## Shadow Mode

Rules can be tagged `shadow = true` in TOML. Shadow rules log what they would deny without actually blocking. This enables safe rule rollout — test a new rule in shadow mode, verify it matches correctly, then promote to enforced.

## Disabling Rules

Individual rules can be disabled via the restrictions system:

```bash
warden debug-restrictions disable substitution.grep_to_rg
warden debug-restrictions enable substitution.grep_to_rg
warden debug-restrictions list
```

Or in `rules.toml`:

```toml
[restrictions]
disable = ["substitution.grep_to_rg", "advisory.large_read"]
```

## Custom Rules

Add rules in `~/.warden/rules.toml` or `.warden/rules.toml`:

```toml
[safety]
patterns = [
    { match = "\\bmy-dangerous-cmd\\b", msg = "BLOCKED: my-dangerous-cmd is not allowed." }
]

[advisories]
patterns = [
    { match = "\\bnpm run build\\b", msg = "Consider using `just build` instead." }
]
```

See the [Rules Guide](rules-guide.md) for the full TOML schema and all pattern options.

## Variable Expansion Detection

Warden detects attempts to bypass pattern matching through shell expansion:

- `$VAR -rf /` — variable expansion in dangerous position
- `` `cmd` -rf / `` — backtick expansion
- `$(cmd) -rf /` — subshell expansion
- `eval $DANGEROUS` — eval with variables
- `xargs rm` — xargs piping to dangerous commands

These are checked before pattern matching, so bypasses are caught regardless of the variable's value.

## Next Steps

- [Session Intelligence](session-intelligence.md) — how Warden tracks session health
- [Configuration](configuration.md) — customize rules and thresholds
- [Rules Guide](rules-guide.md) — full rule reference
