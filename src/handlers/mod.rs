// ─── handlers — active hook entry points ─────────────────────────────────────
//
// Only ACTIVE hook handlers remain here. Shim re-exports have been removed;
// callers now use canonical engines:: paths directly.
//
// Migrated to engines:
//   adaptation → engines::anchor::compass
//   session_start → engines::anchor::session_start
//   session_end → engines::anchor::session_end
//   precompact_memory → engines::anchor::precompact
//   postcompact → engines::anchor::postcompact
//   token_budget → engines::anchor::budget
//   git_summary → engines::anchor::git_summary
//   posttool_session → engines::anchor::ledger
//   learning → engines::dream::lore
//   cross_session → engines::dream::lore
//   describe → engines::harbor::describe
//   explain → engines::harbor::explain
//   export_sessions → engines::harbor::export_sessions
//   mcp_server → engines::harbor::mcp
//   replay → engines::harbor::replay
//   tui → engines::harbor::tui
//   proc_mgmt → engines::harbor::proc_mgmt
// ──────────────────────────────────────────────────────────────────────────────

pub mod auto_changelog;
pub mod config_override;
pub mod git_guardian;
pub mod permission_approve;
pub mod postfailure_guide;
pub mod posttool_mcp;
pub mod pretool_bash;
pub mod pretool_read;
pub mod pretool_redirect;
pub mod pretool_write;
pub mod smart_filter;
pub mod stop_check;
pub mod subagent_context;
pub mod subagent_stop;
pub mod task_completed;
pub mod truncate_filter;
pub mod userprompt_context;
