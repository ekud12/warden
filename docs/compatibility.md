# Warden Compatibility Policy

This document defines the stability guarantees and deprecation process for Warden's public interfaces.

## Stable Interfaces

The following are considered stable. Breaking changes require a major version bump and at least one release window with deprecation warnings.

### Hook Subcommands

These subcommand names are stable and will not be renamed:

| Subcommand | Event |
|-----------|-------|
| `pretool-bash` | Before shell command execution |
| `pretool-read` | Before file read |
| `pretool-write` | Before file write/edit |
| `pretool-redirect` | Before tool redirect evaluation |
| `permission-approve` | Permission request evaluation |
| `posttool-session` | After tool execution (session tracking) |
| `posttool-mcp` | After MCP tool execution |
| `session-start` | Session initialization |
| `session-end` | Session finalization |
| `precompact-memory` | Before context compaction |
| `postcompact` | After context compaction |
| `stop-check` | Stop condition evaluation |
| `userprompt-context` | User prompt context injection |
| `subagent-context` | Subagent context injection |
| `subagent-stop` | Subagent stop evaluation |
| `postfailure-guide` | After tool failure guidance |
| `task-completed` | Task completion event |
| `truncate-filter` | Output truncation/filtering |

### CLI Commands

| Command | Stability |
|---------|-----------|
| `init` | Stable |
| `install <agent>` | Stable |
| `uninstall` | Stable |
| `update` | Stable |
| `config` | Stable |
| `describe` | Stable |
| `version` | Stable |
| `mcp` | Stable |
| `debug-*` | Unstable — may be renamed, removed, or promoted |

### MCP Tool Names

| Tool | Stability |
|------|-----------|
| `session_status` | Stable |
| `explain_denial` | Stable |
| `suggest_action` | Stable |
| `check_file` | Stable |
| `session_history` | Stable |
| `reset_context` | Stable |

### Config Keys

The following `config.toml` keys are stable:

- `assistant.type` — `"auto"`, `"claude-code"`, `"gemini-cli"`
- `telemetry.*` — boolean flags for analytics modules
- `restrictions.disabled` — list of rule IDs to disable

### Rules TOML

The following `rules.toml` sections are stable:

- `safety`, `destructive`, `substitutions`, `advisories`, `hallucination`, `hallucination_advisory`
- `sensitive_paths_deny`, `sensitive_paths_warn`
- `auto_allow`, `zero_trace`, `just`, `thresholds`, `restrictions`
- `command_filters`

The `replace = true` option within pattern sections is stable.

## Session State Schema

SessionState is serialized with `#[serde(default)]` on all fields. This means:

- **Adding fields** is always safe — old state deserializes with defaults.
- **Removing fields** is safe — unknown fields are silently ignored by serde.
- **Renaming fields** is a breaking change unless the old name is preserved as an alias.
- **Changing field types** is a breaking change.

## Redb Storage

Table names are stable:

- `session_state`, `events`, `stats`, `effectiveness`, `filters`, `dream`, `resume_packets`

Key formats within tables may evolve. Warden must handle missing or unparseable values gracefully (fail-open).

## Deprecation Process

1. **Announce** — Add deprecation warning to command output and changelog.
2. **Alias** — Old name continues to work, maps to new name internally.
3. **Warn** — Old name emits a deprecation warning on each use.
4. **Remove** — After at least one minor version with warnings, the old name may be removed in the next minor or major release.

For hook subcommands: deprecation requires TWO release windows because assistants may have the old names in their settings files.

## Migration Requirements

When `warden install` or `warden update` modifies assistant settings:

1. Read existing settings file.
2. Identify Warden-owned hook entries (by command path or marker).
3. Update only Warden-owned entries.
4. Preserve all non-Warden hooks.
5. Back up the original file before writing.
6. Verify the written file is valid JSON.

## Version Consistency

The following must agree on the current version:

- `Cargo.toml` `[package].version`
- `CHANGELOG.md` latest section header
- `warden version` output
- GitHub Release tag (when published)

A CI test validates this consistency.
