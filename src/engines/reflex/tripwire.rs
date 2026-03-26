// ─── Tripwire — High-risk pattern detection ──────────────────────────────────
//
// Catches sophisticated bypass attempts: variable expansion ($VAR -rf),
// backtick wrapping, base64 encoding, prompt injection, social engineering.
// Source: config/core/injection.rs, pretool_bash check_expansion_risk()
//
// Future: advanced hallucination hardening, unicode homoglyph detection.
//
// TODO: Consider absorbing injection patterns from config/core/injection.rs.
// That module defines INJECTION_PATTERNS (regex, category) pairs for prompt
// injection detection. Tripwire is the natural home for all high-risk pattern
// matching — keeping injection patterns separate in config/core/ fragments the
// detection logic.
// ──────────────────────────────────────────────────────────────────────────────
