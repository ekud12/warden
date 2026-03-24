# Contributing to Warden

## Development Setup

```bash
git clone https://github.com/ekud12/warden.git
cd warden
cargo build
cargo test
```

### Prerequisites

- Rust 1.85+ (edition 2024)
- Windows, macOS, or Linux

### Running Tests

```bash
cargo test                  # All 107 tests
cargo test --bin warden     # Unit tests only
cargo test --test integration # Integration tests only
cargo clippy --release      # Lint (must be zero warnings)
```

### Building Release

```bash
cargo build --release       # Optimized binary (~2.1MB)
```

## Architecture

```
src/
  constants.rs          # Product name, paths (single source of truth)
  main.rs               # Subcommand dispatch
  pipeline/             # Middleware trait + executor (composable, panic-isolated)
  assistant/            # Multi-assistant adapter (Claude Code, Gemini CLI)
  handlers/             # Hook handlers (one per subcommand)
    pretool_bash/       # Safety, substitution, hallucination pipeline
    posttool_session/   # Session tracking, syntax validation, co-change
    mcp_server.rs       # MCP server mode (bidirectional harness)
    git_guardian.rs      # Branch awareness, uncommitted tracking
    auto_changelog.rs   # Session-end narrative generation
    adaptation.rs       # Phase state machine (5 phases, hysteresis)
    userprompt_context.rs # Per-turn analytics hub
  common/               # Shared types, I/O, session state
  config/core/          # Compiled rule defaults (221 patterns)
  rules/                # TOML rule merge engine (4-tier)
  analytics/            # Anomaly, forecast, DNA, quality, cost, recovery
  install/              # First-run wizard, PATH registration, CLI detection
  ipc.rs                # Named pipe IPC client
  daemon.rs             # Background pipe server
```

## Adding a New Middleware Stage

1. Create `src/handlers/my_handler.rs`
2. Implement handler logic (read stdin JSON, emit deny/allow/context)
3. Register in `src/handlers/mod.rs`
4. Add to `HOOK_SUBCMDS` and `dispatch_hook` in `src/main.rs`
5. Add integration tests in `tests/integration.rs`
6. Run `cargo test && cargo clippy --release`

## Adding a New Assistant Adapter

1. Create `src/assistant/my_assistant.rs`
2. Implement the `Assistant` trait (`parse_input`, `format_deny`, `format_allow`, etc.)
3. Add detection logic to `detect_assistant()` in `src/assistant/mod.rs`
4. Add install command in `src/main.rs`

## Adding New Rules

Rules live in `src/config/core/`. Each file is a category:

| File | Category | Type |
|------|----------|------|
| `safety.rs` | Dangerous operations | Hard deny |
| `hallucination.rs` | AI-fabricated attacks | Hard deny |
| `substitutions.rs` | CLI tool redirects | Conditional deny |
| `advisories.rs` | Non-blocking hints | Advisory |
| `auto_allow.rs` | Safe command allowlist | Auto-approve |
| `sensitive_paths.rs` | File write protection | Deny + Advisory |
| `injection.rs` | Prompt injection detection | Alert |

To add a rule, append a `(regex, message)` tuple to the appropriate constant. Rules are compiled once via `LazyLock` at startup.

## Code Style

- Zero clippy warnings (enforced in CI)
- No `dead_code` warnings except the top-level allow (for forward-declared APIs)
- Handlers exit 0 on error — never block the AI assistant
- Performance: <5ms for any single handler, <0.5ms per pipeline stage
- Every handler is panic-isolated in daemon mode

## Pull Requests

- Branch from `main`
- Run `cargo test && cargo clippy --release` before submitting
- Include tests for new features
- Keep PRs focused — one feature or fix per PR
