# Commands Reference

## Management

| Command | Description |
|---------|-------------|
| `warden init` | Interactive setup wizard — creates dirs, installs binary, configures hooks |
| `warden install <assistant>` | Configure hooks for `claude-code` or `gemini-cli` |
| `warden version` | Print version |

## Inspection

| Command | Description |
|---------|-------------|
| `warden rules` | Show merged rule counts (compiled + TOML) |
| `warden describe` | Machine-readable capabilities JSON |

## Configuration

| Command | Description |
|---------|-------------|
| `warden config list` | Show current config.toml |
| `warden config get <key>` | Get a config value (e.g. `tools.justfile`) |
| `warden config set <key> <val>` | Set a config value |

## Server

| Command | Description |
|---------|-------------|
| `warden mcp` | Run as MCP server (stdio, JSON-RPC 2.0) |
| `warden daemon` | Start background daemon (auto-managed) |

## Debug Commands

These are for human debugging only. Not part of the agent workflow — all functions they expose run automatically.

| Command | Description |
|---------|-------------|
| `warden debug-explain <rule-id>` | Show what a rule does, pattern, category, how to disable |
| `warden debug-explain-session` | Timeline of every Warden intervention this session |
| `warden debug-restrictions list` | View all 298 rules with metadata |
| `warden debug-restrictions list --category Safety` | Filter rules by category |
| `warden debug-restrictions disable <id>` | Disable a specific rule |
| `warden debug-restrictions enable <id>` | Re-enable a disabled rule |
| `warden debug-stats` | Cross-project learning statistics |
| `warden debug-replay` | Session timeline narrative |
| `warden debug-scorecard` | Session quality scorecard |
| `warden debug-export` | Export session data (JSON/CSV) |
| `warden debug-tui` | Live session dashboard (ratatui terminal UI) |
| `warden debug-daemon-status` | Check if daemon is running |
| `warden debug-daemon-stop` | Stop the background daemon |

## Hook Subcommands

These are called automatically by AI assistants — you don't run them manually.

| Subcommand | Hook Event | Purpose |
|------------|-----------|---------|
| `pretool-bash` | PreToolUse:Bash | Safety, substitution, advisory pipeline |
| `pretool-read` | PreToolUse:Read | Read governance (dedup, large files) |
| `pretool-write` | PreToolUse:Write | Sensitive path + zero-trace enforcement |
| `pretool-redirect` | PreToolUse:Grep/Glob | Tool redirect (Grep→rg, Glob→fd) |
| `permission-approve` | PermissionRequest | Auto-approve safe commands |
| `posttool-session` | PostToolUse | Analytics, error tracking, milestones |
| `posttool-mcp` | PostToolUse:MCP | MCP output trimming |
| `session-start` | SessionStart | Rules-only injection, silent redb storage |
| `session-end` | SessionEnd | Summary, DNA update, cleanup |
| `userprompt-context` | UserPromptSubmit | Per-turn telemetry + adaptation |
| `precompact-memory` | PreCompact | Rules re-injection for compaction |
| `stop-check` | Stop | Session health check |
| `truncate-filter` | (pipe) | Smart output compression |

## Examples

```bash
# See why grep was blocked
warden debug-explain substitution.grep

# Disable cat→bat substitution
warden debug-restrictions disable substitution.cat

# Check what Warden did this session
warden debug-explain-session

# View session quality scorecard
warden debug-scorecard
```
