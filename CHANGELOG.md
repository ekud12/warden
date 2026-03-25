# Changelog

## [1.2.0] - 2026-03-25

### Added — Roadmap V1 (5 tracks)

**Track 1: Upgrade Safety**
- **Compatibility policy** — `docs/compatibility.md` defining stable interfaces, deprecation process
- **Version consistency tests** — CI validates Cargo.toml, CHANGELOG.md, and binary agree
- **Hook install safety** — merge-not-replace: only updates Warden-owned hooks, preserves non-Warden hooks
- **Upgrade compatibility tests** — old config, old state, unknown fields, corrupt JSON all handled gracefully

**Track 2: Real Self-Update**
- **`warden update --check`** — check for newer versions with install method detection (cargo/standalone/npm)
- **`warden update --apply`** — perform actual upgrade with platform-specific binary swap + rollback
- **`warden doctor`** — verify installation health (binary, daemon, config, hooks, install method)

**Track 3: Dream System V2**
- **Typed dream artifacts** — DreamPlaybook, RepairPattern, ProjectConvention, SuccessfulSequence
- **Sequence mining** — 3-gram action sequences correlated with milestones
- **Repair pattern learning** — error signature → successful fix mapping
- **Convention learning** — build preferences, common edit sets, verification frequency
- **Artifact scoring + pruning** — confidence decay, minimum thresholds, stale convention removal
- **Resume Packet V2** — top playbook, convention hints, verification debt

**Track 4: Product Surface Cleanup**
- **Clean command aliases** — `explain`, `stats`, `scorecard`, `replay`, `tui`, `export`, `restrictions`, `daemon-status`, `daemon-stop`
- **`debug-*` backward compat** — old prefixed names still work
- **Help output** — COMMANDS + DIAGNOSTICS sections

**Track 5: Config Intellisense & Schema**
- **JSON Schema** — `schemas/config.schema.json` + `schemas/rules.schema.json`
- **`warden config schema`** / **`warden rules schema`** CLI commands
- **Schema drift tests** — 6 tests ensuring schema covers all known config keys and categories

### Also Added
- **`warden describe`** — shows only active user overrides (--all for full JSON dump)

## [1.1.1] - 2026-03-25

### Added
- **Interactive TUI wizard** (`warden init`) — arrow-key navigation, multi-select, spinners, styled prompts (crossterm raw mode)
- **"Did you mean?"** command suggestions — Levenshtein edit distance for unknown commands (≤3 distance)
- **`warden update`** — checks GitHub Releases API for newer versions, shows comparison + upgrade instructions
- **Already-installed detection** — `warden init` detects existing hooks, offers update instead of fresh install
- **5-platform release pipeline** — GitHub Actions builds x86_64-linux, aarch64-linux, x86_64-macos, aarch64-macos, x86_64-windows
- **npm distribution** — `npx @bitmilldev/warden init` with postinstall binary download + cargo fallback
- **`--version` / `-v` aliases** for version command

### Changed
- Rule count: 298 → **+300 patterns** across 9 categories
- W ASCII banner: left-aligned, clean shape in brand red
- Wizard shows only missing tools + Skip option (installed tools as checkmarks above)
- Esc from any wizard prompt aborts immediately

### Fixed
- **Windows KeyEventKind** — filter for Press only (crossterm sends Press+Release per keystroke)
- **Single stderr handle** — fixed mixed eprint!/write! buffering race
- **Relative cursor movement** — fixed absolute positioning failure in some terminals
- **npm postinstall EFTYPE** — fixed empty binary from private repo 404, added file size validation
- **read_to_string deadlock** — changed to read() with 1MB buffer for relay compatibility

## [1.0.0] - 2026-03-24

### Added
- Initial release of Warden — runtime control layer for AI coding agents
- **Multi-assistant support**: Claude Code + Gemini CLI adapters
- **MCP server mode** (`warden mcp`) — bidirectional harness, 6 tools exposed
- **298 compiled rules** across 9 categories
- **29 runtime analytics features** (session phases, trust scoring, injection budget)
- **Dream state** — background learning with effectiveness scoring, resume packets, error clustering
- **Scorecard** — session quality measurement (safety, efficiency, focus, UX)
- **Tiered rules** with 4-level TOML merge (compiled → core → personal → project)
- **IPC daemon** with named pipes (Windows) + Unix sockets (~2ms latency)
- **redb storage** — ACID embedded database for session state, events, dream artifacts
- **Relay binary** — windowless hook shim for Windows (no CMD flash)
- **RegexSet matching** — single DFA pass for all patterns
- **Cross-platform**: Windows, macOS, Linux (x64 + ARM64)
- **182 tests**, zero clippy warnings, 3.7MB binary
