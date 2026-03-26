// ─── Harbor Engine — "Connect" ────────────────────────────────────────────────
//
// Integration layer: everything that connects Warden to the outside world.
// Assistant adapters, MCP protocol, CLI commands, and future tool bridges.
//
// Modules:
//   Adapter         — trait Assistant (Claude Code, Gemini CLI, future assistants)
//   MCP             — bidirectional MCP server (6 tools via JSON-RPC 2.0)
//   CLI commands    — describe, explain, export_sessions, replay, tui, proc_mgmt
//   Bridge          — future integrations (LangChain, CrewAI, AutoGen, webhooks)
// ──────────────────────────────────────────────────────────────────────────────

pub mod adapter;
pub mod mcp;
pub mod describe;
pub mod explain;
pub mod export_sessions;
pub mod replay;
pub mod tui;
pub mod proc_mgmt;
pub mod bridge;
