# Context Efficiency

AI coding agents have a fixed context window. Every token of noise that enters the context is a token of useful information that gets pushed out. Warden's Output Compression system reduces this waste.

## The Problem

Common commands produce enormous output that the agent does not need:

| Command | Raw Output | Useful Information |
|---------|-----------|-------------------|
| `cargo test` (200 passing) | ~4,000 lines | "200 passed, 0 failed" |
| `npm install` | ~500 lines | "added 847 packages" |
| `git diff` (large) | ~2,000 lines | Changed files + key hunks |
| `cargo build` (clean) | ~100 lines | "Compiling... Finished" |

Without compression, these outputs fill the context window with progress bars, dependency trees, and passing test details that have zero value for the next decision.

## How Compression Works

Warden's smart filters run in the `truncate-filter` pipeline stage, after a command completes but before its output reaches the agent's context.

Each filter is matched by command pattern and applies a strategy:

### Strategy: Strip Matching

Remove lines matching specific patterns. Used for progress indicators, download bars, and compilation noise.

**Example:** `cargo build` filter strips "Compiling..." lines, keeps errors and the final summary.

### Strategy: Keep Matching

Keep only lines matching specific patterns. Used when the signal is sparse in a sea of noise.

**Example:** `cargo test` filter keeps failure details, the summary line, and any compiler errors. Strips all passing test output.

### Strategy: Head/Tail

Keep the first N and last N lines, strip the middle. Used for commands with useful headers and summaries but repetitive middles.

### Strategy: Dedup

Remove duplicate or near-duplicate lines. Used for commands that repeat similar output patterns.

## Built-In Filters

Warden ships with optimized filters for common developer commands:

| Command | Compression | What's Kept |
|---------|-------------|-------------|
| `cargo test` | 90-99% | Failures + summary line |
| `cargo build` | 60-80% | Errors + warnings + summary |
| `cargo clippy` | 70-90% | Warnings + summary |
| `npm install` | 80-95% | Added/removed summary |
| `git diff` | 40-70% | Hunks + stat summary |
| `pip install` | 70-90% | Successfully installed |
| `dotnet build` | 60-80% | Errors + warnings |
| `docker build` | 50-70% | Step markers + errors |

## Example: Before and After

### cargo test (262 passing, 0 failing)

**Before (raw output):** ~4,200 tokens

```
   Compiling my-crate v0.1.0
   Compiling dependency-a v1.2.3
   ... (50 more compilation lines)
running 262 tests
test module_a::test_one ... ok
test module_a::test_two ... ok
... (260 more "ok" lines)
test result: ok. 262 passed; 0 failed; 0 ignored
```

**After (Warden compressed):** ~50 tokens

```
cargo test: 262 passed, 0 failed (262 lines compressed to summary)
```

### cargo test (1 failing)

**Before:** ~4,200 tokens
**After:** ~200 tokens — shows only the failure details and summary

```
FAILED: test module_b::test_edge_case
  thread panicked at 'assertion failed: x > 0'
  src/module_b.rs:42

test result: FAILED. 261 passed; 1 failed; 0 ignored
(261 passing tests compressed)
```

## Token Savings Tracking

Warden tracks cumulative token savings across the session:

- **Dedup savings** — tokens saved by suppressing duplicate reads
- **Deny savings** — tokens saved by blocking unnecessary operations
- **Truncation savings** — tokens saved by output compression
- **Build skip savings** — tokens saved by suppressing redundant builds

These are reported in the session summary at `warden debug-stats`.

## Custom Filters

Add custom command filters in `rules.toml`:

```toml
[[command_filters]]
match = "my-custom-build"
strategy = "keep_matching"
keep_patterns = ["^ERROR", "^warning", "^Build (succeeded|failed)"]
summary_template = "my-custom-build: {kept}/{total} lines kept"
max_lines = 40
```

### Filter Options

| Field | Description | Default |
|-------|-------------|---------|
| `match` | Regex to match command string | (required) |
| `strategy` | `strip_matching`, `keep_matching`, `dedup`, `head_tail`, `passthrough` | `strip_matching` |
| `keep_patterns` | Regex patterns for lines to keep | `[]` |
| `strip_patterns` | Regex patterns for lines to strip | `[]` |
| `keep_first` | Lines to keep from start | 3 |
| `keep_last` | Lines to keep from end | 3 |
| `summary_template` | Summary line template (`{kept}`, `{total}`, `{stripped}` placeholders) | `""` |
| `max_lines` | Maximum output lines after filtering | 40 |

## Adaptive Compression

Warden adjusts compression aggressiveness based on session phase:

- **Early session:** Lighter compression — more context for understanding the codebase
- **Mid session:** Standard compression — balanced signal and noise
- **Late session:** Aggressive compression — every token counts near context limits

This happens automatically based on token budget tracking.

## Next Steps

- [Configuration](configuration.md) — customize filters and thresholds
- [Session Intelligence](session-intelligence.md) — how Warden tracks session health
- [Pipeline Stages](pipeline-stages.md) — where compression fits in the pipeline
