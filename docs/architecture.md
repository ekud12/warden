# Warden Architecture

Single-binary runtime intelligence layer for AI coding assistants. Intercepts hook events (tool calls, session lifecycle, user prompts) and processes them through a composable middleware pipeline.

## Pipeline / Middleware

Every hook invocation flows through a `Pipeline` of `Middleware` stages (`src/pipeline/`).

```
AI Assistant -> hook event (JSON stdin) -> Pipeline [stage1 -> stage2 -> ... -> stageN] -> JSON stdout
```

**Core types:**

- `PipelineContext` -- shared mutable state carrying tool name, tool input, advisories, timing data, and the final decision.
- `StageResult` -- `Continue`, `Deny(msg)`, `Allow(advisory)`, or `Skip`.
- `Decision` -- final outcome: `Deny(String)` or `Allow(Option<String>)`.

**Properties:**

- Stages run in order; `Deny` or `Allow` short-circuits immediately.
- Each stage wrapped in `catch_unwind` -- panics are logged and skipped (fail-open).
- Per-stage timing recorded for profiling.
- Disabled stages (via `enabled()` method) skipped at zero cost.

```rust
pub trait Middleware: Send + Sync {
    fn name(&self) -> &'static str;
    fn enabled(&self, ctx: &PipelineContext) -> bool;
    fn process(&self, ctx: &mut PipelineContext) -> StageResult;
}
```

## Multi-Assistant Adapter

Supports multiple AI assistants through the `Assistant` trait (`src/assistant/mod.rs`). Input is normalized into `HookInput` so all pipeline stages work identically regardless of which assistant is driving.

Detection is automatic via environment variables:

| Adapter | Module | Env Detection | Settings Path |
|---------|--------|---------------|---------------|
| Claude Code | `claude_code.rs` | `CLAUDE_SESSION_ID`, `CLAUDE_CODE_ENTRYPOINT` | `~/.claude/settings.json` |
| Gemini CLI | `gemini_cli.rs` | `GEMINI_SESSION_ID`, `GEMINI_PROJECT_DIR` | `~/.gemini/settings.json` |

**Format differences handled by adapters:**

| Feature | Claude Code | Gemini CLI |
|---------|-------------|------------|
| Deny field | `permissionDecision: "deny"` | `decision: "deny"` |
| Context injection | `additionalContext` | `systemMessage` |
| Hook events | `PreToolUse` / `PostToolUse` | `BeforeTool` / `AfterTool` |
| Tool name field | `tool_name` | `tool.name` |
| Tool input field | `tool_input` | `tool.arguments` |

```rust
pub trait Assistant: Send + Sync {
    fn name(&self) -> &str;
    fn parse_input(&self, raw: &str) -> Option<HookInput>;
    fn format_deny(&self, event: &str, message: &str) -> String;
    fn format_allow(&self, advisory: Option<&str>) -> String;
    fn format_auto_allow(&self) -> String;
    fn format_context(&self, text: &str) -> String;
    fn format_updated_output(&self, output: &Value) -> String;
    fn settings_path(&self) -> PathBuf;
    fn generate_hooks_config(&self, binary_path: &Path) -> String;
}
```

## Tiered Rules

Rules merge from multiple sources. Each tier can append to or replace the tier above it.

```
1. Compiled defaults    (src/config/core/*.rs)     -- always present, baked into binary
2. Global rules.toml    (~/.warden/rules.toml)     -- user-level overrides
3. Project rules.toml   (.warden/rules.toml)       -- per-project overrides
```

Set `replace = true` in any TOML section to discard all previous tiers for that section.

**Compiled rule categories** (`src/config/core/`):

| Category | File | Description |
|----------|------|-------------|
| Safety | `safety.rs` | rm -rf, sudo, git mutations, disk format |
| Destructive | `safety.rs` | knip --fix, sg rewrite, madge --image |
| Substitutions | `substitutions.rs` | grep->rg, find->fd, curl->xh, du->dust |
| Advisories | `advisories.rs` | docker CLI->MCP, symbol rg->aidex |
| Hallucination | `hallucination.rs` | reverse shells, credential piping, env exfil |
| Zero-trace | `zero_trace.rs` | AI attribution blocking in comments/git |
| Sensitive paths | `sensitive_paths.rs` | .ssh, .gnupg, .env, system dirs |
| Injection | `injection.rs` | prompt injection detection |
| Error hints | `error_hints.rs` | PostToolUseFailure recovery suggestions |
| Auto-allow | `auto_allow.rs` | safe read-only commands (auto-approve) |
| Thresholds | `thresholds.rs` | MAX_READ_SIZE, MAX_MCP_OUTPUT, etc. |

Rules are loaded once via `LazyLock` (`src/rules/mod.rs`). The daemon restarts automatically when rules files change (mtime detection).

## Session Telemetry

Session state persisted per-project in `session-state.json` (`src/common/session.rs`). Collections are bounded (max 50 files_read, 20 commands, 20 snapshots, etc.) and evicted by age.

**TurnSnapshot** -- per-turn telemetry collected every user prompt:

| Field | Type | Description |
|-------|------|-------------|
| `turn` | u32 | Turn number |
| `errors_unresolved` | u32 | Cumulative unresolved errors |
| `explore_count` | u32 | Read/search operations |
| `files_edited_count` | u16 | Files modified this session |
| `files_read_count` | u16 | Files read this session |
| `tokens_in_delta` | u64 | Input tokens since last snapshot |
| `tokens_out_delta` | u64 | Output tokens since last snapshot |
| `milestones_hit` | bool | Whether a milestone occurred this turn |
| `edits_this_turn` | bool | Whether edits happened this turn |
| `denials_this_turn` | u8 | Denials in this turn |

**Phase detection** (`src/handlers/adaptation.rs`) classifies the session into phases based on TurnSnapshot patterns:

| Phase | Trigger | Behavior |
|-------|---------|----------|
| Warmup | First few turns | Default parameters |
| Productive | Active editing, milestones | Relaxed limits, wider dedup window |
| Exploring | High read/search, low edits | Higher explore budget |
| Struggling | Rising errors, no milestones | Tighter guardrails, more advisories |
| Late | Approaching context budget | Aggressive truncation (one-way, never exits) |

Each phase adapts ~8 runtime parameters: advisory cooldown, truncation limits, MCP output limit, explore budget, context size, rules reinject interval, read dedup window, drift threshold. Hysteresis of 2 turns prevents flapping.

## Analytics Engine

All analytics run automatically during sessions (`src/analytics/`):

| Module | Algorithm | Purpose |
|--------|-----------|---------|
| `anomaly.rs` | Welford's online mean/variance | Z-score flagging (values >2 sigma from mean) |
| `forecast.rs` | Linear regression on (turn, tokens) | Compaction ETA prediction |
| `dna.rs` | Per-project statistical fingerprint | Baselines for anomaly detection and quality |
| `effectiveness.rs` | Per-rule quality delta | Track which rules help or hurt session quality |
| `quality.rs` | Weighted heuristic ensemble | Session quality score (0-100) |
| `recovery.rs` | Pattern-matching knowledge base | CLI flag fixes and install suggestions |

**Anomaly metrics tracked** (per-project, Welford accumulators in `stats.json`): tokens/turn, errors/session, edit velocity, explore ratio, denial rate, session length, quality score.

**Quality scoring** (predicted at turn 10, then every 5 turns):
- Edit velocity (30%) -- fraction of turns with edits
- Error trajectory (30%) -- inverse of error slope
- Token efficiency (20%) -- tokens saved / total ratio
- Milestone rate (20%) -- milestones per turn

## IPC Daemon Architecture

The daemon compiles regexes once, caches session state in memory, and responds via named pipe.

```
AI Assistant -> hook event -> warden CLI -> named pipe -> daemon -> handler dispatch -> response
                                  |                                       |
                                  |                                  catch_unwind
                                  +-- fallback: direct execution if daemon unavailable
```

**Named pipe:** `\\.\pipe\warden-{username}`

**Protocol:** Length-prefixed JSON. 4-byte little-endian length prefix, then JSON payload. One request/response per connection.

**Request** (`DaemonRequest`): `subcmd`, `payload`, `binary_mtime`, `cwd`, `rules_mtime`

**Response** (`DaemonResponse`): `stdout`, `exit_code`

**Lifecycle:**
- Auto-started on `session-start` if pipe not connectable
- Binary copied to `warden-daemon.exe` so source binary is never locked
- Persists across sessions (like a background service)
- Auto-stops after 1 hour idle (watchdog thread)
- Auto-restarts on binary rebuild (client mtime != daemon mtime)
- Auto-restarts on rules.toml change (rules_mtime mismatch)
- Falls back to direct CLI execution if daemon is unavailable

Exit code `-2` (`EXIT_RESTART`) signals the client that the daemon detected a rebuild; the client retries via direct execution.

## Directory Layout

```
~/.warden/
  bin/
    warden[.exe]             -- main binary (on PATH)
    warden-daemon[.exe]      -- daemon copy (auto-managed)
    warden-daemon.pid        -- daemon PID + exe path
  config.toml                -- user configuration
  rules.toml                 -- global rules (user overrides)
  rules/                     -- rules directory
  projects/
    {hash8}/                 -- per-project state (8-char hash of CWD)
      session-state.json     -- current session state (TurnSnapshots, files, commands)
      session-notes.jsonl    -- session event log (milestones, errors, transitions)
      stats.json             -- project statistics (Welford accumulators)

.warden/                     -- per-project directory (in project root)
  rules.toml                 -- project-specific rule overrides
```

## Performance Targets

| Metric | Target | Mechanism |
|--------|--------|-----------|
| Hook latency (daemon) | <3ms | In-memory state, compiled regexes, named pipe IPC |
| Hook latency (cold) | <15ms | Direct execution, lazy regex compilation |
| Pipeline (10 stages) | <5ms | Sequential with short-circuit |
| Analytics (per turn) | <0.5ms | O(1) Welford updates, O(n) regression on max 20 points |
| Daemon startup | <50ms | Copy binary + spawn detached process |
| Memory (daemon) | <10MB | Bounded collections, minimal allocations |
| Binary size | <3MB | `opt-level = "z"`, LTO, strip, `codegen-units = 1` |
| Panic tolerance | 100% | `catch_unwind` per stage, fail-open on all errors |
