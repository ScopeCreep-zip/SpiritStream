//! File-backed, hashed, TTL'd token sets shared across processes.
//!
//! HTTP sessions and confirm-tokens used to live in per-process memory,
//! which made the CLI's claims about them lies: `confirm-token issue`
//! minted a token that died with the CLI process, and there was no way
//! to revoke a running server's sessions from anywhere else. Persisting
//! the state under `DATA_DIR/run/` gives every transport the same view:
//!
//! * Tokens are stored as SHA-256 hashes — the file never contains a
//!   usable credential.
//! * Writes go through `write_owner_only_atomic` (0600, atomic).
//! * Readers revalidate against the file's `(mtime, len)` before every
//!   answer, so a revoke written by one process is visible to another
//!   on its next request (a local `stat` per check — negligible).
//! * Expired entries are pruned on every load/persist.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::CoreError;

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedEntries {
    /// token-hash (hex) → expiry as unix millis.
    entries: HashMap<String, i64>,
}

struct CachedState {
    entries: HashMap<String, i64>,
    /// `(mtime, len)` of the file the cache was loaded from.
    file_sig: Option<(SystemTime, u64)>,
}

/// One named, TTL'd token set (e.g. `sessions`, `confirm_tokens`).
pub struct TokenFileStore {
    path: PathBuf,
    ttl: Duration,
    state: Mutex<CachedState>,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn hash_token(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            hasher.update([0u8]);
        }
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}

impl TokenFileStore {
    /// `name` becomes `DATA_DIR/run/<name>.json`.
    pub fn new(data_dir: &Path, name: &str, ttl: Duration) -> Self {
        Self {
            path: data_dir.join("run").join(format!("{name}.json")),
            ttl,
            state: Mutex::new(CachedState {
                entries: HashMap::new(),
                file_sig: None,
            }),
        }
    }

    fn file_sig(&self) -> Option<(SystemTime, u64)> {
        std::fs::metadata(&self.path)
            .ok()
            .and_then(|m| m.modified().ok().map(|t| (t, m.len())))
    }

    /// Reload from disk when another process (or this one) changed the
    /// file since the cache was taken.
    fn refresh(&self, state: &mut CachedState) -> Result<(), CoreError> {
        let sig = self.file_sig();
        if sig == state.file_sig {
            return Ok(());
        }
        let entries = if self.path.exists() {
            let text = std::fs::read_to_string(&self.path).map_err(|e| CoreError::Internal {
                context: format!("read {}: {e}", self.path.display()),
            })?;
            serde_json::from_str::<PersistedEntries>(&text)
                .map_err(|e| CoreError::Internal {
                    context: format!("parse {}: {e}", self.path.display()),
                })?
                .entries
        } else {
            HashMap::new()
        };
        state.entries = entries;
        state.file_sig = sig;
        Ok(())
    }

    fn persist(&self, state: &mut CachedState) -> Result<(), CoreError> {
        let cutoff = now_ms();
        state.entries.retain(|_, expires| *expires > cutoff);
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::Internal {
                context: format!("create {}: {e}", parent.display()),
            })?;
        }
        let json = serde_json::to_string_pretty(&PersistedEntries {
            entries: state.entries.clone(),
        })
        .map_err(|e| CoreError::Internal {
            context: format!("serialize token store: {e}"),
        })?;
        crate::services::write_owner_only_atomic(&self.path, json.as_bytes())?;
        state.file_sig = self.file_sig();
        Ok(())
    }

    /// Insert a token (hashed) with this store's TTL. `scope` parts are
    /// folded into the hash so e.g. confirm tokens bind to their intent.
    pub fn insert(&self, scope_and_token: &[&str]) -> Result<(), CoreError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.refresh(&mut state)?;
        state.entries.insert(
            hash_token(scope_and_token),
            now_ms() + self.ttl.as_millis() as i64,
        );
        self.persist(&mut state)
    }

    /// Non-consuming membership check.
    pub fn contains(&self, scope_and_token: &[&str]) -> Result<bool, CoreError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.refresh(&mut state)?;
        let hash = hash_token(scope_and_token);
        Ok(state
            .entries
            .get(&hash)
            .is_some_and(|expires| *expires > now_ms()))
    }

    /// One-shot consume: returns true (and removes the entry) when the
    /// token is present and unexpired.
    pub fn consume(&self, scope_and_token: &[&str]) -> Result<bool, CoreError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.refresh(&mut state)?;
        let hash = hash_token(scope_and_token);
        let valid = state
            .entries
            .remove(&hash)
            .is_some_and(|expires| expires > now_ms());
        self.persist(&mut state)?;
        Ok(valid)
    }

    /// Remove one token (idempotent).
    pub fn remove(&self, scope_and_token: &[&str]) -> Result<(), CoreError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.refresh(&mut state)?;
        state.entries.remove(&hash_token(scope_and_token));
        self.persist(&mut state)
    }

    /// Drop every entry; returns how many were live.
    pub fn clear(&self) -> Result<usize, CoreError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.refresh(&mut state)?;
        let cutoff = now_ms();
        let live = state
            .entries
            .values()
            .filter(|expires| **expires > cutoff)
            .count();
        state.entries.clear();
        self.persist(&mut state)?;
        Ok(live)
    }

    /// Count of unexpired entries.
    pub fn live_count(&self) -> Result<usize, CoreError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.refresh(&mut state)?;
        let cutoff = now_ms();
        Ok(state
            .entries
            .values()
            .filter(|expires| **expires > cutoff)
            .count())
    }
}

/// Cross-process HTTP session set. Cookie values are stored hashed with
/// a 7-day TTL; logout removes one, revoke-all clears the set — and a
/// revoke issued by the CLI is honored by a running server on its next
/// request (the store revalidates against the file before answering).
pub struct SessionStore {
    store: TokenFileStore,
}

impl SessionStore {
    pub const TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

    pub fn new(data_dir: &Path) -> Self {
        Self {
            store: TokenFileStore::new(data_dir, "sessions", Self::TTL),
        }
    }

    pub fn insert(&self, session_id: &str) -> Result<(), CoreError> {
        self.store.insert(&[session_id])
    }

    pub fn is_valid(&self, session_id: &str) -> bool {
        // An unreadable store must fail CLOSED (no session accepted),
        // loudly.
        match self.store.contains(&[session_id]) {
            Ok(valid) => valid,
            Err(e) => {
                log::error!("session store unreadable — rejecting session: {e}");
                false
            }
        }
    }

    pub fn remove(&self, session_id: &str) -> Result<(), CoreError> {
        self.store.remove(&[session_id])
    }

    pub fn revoke_all(&self) -> Result<usize, CoreError> {
        self.store.clear()
    }

    pub fn active_count(&self) -> Result<usize, CoreError> {
        self.store.live_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn insert_contains_consume_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = TokenFileStore::new(dir.path(), "t", Duration::from_secs(30));
        store.insert(&["intent", "tok"]).unwrap();
        assert!(store.contains(&["intent", "tok"]).unwrap());
        // Wrong scope → no match (intent binding).
        assert!(!store.contains(&["other", "tok"]).unwrap());
        assert!(store.consume(&["intent", "tok"]).unwrap());
        // One-shot.
        assert!(!store.consume(&["intent", "tok"]).unwrap());
    }

    #[test]
    fn expired_entries_do_not_validate() {
        let dir = TempDir::new().unwrap();
        let store = TokenFileStore::new(dir.path(), "t", Duration::from_millis(10));
        store.insert(&["tok"]).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        assert!(!store.contains(&["tok"]).unwrap());
        assert!(!store.consume(&["tok"]).unwrap());
    }

    /// THE cross-process property: a second store instance (≈ another
    /// process) sees tokens the first wrote, and a revoke by one is
    /// honored by the other.
    #[test]
    fn separate_instances_share_state_via_the_file() {
        let dir = TempDir::new().unwrap();
        let a = SessionStore::new(dir.path());
        let b = SessionStore::new(dir.path());
        a.insert("session-1").unwrap();
        assert!(b.is_valid("session-1"));
        assert_eq!(b.revoke_all().unwrap(), 1);
        assert!(!a.is_valid("session-1"));
    }

    #[test]
    fn file_never_contains_the_raw_token() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path());
        store.insert("super-secret-session-value").unwrap();
        let on_disk =
            std::fs::read_to_string(dir.path().join("run").join("sessions.json")).unwrap();
        assert!(!on_disk.contains("super-secret-session-value"));
    }
}
