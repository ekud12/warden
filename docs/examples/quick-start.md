# Warden Quick Start

Runtime intelligence layer for AI coding assistants. Intercepts every tool call to enforce safety, substitutions, and session governance.

## Install

Pick one:

```bash
# From crates.io
cargo install warden

# From source
git clone https://github.com/ekud12/warden
cd warden && cargo install --path .

# Pre-built binary (download from releases, then)
cp warden ~/.warden/bin/
chmod +x ~/.warden/bin/warden
```

## Initialize

```bash
warden init
```

The wizard will:

1. Create `~/.warden/` directory structure (`bin/`, `rules/`, `projects/`)
2. Install the binary to `~/.warden/bin/` and add it to PATH
3. Detect installed CLI tools (rg, fd, bat, etc.) and offer to install missing ones
4. Detect AI assistants (Claude Code, Gemini CLI) and configure hooks
5. Write a default `~/.warden/config.toml`
6. Migrate from `~/.hookctl/` if upgrading from hookctl

## Start Coding

No additional steps needed. Once hooks are configured, warden runs automatically:

```bash
# With Claude Code
claude

# With Gemini CLI
gemini
```

Every Bash command, file read, and file write flows through warden's pipeline. Dangerous operations are blocked, banned tools are redirected, and verbose output is truncated.

## Check Session Stats

```bash
# View current session statistics
warden debug-stats

# List all restrictions and their status
warden debug-restrictions list

# Filter by category
warden debug-restrictions list --category Safety

# See merged rule counts
warden rules

# Export session data
warden debug-export --format json
```

## Common Customizations

### Disable a restriction

```bash
# Interactive
warden debug-restrictions disable substitution.cat

# Or edit ~/.warden/config.toml directly
# [restrictions]
# disabled = ["substitution.cat", "read.post-edit"]
```

### Add project-specific rules

Create `.warden/rules.toml` in your project root:

```toml
[auto_allow]
patterns = ["^dotnet ", "^npm run "]

[thresholds]
max_read_size_kb = 100
```

### Add personal rules globally

Edit `~/.warden/rules/personal.toml`:

```toml
[substitutions]
patterns = [
    { match = '\bwget\b', msg = "Use xh instead of wget" },
]
```

### Change a config value

```bash
warden config set assistant.type claude-code
warden config set telemetry.anomaly_detection false
warden config get tools.justfile
```

### Daemon management

Warden runs a background daemon for sub-millisecond hook response:

```bash
warden debug-daemon-status   # Check if running
warden debug-daemon-stop     # Stop the daemon (auto-restarts on next hook call)
```

## Directory Layout

```
~/.warden/
  bin/warden[.exe]          Main binary
  config.toml               User configuration
  rules/
    personal.toml           Your personal rule overrides
  projects/
    {hash}/                 Per-project state
      session-state.json    Current session tracking
      session-notes.jsonl   Session event log
      stats.json            Aggregated statistics

<project>/.warden/
  rules.toml                Project-specific rule overrides
```

## Further Reading

- [Architecture](../architecture.md) -- Pipeline, adapters, analytics internals
- [Configuration](../configuration.md) -- All config options
- [Rules Guide](../rules-guide.md) -- Rule categories and custom rules
- [Pipeline Stages](../pipeline-stages.md) -- Pretool-bash pipeline reference
