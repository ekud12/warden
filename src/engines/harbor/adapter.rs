// ─── Adapter — Assistant abstraction layer ────────────────────────────────────
//
// Defines how Warden talks to different AI coding assistants. Each assistant
// has its own env detection, settings path, deny format, and context format.
//
// Current adapters: Claude Code, Gemini CLI
// Future: Cursor, Windsurf, Cline, Aider, Continue.dev, Zed AI
//
// Source: src/assistant/ (trait Assistant + implementations)
// The existing assistant/ module is the canonical implementation.
// This module will re-export and extend it as Harbor grows.
// ──────────────────────────────────────────────────────────────────────────────

pub use crate::assistant::*;
