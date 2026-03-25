# Warden

<p align="center">
  <img src="assets/logo.png" alt="Warden" width="100" />
</p>

<p align="center"><strong>Runtime control for AI coding agents</strong></p>

---

> Warden is the runtime control layer for AI coding agents. It intercepts tool use, enforces policy, reduces drift, compresses noisy output, and keeps long sessions focused on the actual task.

## The Problem

AI coding agents do not usually fail in one dramatic moment. They degrade gradually.

They drift from the goal, reread too much code, run broad or noisy commands, retry without learning, and occasionally attempt unsafe actions. Traditional controls — prompts, markdown files, conventions — are soft. They can be ignored, diluted, or lost as context fills up.

**Warden is a hard control layer.** It works at runtime. Hook returns `"deny"` — deterministic, cannot be ignored.

## Enforce / Focus / Compress

**Enforce** — Runtime policy over tool use. 298 compiled rules block dangerous commands, catch hallucinations, and gate destructive operations.

> Prompts can be ignored. Runtime policy cannot.

**Focus** — Detect drift and steer sessions back on-task. Verification debt tracking, focus scoring, loop detection, negative memory, and checkpoint enforcement keep agents productive across long sessions.

> Hooks react to events. Warden builds session state across the whole session.

**Compress** — Trim noisy output before it pollutes context. Data-driven filters compress cargo test, git diff, npm install, and 8+ other command outputs by 60-95%.

> Compression alone reduces noise. Warden also enforces policy and steers behavior.

## Quick Example

```
AI:  rm -rf /tmp/*
     BLOCKED. rm -rf on broad paths. Remove specific files by name.

AI:  grep -r "TODO" src/
     BLOCKED. Use rg (ripgrep) — 10x faster, respects .gitignore.

AI:  cargo test    (262 passing, 0 failing)
     → Smart filter: "cargo test (262 passed, 0 failed)"
     → 99% compression — only summary reaches AI context
```

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

Every Bash command, file read, and file write flows through Warden's middleware pipeline. Dangerous operations are blocked, tool substitutions are enforced, verbose output is intelligently compressed, and context is injected to guide the agent.

## Key Metrics

| Metric | Value |
|--------|-------|
| Compiled rules | **298** patterns across 9 categories |
| Hook latency | **~2ms** (daemon), ~12ms (cold) |
| Binary size | **3.7MB** |
| Assistants | Claude Code, Gemini CLI |
| Analytics | **29** runtime features |
| Advisory signals | **7** consolidated (trust-gated) |
| Source | **110** files, 17K lines |
| Storage | redb embedded database |

## Get Started

```bash
git clone https://github.com/ekud12/warden
cd warden && cargo install --path .
warden init
```

The [Installation](installation.md) guide covers platform setup, daemon configuration, and troubleshooting. The [Quick Start](examples/quick-start.md) walks through your first session.

## Documentation

| Section | What You'll Find |
|---------|--------------------|
| [Why Warden](why-warden.md) | Problem statement, use cases, competitive positioning |
| [Installation](installation.md) | Platform setup, daemon, troubleshooting |
| [Quick Start](examples/quick-start.md) | Install, configure, start coding |
| [Runtime Control](runtime-control.md) | Policy engine, rule categories, merge model |
| [Session Intelligence](session-intelligence.md) | Drift detection, focus scoring, loop detection |
| [Context Efficiency](context-efficiency.md) | Output compression, token savings, custom filters |
| [Configuration](configuration.md) | TOML schema, thresholds, telemetry |
| [Rules Guide](rules-guide.md) | All rule categories, custom rules, shadow mode |
| [Commands](commands.md) | CLI reference |
| [FAQ](faq.md) | Common questions and answers |
| [Architecture](architecture.md) | Pipeline, adapters, IPC, analytics |

---

<sub>MIT License · Rust 2024 · [GitHub](https://github.com/ekud12/warden)</sub>
