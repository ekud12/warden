// ─── common::storage — redb embedded database abstraction ────────────────────
//
// Single-file ACID storage replacing JSON files. Each project gets one
// `warden.redb` in its project directory.
//
// Tables:
//   session_state  — current session state (key: "current")
//   events         — session event log (key: timestamp nanos)
//   project_stats  — accumulated project statistics
//   effectiveness  — per-rule quality delta data
//   filters        — command filter rules (Phase 3)
//
// Falls back to JSON if redb fails to open (fail-open principle).
// ──────────────────────────────────────────────────────────────────────────────

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

const STATE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("session_state");
const EVENTS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("events");
const STATS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("stats");
const EFFECTIVENESS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("effectiveness");
const FILTERS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("filters");
const DREAM_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("dream");
const RESUME_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("resume_packets");
/// Flight recorder: structured diagnostic events (errors, timings, unexpected states).
/// Key: timestamp nanos. Value: JSON blob. Bounded to last 500 entries.
const DIAGNOSTICS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("diagnostics");

/// Cached DB handle — opened once, reused across calls.
/// Avoids the overhead of `Database::create()` on every read/write.
static DB_HANDLE: LazyLock<Mutex<Option<(PathBuf, Database)>>> = LazyLock::new(|| Mutex::new(None));

/// Open the database for a project directory. Stores path for future access.
pub fn open_db(project_dir: &Path) -> Option<()> {
    // Migrate legacy warden.db → warden.redb if needed
    migrate_db_rename(project_dir);

    let db_path = project_dir.join("warden.redb");

    // Create tables on first open
    let db = Database::create(&db_path).ok()?;
    let write_txn = db.begin_write().ok()?;
    {
        let _ = write_txn.open_table(STATE_TABLE);
        let _ = write_txn.open_table(EVENTS_TABLE);
        let _ = write_txn.open_table(STATS_TABLE);
        let _ = write_txn.open_table(EFFECTIVENESS_TABLE);
        let _ = write_txn.open_table(FILTERS_TABLE);
        let _ = write_txn.open_table(DREAM_TABLE);
        let _ = write_txn.open_table(RESUME_TABLE);
        let _ = write_txn.open_table(DIAGNOSTICS_TABLE);
    }
    write_txn.commit().ok()?;

    if let Ok(mut handle) = DB_HANDLE.lock() {
        *handle = Some((db_path.clone(), db));
    }
    Some(())
}

/// Execute a closure with a reference to the cached Database handle.
/// Returns None if the DB isn't open yet.
fn with_db<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Database) -> Option<R>,
{
    let guard = DB_HANDLE.lock().ok()?;
    let (_, db) = guard.as_ref()?;
    f(db)
}

/// Read a value from a named table
pub fn read_key(table_name: &str, key: &str) -> Option<Vec<u8>> {
    let table_def = resolve_table(table_name)?;
    with_db(|db| {
        let read_txn = db.begin_read().ok()?;
        let table = read_txn.open_table(table_def).ok()?;
        let value = table.get(key).ok()??;
        Some(value.value().to_vec())
    })
}

/// Write a value to a named table
pub fn write_key(table_name: &str, key: &str, value: &[u8]) -> Option<()> {
    let table_def = resolve_table(table_name)?;
    with_db(|db| {
        let write_txn = db.begin_write().ok()?;
        {
            let mut table = write_txn.open_table(table_def).ok()?;
            table.insert(key, value).ok()?;
        }
        write_txn.commit().ok()?;
        Some(())
    })
}

/// Append an event to the events table (keyed by timestamp nanos)
pub fn append_event(value: &[u8]) -> Option<()> {
    with_db(|db| {
        let write_txn = db.begin_write().ok()?;
        {
            let mut table = write_txn.open_table(EVENTS_TABLE).ok()?;
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            table.insert(ts, value).ok()?;
        }
        write_txn.commit().ok()?;
        Some(())
    })
}

/// Append a diagnostic entry to the flight recorder.
/// Used for internal errors, unexpected states, handler timings, and mishaps.
/// Bounded to 500 entries (old entries pruned on write).
pub fn append_diagnostic(category: &str, detail: &str) -> Option<()> {
    with_db(|db| {
        let write_txn = db.begin_write().ok()?;
        {
            let mut table = write_txn.open_table(DIAGNOSTICS_TABLE).ok()?;
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            let entry = serde_json::json!({
                "ts": ts,
                "cat": category,
                "detail": detail,
            });
            if let Ok(bytes) = serde_json::to_vec(&entry) {
                table.insert(ts, bytes.as_slice()).ok()?;
            }

            // Prune: keep only last 500 entries (count via iter)
            let count = table.iter().ok().map(|i| i.count()).unwrap_or(0);
            if count > 500 {
                let to_remove: Vec<u64> = table
                    .iter()
                    .ok()
                    .map(|iter| {
                        iter.filter_map(|e| e.ok().map(|(k, _)| k.value()))
                            .take(count - 500)
                            .collect()
                    })
                    .unwrap_or_default();
                for key in to_remove {
                    let _ = table.remove(key);
                }
            }
        }
        write_txn.commit().ok()?;
        Some(())
    })
}

/// Read the last N diagnostic entries (reverse iteration — no full table scan)
pub fn read_last_diagnostics(count: usize) -> Vec<serde_json::Value> {
    with_db(|db| {
        let read_txn = db.begin_read().ok()?;
        let table = read_txn.open_table(DIAGNOSTICS_TABLE).ok()?;
        let iter = table.iter().ok()?;
        let mut results: Vec<serde_json::Value> = iter
            .rev()
            .take(count)
            .filter_map(|entry| {
                entry
                    .ok()
                    .and_then(|(_, v)| serde_json::from_slice(v.value()).ok())
            })
            .collect();
        results.reverse(); // restore chronological order
        Some(results)
    })
    .unwrap_or_default()
}

/// Read the last N events (reverse iteration — no full table scan)
pub fn read_last_events(count: usize) -> Vec<Vec<u8>> {
    with_db(|db| {
        let read_txn = db.begin_read().ok()?;
        let table = read_txn.open_table(EVENTS_TABLE).ok()?;
        let iter = table.iter().ok()?;
        let mut results: Vec<Vec<u8>> = iter
            .rev()
            .take(count)
            .filter_map(|entry| entry.ok().map(|(_, v)| v.value().to_vec()))
            .collect();
        results.reverse(); // restore chronological order
        Some(results)
    })
    .unwrap_or_default()
}

/// Read a typed value from the DB, deserializing from JSON
pub fn read_json<T: serde::de::DeserializeOwned>(table: &str, key: &str) -> Option<T> {
    let bytes = read_key(table, key)?;
    serde_json::from_slice(&bytes).ok()
}

/// Write a typed value to the DB, serializing as JSON
pub fn write_json<T: serde::Serialize>(table: &str, key: &str, value: &T) -> Option<()> {
    let bytes = serde_json::to_vec(value).ok()?;
    write_key(table, key, &bytes)
}

/// Check if the DB is open and available
pub fn is_available() -> bool {
    DB_HANDLE.lock().ok().map(|h| h.is_some()).unwrap_or(false)
}

/// Get the DB file path for a project directory
pub fn db_path(project_dir: &Path) -> PathBuf {
    project_dir.join("warden.redb")
}

/// Close the database
pub fn close() {
    if let Ok(mut handle) = DB_HANDLE.lock() {
        *handle = None;
    }
}

/// Resolve table name to definition
fn resolve_table(name: &str) -> Option<TableDefinition<'static, &'static str, &'static [u8]>> {
    match name {
        "session_state" => Some(STATE_TABLE),
        "stats" => Some(STATS_TABLE),
        "effectiveness" => Some(EFFECTIVENESS_TABLE),
        "filters" => Some(FILTERS_TABLE),
        "dream" => Some(DREAM_TABLE),
        "resume_packets" => Some(RESUME_TABLE),
        _ => None,
    }
}

/// Migrate legacy `warden.db` to `warden.redb` (rename only, same format).
/// Called on open_db when old file exists but new one doesn't.
pub fn migrate_db_rename(project_dir: &Path) {
    let old = project_dir.join("warden.db");
    let new = project_dir.join("warden.redb");
    if old.exists() && !new.exists() {
        let _ = std::fs::rename(&old, &new);
    }
}

/// Migrate existing JSON files into the database
pub fn migrate_from_json(project_dir: &Path) {
    let state_path = project_dir.join("session-state.json");
    if state_path.exists()
        && let Ok(content) = std::fs::read(&state_path)
    {
        let _ = write_key("session_state", "current", &content);
    }

    let stats_path = project_dir.join("stats.json");
    if stats_path.exists()
        && let Ok(content) = std::fs::read(&stats_path)
    {
        let _ = write_key("stats", "project", &content);
    }

    let eff_path = project_dir.join("rule-effectiveness.json");
    if eff_path.exists()
        && let Ok(content) = std::fs::read(&eff_path)
    {
        let _ = write_key("effectiveness", "rules", &content);
    }

    let notes_path = project_dir.join("session-notes.jsonl");
    if notes_path.exists()
        && let Ok(content) = std::fs::read_to_string(&notes_path)
    {
        for line in content.lines() {
            if !line.trim().is_empty() {
                let _ = append_event(line.as_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All redb tests must run serially since they share the global DB_PATH
    #[test]
    fn redb_all_tests_serial() {
        // Test 1: write/read roundtrip
        {
            let dir = std::env::temp_dir().join("warden_redb_t1");
            let _ = std::fs::create_dir_all(&dir);
            close();
            assert!(open_db(&dir).is_some(), "open_db failed");

            let data = b"hello world";
            assert!(write_key("session_state", "test_key", data).is_some());
            let result = read_key("session_state", "test_key");
            assert_eq!(result, Some(data.to_vec()));

            close();
            let _ = std::fs::remove_dir_all(&dir);
        }

        // Test 2: JSON typed roundtrip
        {
            let dir = std::env::temp_dir().join("warden_redb_t2");
            let _ = std::fs::create_dir_all(&dir);
            close();
            assert!(open_db(&dir).is_some(), "open_db failed");

            let value = serde_json::json!({"quality": 85, "turns": 20});
            assert!(write_json("stats", "test", &value).is_some());
            let result: Option<serde_json::Value> = read_json("stats", "test");
            assert_eq!(result.unwrap()["quality"], 85);

            close();
            let _ = std::fs::remove_dir_all(&dir);
        }

        // Test 3: append and read events
        {
            let dir = std::env::temp_dir().join("warden_redb_t3");
            let _ = std::fs::create_dir_all(&dir);
            close();
            assert!(open_db(&dir).is_some(), "open_db failed");

            for i in 0..5 {
                let event = format!("event_{}", i);
                assert!(append_event(event.as_bytes()).is_some());
                std::thread::sleep(std::time::Duration::from_millis(2));
            }

            let events = read_last_events(3);
            assert_eq!(events.len(), 3);

            close();
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
