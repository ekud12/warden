# Changelog

## [2.10.0] - 2026-03-28

### Critical — IPC Security
- **Fixed Windows named pipe DACL** — null DACL granted world-open access (CRITICAL). Removed DACL manipulation entirely; uses process token default (current user + SYSTEM + Admins only)
- **Fixed server-status naming mismatch** — CLI sent "server-status" but daemon handled "daemon-status". Normalized to "server-status" everywhere

### High — Supply Chain & Safety
- **SHA-256 checksum verification** — update flow and npm postinstall now verify downloaded binaries against published checksums-sha256.txt. Fail closed on mismatch
- **Safety-critical handlers fail closed** — pretool-bash, pretool-write, pretool-read, pretool-redirect, permission-approve now exit 1 on panic (was exit 0). Advisory handlers remain fail-open
- **Added sha2 crate** for cryptographic hash verification

### Medium — Operational Integrity
- **Fixed package version/license drift** — all manifests (npm, brew, scoop, winget) updated to v2.9.0 + AGPL-3.0-only (were MIT at various stale versions)
- **Narrowed privacy claim** — "100% local during hook execution" replaces misleading "zero network calls"
- **Fixed webhook delivery** — auth header now passed in curl fallback, timeout_ms honored
- **Subprocess stderr captured** — SubprocessResult now includes stderr field (was discarded)
- **Storage performance** — cached DB handle via with_db() closure (no reopen per call), reverse iteration for read_last_events/diagnostics (no full table scan)

### Quick Wins
- **Tool install parsing** — uses platform shell instead of split_whitespace (handles paths with spaces)
- **cargo audit in CI** — automated vulnerability scanning added to CI pipeline

## [2.9.0] - 2026-03-28

### Evidence Consolidation (Audit V3 — Phase 1)
- **Session events for all safety checks** — expansion-risk, hallucination, zero-trace, destructive, and advisory checks now produce persistent session events (was log-only for 5 of 6 checks)
- **Session events for read/write governance** — post-edit, dedup, large-file, progressive-read, sensitive-path, and zero-trace-write all produce session events
- **Enriched advisory_selection events** — now include advisory_id (stable hash), utility score per selected, and session phase
- **Threshold promotion events** — anomaly_promoted, forecast_promoted, goal_anchoring emit structured events with threshold data
- **Fixed learn_effectiveness** — uses structured event types instead of fragile substring matching; recognizes build-ok/test-pass as positive signals
- **Enhanced doctor intelligence** — new Reason column (config-off/budget-gated/no-trigger/active), phase transition history, promoted signal counts, effectiveness arrows

### Intelligence Wiring (Audit V3 — Phase 2)
- **Repair patterns → advisory pipeline** — Dream's learned repair patterns now surface as advisories (utility 0.65) when recent errors match known signatures
- **Project conventions → advisory pipeline** — high-confidence conventions (>0.8) inject as low-priority advisories (utility 0.25)
- **Promoted signal diagnostics** — threshold crossing events enable Dream to learn from promoted signals

### Dream Deepening (Audit V3 — Phase 3)
- **Resume packet auto-injection** — after compaction or 10+ minutes inactivity, Dream's resume packet injects as high-priority advisory (utility 0.85) with current issue, dead ends, and verification debt
- **Sequence-based next-step suggestion** — when last 2 actions match a known successful 3-gram (3+ occurrences), suggests the next action (utility 0.2)
- **Quality trend decline advisory** — warns when quality score declines for 3+ consecutive snapshots (utility 0.3)
- **Intervention effectiveness at session end** — learn_effectiveness now runs at session close, not just during daemon idle time
- **MCP introspection expanded** — session_status now shows verification debt, "why this phase" reason, and dropped advisories

### CLI & Diagnostics
- **Fixed cleanup age calculation** — project dirs without key files no longer show "20540 days old" (UNIX_EPOCH fallback → directory mtime)

## [2.8.0] - 2026-03-28

### Audit V2 Implementation
- **Promoted 3 silent signals to injected** — anomaly (z>2.5→pressure), compaction forecast (<5 turns→pressure), goal anchoring (every 5 turns→focus). 10 injected signal categories now, up from 7.
- **Removed dead code** — `build_files_in_context()` and `shorten_path()` (flagged by audit as unused)
- **Updated claims registry** — 5 stale claims marked fixed, 5 new v2.6/v2.7 claims added, LangChain/CrewAI/AutoGen removed
- **Updated feature maturity** — 3 signals promoted from Background Analytics to Runtime Heuristics
- **Assistant boundary docs** — new bitmill page covering Claude Code + Gemini CLI ownership boundaries
- **Docs visibility filter** — `public: false` frontmatter hides internal pages from sidebar/search
- **Claims registry migrated** — warden docs/claims.yaml → bitmill internal page (contributor-only)
- **Enhanced `warden cleanup`** — now detects and removes stale global files (warden.db, daemon shadows, legacy JSON)
- **Enhanced `warden doctor`** — warns about stale files, auto-starts server if not running before health check

## [2.7.0] - 2026-03-28

### Per-Project Storage & CLI Hygiene
- **Per-project redb** — each project gets its own `warden.redb` in `~/.warden/projects/{hash8}/`, replacing the single global `warden.db`. Eliminates cross-project lock contention.
- **Session notes → redb primary** — `session-notes.jsonl` is now written to redb events table first, JSONL only as fallback. All readers (doctor intelligence, MCP, dream, export) updated to prefer redb.
- **Auto-migration** — old `warden.db` → `warden.redb` rename on first open
- **`warden cleanup`** — new command scans for stale project directories (>30 days), supports `--dry-run`, `--force`, `--days N`
- **Doctor command fixed** — removed stale "Daemon binary missing" check, all output now says "Server" instead of "Daemon"
- **CLI daemon→server** — all user-facing messages purged of "daemon" terminology
- **Harbor bridge simplified** — removed aspirational LangChain/CrewAI/AutoGen stubs, kept webhook (shipped)
- **Bitmill pager fixed** — first docs page no longer shows itself as "next" link (robust slug matching + index guard)

## [2.6.0] - 2026-03-28

### Audit Grounding Pass
- README overhauled to stable stub — dynamic version badge, no drifting content, full docs at bitmill.dev
- Added `docs/feature-maturity.md` — 4-tier classification (deterministic, heuristic, analytics, experimental)
- Strengthened `doctor intelligence` — last turn, injected/silent, effectiveness scores, trust+budget display
- Added advisory selection logging to session-notes.jsonl (selected vs dropped categories)
- Added dream score change logging (intervention effectiveness updates)
- Enriched MCP `session_status` — focus score, advisory budget, goal, forecast, effectiveness scores
- Added telemetry flag documentation (11 flags with runtime effect + visibility)
- Expanded parity script — prohibited wording check, feature maturity validation, dream task honesty
- Updated claims registry — removed onboarding, softened Project DNA language
- 6 new intelligence tests (context switch, budget, forecast, struggling phase, reinjection, dream scores)

### Bitmill Docs
- Dream task table now shows Active/Stub status with cross-links to Feature Maturity page
- New Feature Maturity docs page with 4-tier classification
- Softened "Project DNA" to "per-project baselines" throughout
- Version bumped to v2.6.0

## [2.5.0] - 2026-03-27

### Trust & Sync Repair
- Audited 31 public claims against codebase, created claims registry (docs/claims.yaml)
- Fixed version badge, latency claims (~2ms → ~10ms), config tiers (4 → 3)
- Removed unimplemented progressive onboarding claim
- Reframed intelligence features as heuristics, removed unverified algorithms
- Enriched check_file MCP: working set, syntax coverage, generated files, error history
- Added `doctor intelligence` subcommand
- Added docs/code parity CI script
- Added intelligence fixture tests
- Fixed CI test failures: strip CI env vars so session tracking works on GitHub Actions

## [2.4.0] - 2026-03-27

### Unified Binary Architecture
- Relay rewritten as IPC client — connects directly to warden server via named pipe (~10ms)
- Server entry point (`__server`) — warden.exe runs as persistent background server
- No more daemon binary copy/shadow-copy — server IS warden.exe
- dispatch.rs simplified — no daemon fast-path, direct execution for fallback
- Dream worker thread runs inside server (same as before)

### Cold Start Fix
- stdin read timeout (5s) prevents deadlock on first hook call
- install_binary() stops server before copying

### Testing & Diagnostics
- Compass E2E tests (5 phase transition scenarios)
- Performance benchmarks (pretool 49ms, posttool 28ms, userprompt 21ms)
- Redb diagnostics table (flight recorder, 500-entry ring buffer)
- `warden redb` CLI (stats, diagnostics, events, dump)
- `warden state` command for cross-platform test support

### Engine & Architecture
- Engine trait: ReflexEngine + AnchorEngine implement process()
- SignalCategory: added Learn, Integration
- FocusCritical signal at score < 20
- Gatekeeper confidence scoring + `warden allow` appeal
- Webhook bridge (fire-and-forget HTTP POST)
- Timeout/failure prediction in Anchor ledger

### CLI
- New: status, allow, daemon-start, daemon-restart, session list/end, redb, state
- CLI internalization: rule counts + daemon health logged at session boundaries

### Docs & Legal
- License changed from MIT to AGPL-3.0
- Bitmill: privacy/local-only messaging, phase names aligned, version badge
- Normalization pipeline documented (whitespace, quotes, compound, alias)

### CI
- Pinned Rust 1.93.0 for consistent fmt/clippy
- Cross-platform test helpers (warden state command, no hash computation)

## [2.0.0] - 2026-03-26

### Architecture — 4-Engine Model

Warden's 95 modules reorganized into 4 named engines with clear ownership and purpose.

**Reflex Engine** — Act Now (<50ms)
- Sentinel: safety + hallucination pattern matching (~300 patterns)
- Loopbreaker: 2/3-gram detection, read spirals, entropy, action novelty
- Tripwire: injection detection, variable expansion bypass
- Gatekeeper: central decision trait (interface defined, implementation future)

**Anchor Engine** — Stay Grounded (<100ms)
- Compass: 5 session phases + 8 adaptive parameters + goal tracking
- Focus: composite 0-100 focus score
- Ledger: turn-by-turn event tracking (edits, reads, errors, milestones)
- Debt: verification tracking (edits since last build/test)
- Trust: composite 0-100 trust score, gates injection budget

**Dream Engine** — Learn Quietly (async)
- Imprint: error clustering + anomaly baselines (Welford's algorithm)
- Trace: successful sequence mining + repair pattern learning
- Lore: convention learning + cross-project knowledge + cross-session errors
- Pruner: effectiveness scoring + artifact decay + cleanup
- Replay: resume packet generation + working set ranking
- Budget enforcement: every task has max_events, max_ms, max_artifacts

**Harbor Engine** — Connect
- Adapter: trait Assistant (Claude Code, Gemini CLI, future assistants)
- MCP: 6-tool JSON-RPC 2.0 server (bidirectional)
- CLI: describe, explain, export, replay, tui, proc_mgmt
- Bridge: scaffold for LangChain, CrewAI, AutoGen integrations

### Added
- `engines/signal.rs` — Signal, SignalCategory, Verdict, Budget shared types
- Signal wrappers on all analytics modules (typed `Signal { category, utility, message }`)
- Budget enforcement on all 10 Dream tasks via `DreamTask::budget()`
- Candle semantic embeddings behind `[features] semantic` flag (compiles on Windows)
- `release-ship.yml` — automated version bump → tag → release workflow

### Changed
- Interactive `warden update` — prompts to apply, `--check` for print-only, `--yes` for CI
- Precompact no longer re-injects full rules file (~1000 lines saved per compaction)
- Daemon mtime check detects NEW rules files (was broken when startup_mtime=0)
- README: v2.0.0 badge, 224+ tests, 4-engine architecture diagram
- Bitmill docs: 18 pages (was 9), View Transitions, engine architecture pages

### Removed
- `negative_memory.rs` — dead code, duplicate of dream E6 build_dead_end_memory

### Fixed
- Integration test assertions: version_output (dynamic), help_output (flexible tagline)
- XSS in bitmill search modal (HTML entity escaping)
- Event listener accumulation on Astro View Transitions
- `user-select: none` on body removed (was blocking text selection)

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
