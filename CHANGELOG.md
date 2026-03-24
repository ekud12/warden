# Changelog

## [1.0.0] - 2026-03-24

### Added
- Initial release of Warden — AI Coding Session Guardian
- **Pipeline/middleware architecture** with panic isolation
- **Multi-assistant support**: Claude Code + Gemini CLI adapters
- **MCP server mode** (`warden mcp`) — bidirectional harness, 5 tools exposed
- **221 compiled rules** across 7 categories:
  - 28 safety rules (rm -rf, sudo, chmod 777, shutdown, dd, etc.)
  - 44 hallucination prevention (reverse shells, credential theft, code injection)
  - 12 substitution rules (grep->rg, find->fd, curl->xh, etc.)
  - 13 advisory rules (docker, symbol lookups, build warnings)
  - 58 auto-allow patterns (safe read-only commands)
  - 21 sensitive path rules (SSH, GPG, AWS, Kubernetes, Docker credentials)
  - 35 prompt injection detection patterns (instruction hijack, role manipulation, exfiltration)
- **13 runtime intelligence features** (all automatic, no user action needed):
  - Phase-adaptive thresholds (Warmup, Productive, Exploring, Struggling, Late)
  - Quality prediction (heuristic ensemble at turn 10+)
  - Anomaly detection (Welford's algorithm, z-score flagging)
  - Token budget forecasting (linear regression, compaction ETA)
  - Error prevention (Bayesian transition matrices)
  - Cost tracking (token categorization: explore/implement/waste/saved)
  - Project DNA fingerprinting (per-project statistical baselines)
  - Rule effectiveness scoring (quality delta per rule)
  - Smart truncation (keyword relevance, edited-file boosting)
  - Git branch guardian (main branch warning, uncommitted tracking, co-change suggestions)
  - Auto-changelog (session narrative at end)
  - CLI command recovery (flag fixes, install suggestions for 13+ tools)
  - Drift detection (denial density monitoring, rule re-injection)
- **Tiered rules** with 4-level TOML merge (compiled → core → personal → project)
- **First-run wizard** (`warden init`): OS detection, CLI installation, hook configuration
- **PATH registration** (Windows registry + Unix shell configs)
- **CLI tool detection** with graceful degradation (substitutions auto-disabled when target missing)
- **IPC daemon** with named pipe fast-path (~2ms latency)
- **Syntax validation** after edits: JSON, TOML, YAML
- **File co-change detection** from git history
- **Session export** (`warden export-sessions --format json|csv`)
- **Live TUI dashboard** (`warden tui`) with ratatui
- **Session replay** (`warden replay`) and diff (`warden diff`)
- **Config system** (`warden config set/get/list`)
- **Restriction registry** (221 rules, runtime enable/disable)
- **Cross-platform**: Windows, macOS, Linux (x64 + ARM64)
- **107 tests** (unit + integration), zero clippy warnings
- **2.1MB binary**, ~2ms daemon latency, <5MB memory
