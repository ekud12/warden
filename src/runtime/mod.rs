// ─── runtime — Warden infrastructure layer ───────────────────────────────────
//
// The substrate underneath the 4 engines. Manages process lifecycle, IPC,
// hook dispatch, and binary health. Answers "is Warden healthy?" not
// "is the agent healthy?"
//
// dispatch.rs  — hook execution entrypoint (CI / daemon / direct)
// ipc.rs       — named-pipe protocol, daemon spawn, version handshake
// daemon.rs    — background server, idle timeout, dream worker
// ──────────────────────────────────────────────────────────────────────────────

pub mod daemon;
pub mod dispatch;
pub mod ipc;
