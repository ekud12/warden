# Why Warden

## The Problem

AI coding agents are powerful. They write code, run tests, navigate complex codebases, and solve real problems. But they degrade over long sessions.

The longer a session runs, the more likely an agent will:

- **Drift from the goal** — exploring tangential code paths instead of staying focused
- **Retry without learning** — repeating the same failing approach instead of changing strategy
- **Waste context** — reading too many files or running verbose commands that fill the context window with noise
- **Attempt dangerous operations** — running destructive commands, overwriting files, or modifying sensitive paths
- **Lose session awareness** — forgetting what was already tried, re-reading files, or losing track of progress

These failures are not dramatic. They are gradual. The agent looks productive while making no real progress.

## Why Prompt Rules Are Not Enough

Most teams try to solve this with prompt-based guidance:

| Approach | Limitation |
|----------|-----------|
| CLAUDE.md / system prompts | Can be ignored as context fills. Not enforceable. |
| Skill files | Stateless. No memory between invocations. |
| Bash wrappers | Fragile. No session awareness. Single assistant. |
| Manual supervision | Does not scale. Defeats the purpose of automation. |

These are **soft controls**. They rely on the agent choosing to follow instructions. Under context pressure, soft controls degrade first.

## Runtime Control

Warden takes a different approach: **runtime control**.

Instead of asking the agent to follow rules, Warden intercepts every tool call at the hook layer and applies policy before the call reaches the environment. When Warden returns `"deny"`, the operation is blocked deterministically — regardless of what the agent intended.

This is the difference between guidance and governance.

> Prompts can be ignored. Runtime policy cannot.

## What Warden Does

Warden operates across three capabilities:

### Enforce

Runtime policy over tool use. Compiled rules block dangerous commands, catch hallucinated flags and paths, gate destructive operations, and enforce tool substitutions. Every bash command, file write, and file read flows through the policy engine.

### Focus

Detect drift and steer sessions back on-task. Verification debt tracking, focus scoring, loop detection, negative memory, and checkpoint enforcement keep agents productive across long sessions. When an agent starts wandering, Warden surfaces targeted advisories.

### Compress

Trim noisy output before it pollutes context. Data-driven filters compress `cargo test`, `git diff`, `npm install`, and other verbose command outputs by 60-95%. Token waste becomes token savings — keeping the context window pointed at the task, not the noise.

## Who Uses Warden

**Individual developers** using AI coding agents in real repositories. You want fewer wasted turns, fewer dumb loops, lower token costs, and safer automation.

**Engineering teams** standardizing AI-assisted development workflows. You want enforceable policy, consistency across projects, and guardrails that work without babysitting.

**Platform engineers** responsible for developer tooling and security. You want runtime control, structured behavior, and measurable effectiveness.

## When to Use Warden

Use Warden when:

- You run long coding sessions (10+ turns) with AI agents
- You work in repositories where destructive operations matter
- You want consistent behavior enforcement across multiple projects
- You use Claude Code, Gemini CLI, or both
- You want to reduce token waste from verbose command output

You might not need Warden for:

- Single-shot questions or simple lookups
- Projects where you manually review every agent action
- Environments where AI agents have no tool access

## How It Works

```
AI Agent → hook event (JSON) → Warden Pipeline → decision (allow/deny/advisory)
                                    ↑
                                10 stages, each:
                                - independent
                                - panic-isolated
                                - <0.5ms
                                - configurable via TOML
```

Every tool call flows through Warden's middleware pipeline. Each stage runs independently, cannot crash the others, and completes in under half a millisecond. The pipeline decides: allow, deny with explanation, or allow with advisory context.

## Key Metrics

| Metric | Value |
|--------|-------|
| Compiled rules | 298 patterns across 9 categories |
| Hook latency | ~2ms (daemon), ~12ms (cold start) |
| Binary size | 3.7MB |
| Assistants | Claude Code, Gemini CLI |
| Analytics | 29 runtime features, 7 consolidated signals |

## Next Steps

- [Installation](installation.md) — install and configure Warden
- [Quick Start](examples/quick-start.md) — your first session with Warden
- [Runtime Control](runtime-control.md) — understand what Warden controls
