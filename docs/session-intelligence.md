# Session Intelligence

Warden tracks session state across every tool call, building a model of how the session is progressing. This page explains the intelligence features that detect drift, measure focus, and steer sessions toward productive outcomes.

## How It Works

Every hook event updates Warden's session state. Over the course of a session, Warden accumulates signals about:

- What files have been read and edited
- What commands have succeeded and failed
- Where milestones have been reached
- How the agent's attention is distributed
- Whether the session is making real progress

These signals feed into several analysis modules that generate targeted advisories.

## Drift Detection

Drift is the gradual loss of focus that happens in long sessions. The agent starts working on the right task, then slowly wanders into tangential exploration.

Warden detects drift through multiple signals:

### Focus Score

A composite 0-100 score measuring how focused the current session is. Penalizes:

- **Directory spread** — touching many unrelated directories
- **Subsystem switches** — jumping between areas without completing work
- **Exploration without action** — reading many files without editing

When the focus score drops below 40, Warden advises the agent to narrow scope.

### Verification Debt

Tracks the number of edits since the last successful build or test run. When edits accumulate without verification:

- At 4+ edits: "N edits since last build/test. Verify before continuing."

This prevents the common failure mode where an agent makes many changes, discovers they don't work, and has to revert.

### Checkpoint Enforcement

Tracks turns since the last milestone (successful build, test pass, or significant verification). After extended periods without a checkpoint, Warden nudges the agent to verify progress.

## Loop Detection

Agents sometimes get stuck in behavioral loops — repeating the same sequence of actions without making progress.

Warden detects three patterns:

### 2-Gram Loops

Alternating between two actions: A → B → A → B. Common example: edit a file, run tests, see the same error, edit again without changing approach.

### 3-Gram Loops

Three-step cycles: A → B → C → A → B → C. Common example: read docs, try an approach, fail, read more docs, try the same approach.

### Read Spirals

Five or more consecutive file reads without any edit. Indicates the agent is exploring without committing to an approach.

When a loop is detected, Warden advises: "Break the loop — try a different approach."

## Negative Memory

Warden remembers what did not work during the session:

- **Dead ends** — files or approaches that were explored and abandoned
- **Failed commands** — command patterns that failed repeatedly

When the agent revisits a dead end or retries a failed command pattern, Warden warns: "Previously explored X and found Y. Choose a different approach."

This prevents the agent from re-discovering the same dead ends.

## Goal Tracking

Warden tracks the session's goal at three levels:

| Level | Source | Example |
|-------|--------|---------|
| **Primary goal** | Extracted from initial user message | "Fix the authentication bug" |
| **Current subgoal** | Inferred from recent edits | "Working on auth middleware" |
| **Blocked on** | Set when errors occur | "Compilation error in auth.rs" |

The goal stack is tracked in session state and available via MCP on demand. It feeds into the dream worker's resume packet but is not injected as an advisory (silent signal).

## Session Phases

Warden automatically detects the current session phase and adjusts 8 runtime parameters:

| Phase | Characteristics | Warden Behavior |
|-------|----------------|-----------------|
| **Warmup** | First few turns | Default parameters, room to explore |
| **Productive** | Edits + milestones flowing | Relaxed limits, wider dedup window |
| **Exploring** | Many reads, few edits | Higher explore budget, nudges toward action |
| **Struggling** | Errors rising, no milestones | Tighter guardrails, more advisories |
| **Late** | High token usage | Aggressive compression, targeted reads only (one-way) |

Phase transitions happen automatically based on session signals. Hysteresis of 2 turns prevents flapping.

## Trust Score

An internal composite score (never shown to the user) that measures overall session health. When trust is high, Warden stays quiet. When trust degrades, Warden increases intervention frequency.

Trust factors:

- Unresolved errors (negative)
- Edits without verification (negative)
- Subsystem switching (negative)
- Dead ends accumulated (negative)
- Milestones reached (positive)
- Consistent progress (positive)

## Anomaly Detection

Warden maintains per-project statistical baselines using Welford's online algorithm. When session metrics deviate significantly from the project's historical norms, anomaly alerts fire:

- Unusually high token usage per turn
- Abnormally high error rate
- Exploration ratio outside normal range

## Injection Budget

Not all intelligence signals reach the agent. Warden uses a trust-gated injection budget to control how many advisories are injected per turn:

| Trust Score | Budget | Meaning |
|-------------|--------|---------|
| > 85 | Top 1 | Healthy session — almost silent |
| 50-85 | Top 3 | Normal — selective intervention |
| 25-50 | Top 5 | Degraded — more intervention |
| < 25 | Up to 15 | Struggling — uncapped |

All 29 analytics still compute and write to redb every turn — nothing is lost. The budget only controls what gets injected into the agent's context. Seven consolidated signals compete for budget slots: safety, loop, verification, phase, recovery, focus, and pressure.

Dream-informed utility adjustment multiplies each signal's score by its historical effectiveness (learned from advisory → milestone patterns). Ineffective advisories get quieter over time.

## Runtime Analytics

All intelligence features run automatically. No configuration is needed for default behavior. Individual features can be disabled via the telemetry configuration:

```toml
# ~/.warden/config.toml
[telemetry]
anomaly_detection = true
quality_predictor = true
focus_tracking = true
# ... (all default to true)
```

## Next Steps

- [Context Efficiency](context-efficiency.md) — how Warden reduces token waste
- [Configuration](configuration.md) — tune thresholds and disable features
- [Architecture](architecture.md) — technical details of the intelligence pipeline
