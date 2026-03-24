# Warden

**Harness engineering for AI coding agents.**

Warden is a runtime intelligence layer that sits between AI coding assistants (Claude Code, Gemini CLI) and your codebase. It enforces rules deterministically, adapts to session context, learns across sessions, and provides the AI with real-time guidance.

## What Warden Does

- **Blocks dangerous commands** — rm -rf, sudo, force push, credential piping
- **Catches hallucinations** — reverse shells, eval of remote scripts, prompt injection
- **Redirects to better tools** — grep→rg, find→fd, curl→xh (only when installed)
- **Adapts to your session** — 5 session phases, 8 tunable parameters
- **Learns across sessions** — per-project quality fingerprints (Project DNA)
- **Provides bidirectional guidance** — MCP server lets the AI query Warden

## Key Numbers

| Metric | Value |
|--------|-------|
| Rules | 298 patterns across 9 categories |
| Hook latency | ~2ms (daemon), ~12ms (cold) |
| Binary size | ~2.7MB |
| Tests | 107 |
| Assistants | Claude Code, Gemini CLI |

## Getting Started

See the [Quick Start](examples/quick-start.md) guide to install and configure Warden in under 5 minutes.

## Architecture

Warden is a single Rust binary with a composable middleware pipeline, multi-assistant adapter, tiered rule system, and background daemon for sub-millisecond response times. See [Architecture](architecture.md) for details.
