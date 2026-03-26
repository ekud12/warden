// ─── Tripwire — High-risk pattern detection ──────────────────────────────────
//
// Catches sophisticated bypass attempts: variable expansion ($VAR -rf),
// backtick wrapping, base64 encoding, prompt injection, social engineering.
// Source: config/core/injection.rs, pretool_bash check_expansion_risk()
//
// Future: advanced hallucination hardening, unicode homoglyph detection.
// ──────────────────────────────────────────────────────────────────────────────
