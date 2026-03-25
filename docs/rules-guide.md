# Rules Guide

## How Rules Work

Warden matches regex patterns against Bash commands, file paths, and tool outputs. Each pattern has a message returned as either a deny reason (blocks the action) or an advisory (warning injected as context).

Rules merge from multiple tiers. Each tier can append to or fully replace the tier above it.

## Rule Tiers

### 1. Compiled defaults (always active)

Baked into the binary at `src/config/core/`. Emergency fallback -- even if all TOML files are missing or corrupt, these rules still apply.

### 2. Global rules (~/.warden/rules.toml)

User-level overrides that apply to every project. Created by `warden init` or manually.

### 3. Project rules (.warden/rules.toml)

Per-project overrides in your project root. Highest priority -- project rules win over global rules.

**Merge behavior:** Each section appends patterns by default. Set `replace = true` to discard all patterns from prior tiers for that section.

## Rule Categories

All compiled rules live in `src/config/core/`:

| Category | File | Severity | Description |
|----------|------|----------|-------------|
| Safety | `safety.rs` | Hard deny | rm -rf, sudo, git mutations, disk format, kill -9, registry edit, firewall |
| Destructive | `safety.rs` | Hard deny | knip --fix, sg rewrite, madge --image |
| Hallucination (deny) | `hallucination.rs` | Hard deny | Reverse shells, credential piping, SSH key writes, env exfil, base64 to network, npm publish, pip install from URL, SUID, crontab, /etc/hosts |
| Hallucination (advisory) | `hallucination.rs` | Advisory | Deep traversal, shell config writes, global installs, docker prune, service restart |
| Substitutions | `substitutions.rs` | Hard deny | grep->rg, find->fd, curl->xh, ts-node->tsx, du->dust, sort\|uniq->huniq, sd blocked on Windows |
| Advisories | `advisories.rs` | Advisory | docker CLI->MCP, symbol rg->aidex |
| Zero-trace | `zero_trace.rs` | Hard deny | AI attribution in comments, git messages, content, file paths |
| Sensitive paths (deny) | `sensitive_paths.rs` | Hard deny | .ssh, .gnupg, .env, credentials |
| Sensitive paths (warn) | `sensitive_paths.rs` | Advisory | System directories |
| Injection | `injection.rs` | Detection | Prompt injection patterns in tool output |
| Error hints | `error_hints.rs` | Recovery | CLI error -> fix suggestion (PostToolUseFailure) |
| Auto-allow | `auto_allow.rs` | Auto-approve | Safe read-only commands that skip permission prompt |
| Thresholds | `thresholds.rs` | Limits | MAX_READ_SIZE (50KB), MAX_MCP_OUTPUT (15KB), etc. |

## Pattern Format

Patterns use Rust regex syntax:

```toml
{ match = 'REGEX_PATTERN', msg = "Message shown to the AI assistant" }
```

- `match` -- Rust regex (case-sensitive by default; use `(?i)` for case-insensitive)
- `msg` -- deny reason or advisory text

Examples:

```toml
# Block a dangerous command
{ match = '^\s*poweroff\b', msg = "BLOCKED: poweroff is not allowed" }

# Advisory (non-blocking)
{ match = '^\s*npm run\b', msg = "Consider using a just recipe instead" }

# Case-insensitive match
{ match = '(?i)password\s*=', msg = "BLOCKED: Hardcoded password detected" }
```

## TOML Schema Reference

Full schema in `src/rules/schema.rs`. All sections are optional.

```toml
# --- Pattern sections (all follow same format) ---
# Available: [safety], [destructive], [substitutions], [advisories],
#            [hallucination], [hallucination_advisory],
#            [sensitive_paths_deny], [sensitive_paths_warn]

[safety]
replace = false  # true = discard compiled defaults; false = append
patterns = [
    { match = '\bpoweroff\b', msg = "BLOCKED: poweroff" },
]

# --- Auto-allow (regex list, no messages) ---
[auto_allow]
replace = false
patterns = ["^my-safe-tool "]

# --- Zero-trace (single regex overrides) ---
[zero_trace]
content_pattern = '(?i)generated\s+by\s+claude'
cmd_pattern = '(?i)commit.*-m.*ai|claude|copilot'
write_pattern = '(?i)generated|auto-generated|copilot'
path_exclude = '\.md$|CHANGELOG'

# --- Just-first configuration ---
[just]
replace_map = false
map = [
    { prefix = "make build", recipe = "just build" },
]
replace_verbose = false
verbose = ["my-verbose-recipe"]
replace_short = false
short = ["my-short-recipe"]

# --- Threshold overrides ---
[thresholds]
max_read_size_kb = 50
max_mcp_output_kb = 15
max_string_len = 2000
max_array_len = 30
doom_loop_threshold = 3
offload_threshold_kb = 8
token_budget_advisory_k = 700
progressive_read_deny_turn = 80
progressive_read_advisory_turn = 50
rules_reinject_interval = 30
drift_threshold = 3
error_slope_threshold = 0.5
stale_milestone_turns = 10
token_burn_threshold_k = 15
stagnation_turns = 3

# --- Restriction toggling ---
[restrictions]
disable = ["substitution.cat", "read.post-edit"]
```

## Adding Custom Rules

### Personal rules (all your projects)

Edit `~/.warden/rules.toml`:

```toml
[substitutions]
patterns = [
    { match = '\bwget\b', msg = "Use xh instead of wget" },
]

[advisories]
patterns = [
    { match = '\bnpm run\b', msg = "Consider using a just recipe" },
]
```

### Project rules (one project only)

Create `.warden/rules.toml` in your project root:

```toml
[thresholds]
max_read_size_kb = 100  # Allow larger files in this project

[auto_allow]
patterns = ["^dotnet ", "^cargo clippy"]

[safety]
patterns = [
    { match = '\bdeploy\s+prod\b', msg = "BLOCKED: Production deploy. Use CI/CD." },
]
```

### Replacing defaults entirely

Set `replace = true` to discard ALL compiled defaults for a section:

```toml
[substitutions]
replace = true
patterns = [
    # Only these patterns apply -- all compiled substitutions removed
    { match = '\bgrep\s', msg = "Use rg" },
]
```

## Restriction Registry

48 restrictions across 8 categories. Each has a unique ID, severity, and disable flag.

```bash
warden debug-restrictions list                          # Show all restrictions
warden debug-restrictions list --category safety        # Filter by category
warden debug-restrictions list --category substitution
warden debug-restrictions list --disabled               # Show only disabled

warden debug-restrictions disable substitution.grep     # Disable at runtime
warden debug-restrictions enable substitution.grep      # Re-enable
```

**Disable in config** (persists across sessions):

In `config.toml`:
```toml
[restrictions]
disabled = ["substitution.cat", "read.post-edit"]
```

In `rules.toml`:
```toml
[restrictions]
disable = ["substitution.cat", "read.post-edit"]
```

Both locations are merged. Safety-critical restrictions (`can_disable: false`) cannot be disabled.

## Hot Reload

Rules files are checked for changes via mtime. When the daemon detects a rules.toml modification, it restarts automatically to pick up new patterns. No manual restart needed.
