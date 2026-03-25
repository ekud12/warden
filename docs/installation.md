# Installation

## Requirements

- **Rust toolchain** — Rust 1.85+ (edition 2024). Install via [rustup](https://rustup.rs/).
- **Platform** — Windows 10+, macOS 12+, or Linux (x86_64). ARM64 supported but untested.
- **AI Assistant** — Claude Code or Gemini CLI installed and configured.

## Install from Source

```bash
git clone https://github.com/ekud12/warden
cd warden
cargo install --path .
```

This builds an optimized release binary (~3MB) and places it in `~/.cargo/bin/`.

## Initialize

```bash
warden init
```

This runs the setup wizard:

1. **Detects your AI assistant** (Claude Code or Gemini CLI)
2. **Installs hook configuration** in the assistant's settings
3. **Creates default rules** at `~/.warden/rules.toml`
4. **Optionally starts the daemon** for sub-millisecond hook latency

## Verify Installation

```bash
warden describe
```

This prints your Warden configuration: installed rules, active hooks, daemon status, and project detection.

```bash
warden debug-stats
```

Shows accumulated session statistics across all projects.

## Daemon

Warden includes an optional background daemon that keeps compiled rules in memory and responds to hook requests via IPC. This reduces per-hook latency from ~12ms to ~2ms.

The daemon:

- Starts automatically on first session (if configured)
- Persists across sessions (like a language server)
- Auto-restarts on binary rebuild
- Auto-stops after 1 hour idle
- Falls back to CLI mode if unavailable

**Windows:** Named pipe IPC with owner-only DACL security.
**macOS/Linux:** Unix domain socket IPC with 0600 permissions.

### Manual daemon control

```bash
warden daemon          # Start daemon in foreground
warden debug-daemon-stop     # Stop running daemon
warden debug-daemon-status   # Check daemon status
```

## Upgrade

```bash
cd warden
git pull
cargo install --path .
```

The daemon detects binary changes via mtime and auto-restarts.

## Uninstall

```bash
cargo uninstall warden
rm -rf ~/.warden
```

Then remove the hook configuration from your AI assistant's settings. For Claude Code, remove the Warden entries from `~/.claude/settings.json` under the `hooks` key.

## Troubleshooting

### "command not found: warden"

Ensure `~/.cargo/bin` is in your `PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### "Hook not firing"

1. Check `warden describe` — verify hooks are registered
2. Check your AI assistant's settings file for the hook configuration
3. Ensure the warden binary is accessible from the assistant's environment

### "Daemon won't start"

1. Check if another instance is running: `warden debug-daemon-status`
2. Check logs: `~/.warden/projects/*/logs/daemon.log`
3. On Unix, check socket permissions: `ls -la /tmp/warden-*.sock`

### Build errors

Ensure Rust 1.85+ is installed:

```bash
rustup update stable
rustc --version
```

## Next Steps

- [Quick Start](examples/quick-start.md) — your first session with Warden
- [Configuration](configuration.md) — customize rules and thresholds
