# Pipeline Stages Reference

Warden processes every `pretool-bash` hook through an ordered pipeline of checks. Each stage either passes through, denies the command, or transforms it. The first deny or transform short-circuits the pipeline.

## Architecture

The pipeline is built on the `Middleware` trait:

```rust
pub trait Middleware: Send + Sync {
    fn name(&self) -> &'static str;
    fn enabled(&self, ctx: &PipelineContext) -> bool { true }
    fn process(&self, ctx: &mut PipelineContext) -> StageResult;
}
```

`StageResult` variants:
- **Continue** -- proceed to next stage
- **Deny(msg)** -- block the command, short-circuit
- **Allow(advisory)** -- approve, optionally with advisory message, short-circuit
- **Skip** -- stage does not apply

Every stage is panic-isolated via `catch_unwind`. A panicking stage is logged and skipped (fail-open design). Warden never blocks the AI assistant due to an internal error.

## The 10 Stages (pretool-bash)

### Stage -1: Health Gate

Denies HTTP requests (xh, curl, wget) targeting `localhost:PORT` when the managed process on that port is unhealthy. Prevents wasted tool calls against crashed services.

**Result:** DENY if target process is unhealthy. PASS otherwise.

### Stage 0: cd+just Transform

Rewrites `cd /path && just recipe` to `just recipe`. The `cd` is unnecessary because just walks up the directory tree to find the Justfile, and recipes have working-directory annotations.

**Result:** TRANSFORM (allow with updated command). PASS if no match.

### Stage 1: Just Passthrough

Commands starting with `just ` skip directly to truncation (stage 7). Since just recipes are user-defined and trusted, they bypass all safety/substitution checks.

**Result:** Jump to stage 7. PASS if command does not start with `just `.

### Stage 2: Safety Check

Matches against compiled + TOML safety patterns. Blocks destructive system operations:
- `rm -rf` on broad paths
- `sudo` / privilege escalation
- All mutating git commands (add, commit, push, pull, merge, rebase, checkout, etc.)
- `chmod 777`, disk formatting, registry editing, firewall modification

Exclusions: `git clean --dry-run`, `git stash list`, `git stash show` are allowed.

**Result:** DENY with explanation. PASS if no match.

### Stage 2.5: Hallucination Hardening

Blocks commands that an AI might hallucinate but a human would never intentionally run:
- Reverse shell patterns (`/dev/tcp`, `mkfifo`)
- Credential piping to external services
- SSH key/config writes
- Environment variable exfiltration
- Base64-encoded data piped to network
- npm publish, pip install from arbitrary URLs
- SUID bit setting, crontab modification, /etc/hosts editing

**Result:** DENY. These are never disableable (safety-critical).

### Stage 2.75: Hallucination Advisory

Suspicious-but-possibly-legitimate patterns that get a non-blocking advisory:
- Deep directory traversal
- Shell config writes (.bashrc, .zshrc)
- Global package installation
- Docker system prune
- System service restarts

**Result:** ALLOW with advisory message.

### Stage 2.8: Control Character Detection

Blocks commands containing embedded control characters (null bytes, escape sequences, etc.) that could indicate prompt injection or corrupted input.

**Result:** DENY if suspicious characters found. PASS otherwise.

### Stage 3: Destructive Check

Blocks tools that auto-modify code in ways that need explicit human approval:
- `knip --fix` (auto-deletes unused exports)
- `sg -r` (AST rewrite mode, unless `--dry-run` is present)
- `madge --image` (overwrites dependency graph file)

**Result:** DENY with safe alternative. PASS if no match.

### Stage 4: Zero-Trace Check

Blocks AI attribution text in write-like commands (`echo`, `printf`, `tee`, `>>`). Matches patterns like "claude", "copilot", "ai-generated", "llm" when combined with output redirection. Excludes paths inside `.claude/` directories.

**Result:** DENY with instruction to remove attribution. PASS if no match.

### Stage 5: Substitution Check

Enforces CLI tool substitutions by denying banned commands with a redirect message:
- `grep` --> `rg`
- `find` --> `fd`
- `curl` --> `xh`
- `cat` --> `bat` (via pretool-redirect for the Cat tool)
- `ts-node` --> `tsx`
- `du` --> `dust`
- `sort | uniq` / `sort -u` --> `huniq`
- `sd` --> Edit tool (Windows only, sd mangles newlines)

Each denial includes the replacement command syntax.

**Result:** DENY with redirect message. PASS if no match.

### Stage 5.5: Pre-Execution Dedup

Checks if an identical command was already run with no file edits since. Two modes:
- **Read-only commands** (git status, git diff, etc.): Hard DENY with "output is still in your context"
- **Other commands**: ALLOW with advisory suggesting to skip

Tracks estimated token savings from avoided duplicate execution.

**Result:** DENY (read-only) or ALLOW+advisory (other). PASS if not a duplicate.

### Stage 5.75: No-Op Build Detection

Detects build/test commands that would be no-ops because no source files were edited since the last successful build. Issues an advisory rather than a hard deny.

**Result:** ALLOW with advisory. PASS if edits occurred since last build.

### Stage 6: Just-First Transform

When a Justfile exists in the project, transforms raw commands to their just recipe equivalents based on prefix mapping (e.g., `npm run build` --> `just build`). Mappings come from compiled defaults + TOML overrides.

**Result:** TRANSFORM, DENY, or ADVISORY depending on mapping. PASS if no match or no Justfile.

### Stage 6.5: Advisory Patterns

Non-blocking hints for commands that work but have better alternatives:
- `docker ps/logs/inspect` --> docker MCP tools
- `rg` for symbol lookups --> aidex_query
- `rg` for structural patterns --> ast-grep (sg)
- Piped `awk/sed/cut` --> jc for structured JSON
- `rg` for markdown structure --> mdq

**Result:** ALLOW with advisory message. PASS if no match.

### Stage 7: Truncation / Auto-Allow

Final stage. Determines how to handle the command:
1. **Piped commands** (contains `|` outside quotes): auto-allow (already piped)
2. **Compact tools** (rg, fd, etc.): auto-allow (output is naturally compact)
3. **Short commands** (version checks, etc.): auto-allow
4. **Verbose commands** (builds, tests, installs): wrap with `warden truncate-filter --mode MODE`
5. **Auto-allow patterns** (git read-only, cargo build, etc.): auto-allow
6. **Unknown**: silent passthrough to the assistant's permission system

Truncation modes: `test`, `build`, `install`, `default` -- each has tailored output reduction.

**Result:** ALLOW, TRANSFORM (truncation wrap), or PASS.

## Adding a Custom Stage

Custom stages are added via TOML rules, not code. Each pattern section maps to a specific pipeline stage:

| TOML Section            | Stage     | Severity    |
|------------------------|-----------|-------------|
| `[safety]`             | Stage 2   | HardDeny    |
| `[hallucination]`      | Stage 2.5 | HardDeny    |
| `[hallucination_advisory]` | Stage 2.75 | Advisory |
| `[destructive]`        | Stage 3   | HardDeny    |
| `[substitutions]`      | Stage 5   | HardDeny    |
| `[advisories]`         | Stage 6.5 | Advisory    |
| `[auto_allow]`         | Stage 7   | Allow       |

Example -- add a custom substitution in `~/.warden/rules/personal.toml`:

```toml
[substitutions]
patterns = [
    { match = '\bwget\b', msg = "Use xh instead of wget" },
]
```

This appends to compiled defaults. Set `replace = true` to override them entirely.

## Stage Ordering and Short-Circuit

Stages run in strict numeric order. The first DENY or ALLOW short-circuits -- no subsequent stages execute. This means:

1. **Safety always wins.** A command blocked by safety (stage 2) never reaches substitutions (stage 5).
2. **Just recipes are trusted.** `just ` commands (stage 1) skip all safety checks.
3. **Transforms before denials.** cd+just normalization (stage 0) happens before safety.
4. **Advisories are last.** Non-blocking hints (stages 6.5, 7) only fire if nothing else matched.

The three-tier merge order also matters: compiled defaults < `~/.warden/rules/personal.toml` < `.warden/rules.toml`. Project rules have final say.

## Performance

Each stage is designed for sub-millisecond execution:

- All regex patterns are compiled once via `LazyLock` and reused across calls
- The daemon keeps compiled patterns in memory (no re-parsing per hook call)
- Typical total pipeline time: **0.1-0.5ms** for the full 10-stage pretool-bash pipeline
- Panic isolation adds negligible overhead (only allocates on actual panic)
- Per-stage timing is recorded in `PipelineContext.timings` for profiling

The IPC daemon path (named pipe on Windows, Unix socket on Linux/macOS) adds ~0.2ms round-trip, keeping total hook latency under 1ms in practice.
