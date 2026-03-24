# Contributing

## Development Setup

```bash
git clone https://github.com/ekud12/warden
cd warden
cargo build --release
cargo test
cargo clippy --release
```

## Project Structure

```
src/
├── main.rs              — CLI entry, dispatch, catch_unwind
├── constants.rs         — Product identity (NAME, DIR, etc.)
├── daemon.rs            — Background IPC daemon
├── ipc.rs               — Named pipe client
├── pipeline/            — Composable middleware
├── rules/               — Rule merging (compiled + TOML)
├── assistant/           — Multi-assistant adapter
├── common/              — Shared I/O, session state, output
├── config/core/         — All compiled-in patterns (14 files)
├── handlers/            — Hook implementations (30+ files)
├── analytics/           — Runtime intelligence (10 modules)
└── install/             — Setup wizard
```

## Adding a New Rule

1. Choose the category file in `src/config/core/` (safety, hallucination, substitution, etc.)
2. Add a `(regex, message)` tuple to the appropriate constant
3. Run `cargo test` to verify no regex compilation errors
4. Add a test case in `tests/integration.rs` if the rule is safety-critical

## Adding a New Handler

1. Create `src/handlers/your_handler.rs`
2. Add `pub mod your_handler;` to `src/handlers/mod.rs`
3. Add the subcommand to `HOOK_SUBCMDS` and `dispatch_hook()` in `main.rs`
4. Add the hook event to the assistant config in `claude_code.rs` / `gemini_cli.rs`

## Adding a New Assistant

1. Create `src/assistant/your_assistant.rs` implementing the `Assistant` trait
2. Add detection logic to `detect_assistant()` in `src/assistant/mod.rs`
3. Add `install` subcommand handling in `main.rs`

## Code Style

- Edition 2024, Rust stable
- Zero clippy warnings (`cargo clippy --release`)
- `catch_unwind` on all handler dispatch (fail open, never block AI)
- `#![allow(dead_code)]` at crate level (application binary, not library)
- Prefer `let-else` over `.unwrap()` in production paths
