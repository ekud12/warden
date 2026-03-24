# Configuration Reference

## Config File

Location: `~/.warden/config.toml`

Generated on first run via `warden init`. View with `warden config list`.

## Sections

### [assistant]

```toml
[assistant]
type = "auto"  # "claude-code" | "gemini-cli" | "auto"
```

| Key | Default | Description |
|-----|---------|-------------|
| `type` | `"auto"` | Which assistant adapter to use. `auto` detects from environment variables (`CLAUDE_SESSION_ID` / `GEMINI_SESSION_ID`). |

### [tools]

```toml
[tools]
justfile = true
rg = true
fd = true
bat = true
eza = true
dust = true
xh = true
```

| Key | Default | Description |
|-----|---------|-------------|
| `justfile` | `true` | Enable just-first transforms (requires `just` installed) |
| `rg` | `true` | Enable grep -> rg substitution |
| `fd` | `true` | Enable find -> fd substitution |
| `bat` | `true` | Enable cat -> bat substitution |
| `eza` | `true` | Enable ls -> eza substitution |
| `dust` | `true` | Enable du -> dust substitution |
| `xh` | `true` | Enable curl -> xh substitution |

Tools are auto-detected on `warden init`. If a tool is not installed, its substitution rule is automatically disabled with no errors. Set any key to `false` to disable its substitution even if the tool is installed.

### [restrictions]

```toml
[restrictions]
disabled = ["substitution.cat", "read.post-edit"]
```

| Key | Default | Description |
|-----|---------|-------------|
| `disabled` | `[]` | List of restriction IDs to disable. See `warden restrictions list` for all IDs. |

Each restriction has a unique dotted ID (e.g., `safety.rm-rf`, `substitution.grep`, `read.dedup`). Safety restrictions (`can_disable: false`) cannot be disabled.

**Runtime toggle:**

```bash
warden restrictions disable substitution.grep   # Add to disabled list
warden restrictions enable substitution.grep    # Remove from disabled list
warden restrictions list                        # Show all restrictions
warden restrictions list --disabled             # Show only disabled
warden restrictions list --category safety      # Filter by category
```

**Categories:** Safety, Destructive, Substitution, Governance, Hallucination, ZeroTrace, Redirect, Permission.

**Severities:** HardDeny (blocked), SoftDeny (blocked but disableable), Advisory (warning only).

### [telemetry]

```toml
[telemetry]
anomaly_detection = true
quality_predictor = true
cost_tracking = true
error_prevention = true
token_forecast = true
smart_truncation = true
project_dna = true
rule_effectiveness = true
drift_velocity = true
compaction_optimizer = true
command_recovery = true
```

| Key | Default | Description |
|-----|---------|-------------|
| `anomaly_detection` | `true` | Z-score flagging vs project baselines (Welford's algorithm) |
| `quality_predictor` | `true` | Session quality prediction at turn 10, then every 5 turns |
| `cost_tracking` | `true` | Token cost categorization (explore/implement/waste/saved) |
| `error_prevention` | `true` | Error pattern prediction |
| `token_forecast` | `true` | Compaction ETA forecasting via linear regression |
| `smart_truncation` | `true` | Keyword-relevance truncation for large outputs |
| `project_dna` | `true` | Per-project statistical fingerprinting |
| `rule_effectiveness` | `true` | Per-rule fire count + quality delta scoring |
| `drift_velocity` | `true` | Cross-session rule learning curves |
| `compaction_optimizer` | `true` | Ranked file list for precompact context |
| `command_recovery` | `true` | CLI flag fix / install suggestions on failure |

All features are on by default. Set any to `false` to opt out.

### [appearance]

```toml
[appearance]
tui_refresh_ms = 2000
```

| Key | Default | Description |
|-----|---------|-------------|
| `tui_refresh_ms` | `2000` | TUI dashboard refresh interval in milliseconds |

## Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `WARDEN_HOME` | Override home directory | `~/.warden/` |
| `WARDEN_TEST` | Test mode -- disables session note writes and side effects | unset |
| `WARDEN_NO_DAEMON` | Skip daemon IPC, always run hooks via direct execution | unset |

## CLI Commands

```bash
# Config management
warden config list                          # Print full config.toml
warden config get <section.key>             # Get a value (e.g., tools.rg)
warden config set <section.key> <value>     # Set a value (e.g., tools.justfile false)
warden config path                          # Print config file path

# Setup
warden init                                 # First-run setup wizard
warden install claude-code                  # Generate hooks config for Claude Code
warden install gemini-cli                   # Generate hooks config for Gemini CLI
warden version                              # Print version

# Inspection
warden rules                                # Show merged rule counts per category
warden restrictions list                    # Show all restrictions with metadata
warden stats                                # Show learning/effectiveness stats
warden describe                             # Describe warden capabilities
warden project-dir                          # Print per-project state directory

# Daemon
warden daemon-status                        # Check if daemon is running (JSON: pid, mtime)
warden daemon-stop                          # Stop the background daemon

# Session data
warden export-sessions [--json]             # Export session history
warden replay <session-id>                  # Replay a session
warden diff <session-a> <session-b>         # Compare two sessions
```

## Threshold Overrides via rules.toml

Thresholds can also be overridden in `~/.warden/rules.toml` or `.warden/rules.toml`:

```toml
[thresholds]
max_read_size_kb = 100               # Max file size for Read (default: 50KB)
max_mcp_output_kb = 15               # Max MCP output size (default: 15KB)
max_string_len = 2000                # Max string before trimming (default: 2000)
max_array_len = 30                   # Max array before trimming (default: 30)
doom_loop_threshold = 3              # Identical tool calls before warning (default: 3)
offload_threshold_kb = 8             # Write to scratch if output exceeds (default: 8KB)
token_budget_advisory_k = 700        # Token budget warning in K tokens (default: 700K)
progressive_read_deny_turn = 80      # Turn after which reads tighten (default: 80)
progressive_read_advisory_turn = 50  # Turn for read advisory (default: 50)
rules_reinject_interval = 30         # Re-inject rules every N turns (default: 30)
drift_threshold = 3                  # Denials in 10-turn window for drift (default: 3)
error_slope_threshold = 0.5          # Error slope for advisory (default: 0.5)
stale_milestone_turns = 10           # Turns without milestone warning (default: 10)
token_burn_threshold_k = 15          # Token burn rate K/turn warning (default: 15K)
stagnation_turns = 3                 # Stagnation snapshots before advisory (default: 3)
```
