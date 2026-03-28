# Feature Maturity

Every significant feature in Warden is classified into one of four maturity levels. This classification determines how the feature is documented, marketed, and tested.

## Deterministic (always active, tested, observable)

These features run on every tool call with no configuration required. They are tested in CI and produce deterministic results.

- **Rule enforcement** — safety (50 patterns), hallucination (48+20), substitution (11), injection (38), auto-allow (67), sensitive paths (27), error hints (28)
- **Output compression** — data-driven filter engine, 8 default command rules, TOML-extensible
- **Config merge** — 3-tier model (compiled → global rules.toml → project rules.toml)
- **Session state persistence** — redb ACID storage, turn tracking, file tracking
- **Progressive read governance** — advisory at turn 50, deny at turn 80 for large files
- **Syntax validation** — JSON and TOML parse validation on edit; YAML lightweight structural checks

## Runtime Heuristics (active, observable via diagnostics)

These features run in real-time and influence session behavior. They are heuristic — they use approximations rather than exact analysis. Observable via `warden doctor intelligence`, session notes, and MCP.

- **Session phase detection** — 5 phases (Warmup, Productive, Exploring, Struggling, Late), 8 adaptive parameters
- **Goal extraction** — extracts session intent from first user message (22 action verbs)
- **Loop detection** — 2-gram and 3-gram action patterns, read spirals, entropy-based exploration detection
- **Drift detection** — keyword overlap between stated goal and recent actions (Jaccard)
- **Focus scoring** — composite 0-100 score based on file-set coherence
- **Verification debt tracking** — counts edits since last build/test, triggers checkpoints
- **Context switch detection** — detects task pivots, auto-resets goals and working set
- **Error hints** — 28 pattern-matched recovery suggestions for common CLI failures
- **Anomaly detection** — Welford's online mean/variance, z-score flagging; injected when z-score > 2.5
- **Compaction forecast** — linear regression on token usage; injected when < 5 turns remaining before predicted compaction
- **Goal anchoring** — injected every 5 turns as focus signal
- **Advisory injection budget** — trust-gated (1/3/5/15 advisories), utility-ranked, dedup-filtered

## Background Analytics (computed, logged, inspectable)

These features compute and persist data but do not directly inject into the session. They are observable via MCP `session_status`, `warden doctor intelligence`, or session-notes.jsonl.

> **Note:** Anomaly detection, compaction forecast, and goal anchoring were promoted to Runtime Heuristics in v2.6 after gaining injection triggers.

- **Quality prediction** — weighted heuristic ensemble (0-100)
- **Markov transitions** — 2-gram action transition tracking and prediction
- **Topic coherence** — periodic (every 10 turns) similarity check
- **Per-project baselines** — 7 metrics tracked via Welford's algorithm (tokens, errors, edits, explore ratio, denial rate, session length, quality)
- **Intervention effectiveness scoring** — correlates advisory categories with milestone occurrence, feeds back into injection budget utility weights
- **Dream resume packets** — compact session grounding (top files, dead ends, conventions, verification debt)

## Experimental / Infrastructure (architecture exists, not hardened)

These features have code and architecture in place but are not yet producing robust, user-visible results. They are not marketed as shipped features.

- **Dream sequences** (E7: LearnSequences) — 3-gram action mining, implemented but sparse data
- **Dream repair patterns** (E8: LearnRepairPatterns) — error→fix mapping, implemented but sparse data
- **Dream conventions** (E9: LearnConventions) — project pattern learning, stub
- **Dream error clustering** (E4: ClusterErrors) — error grouping by signature, stub
- **Dream event consolidation** (E1: ConsolidateEvents) — event store health check, stub
- **Dream dead-end memory** (E6: BuildDeadEndMemory) — failed approach tracking, stub
- **Semantic embeddings** — candle feature flag compiles, Jaccard/Levenshtein fallback active
