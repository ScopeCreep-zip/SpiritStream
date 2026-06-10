//! Out-of-band tail anchor for the audit chain.
//!
//! A hash chain alone cannot detect TAIL TRUNCATION: deleting the last
//! N lines (or the whole file) leaves a perfectly valid prefix, and the
//! pre-fix startup scan happily resumed from whatever tail remained —
//! the evidence (say, a `PanicTriggered` entry) vanished with zero
//! tamper signal. The anchor persists the latest `(seq, hmac)` pair in
//! the [`SecretStore`] (OS keyring or the encrypted file store), which
//! an attacker editing `audit.log` cannot reach with the same access.
//!
//! Honest limits, documented on purpose:
//! - The anchor is written asynchronously after each append, so a crash
//!   can leave it one-or-few entries behind the log. Verification
//!   therefore only treats `log tail < anchor` as truncation; entries
//!   PAST the anchor verify by chain alone.
//! - Detection is reliable at the next startup / verify after the
//!   truncation. The durable evidence is the `AuditLogTamperDetected`
//!   entry the transports record when they observe the breach.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::traits::SecretStore;

const ANCHOR_NAMESPACE: &str = "audit";
const ANCHOR_KEY: &str = "chain-anchor";

/// Persisted anchor payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct AnchorRecord {
    pub(super) seq: u64,
    pub(super) hmac: String,
}

enum AnchorCommand {
    Persist(AnchorRecord),
    Clear,
}

/// Handle to the dedicated anchor-writer thread.
///
/// The `SecretStore` trait is async while `AuditLogService::record` is
/// sync (and must work with or without an ambient tokio runtime —
/// one-shot CLI, unit tests, HTTP server alike). A dedicated OS thread
/// owning its own current-thread runtime sidesteps every
/// nested-runtime hazard; commands are coalesced so a burst of appends
/// costs one store write.
pub(super) struct AnchorHandle {
    tx: std::sync::mpsc::Sender<AnchorCommand>,
    degraded: Arc<AtomicBool>,
    current: Mutex<Option<AnchorRecord>>,
}

impl AnchorHandle {
    /// Spawn the writer thread and synchronously load the persisted
    /// anchor (the load happens on the writer thread; construction
    /// blocks on the reply, never on a nested runtime).
    pub(super) fn start(secrets: Arc<dyn SecretStore>) -> (Self, Option<AnchorRecord>) {
        let (tx, rx) = std::sync::mpsc::channel::<AnchorCommand>();
        let (load_tx, load_rx) = std::sync::mpsc::channel::<Option<AnchorRecord>>();
        let degraded = Arc::new(AtomicBool::new(false));
        let degraded_thread = degraded.clone();

        std::thread::Builder::new()
            .name("audit-anchor".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        log::error!("audit anchor: runtime build failed: {e}");
                        degraded_thread.store(true, Ordering::SeqCst);
                        let _ = load_tx.send(None);
                        return;
                    }
                };

                let loaded = match rt.block_on(secrets.get(ANCHOR_NAMESPACE, ANCHOR_KEY)) {
                    Ok(Some(bytes)) => match serde_json::from_slice::<AnchorRecord>(&bytes) {
                        Ok(record) => Some(record),
                        Err(e) => {
                            log::error!("audit anchor: stored anchor unparseable: {e}");
                            degraded_thread.store(true, Ordering::SeqCst);
                            None
                        }
                    },
                    Ok(None) => None,
                    Err(e) => {
                        log::error!("audit anchor: load failed: {e}");
                        degraded_thread.store(true, Ordering::SeqCst);
                        None
                    }
                };
                let _ = load_tx.send(loaded);

                while let Ok(mut cmd) = rx.recv() {
                    // Coalesce a burst of appends into one store write.
                    while let Ok(next) = rx.try_recv() {
                        cmd = next;
                    }
                    let result = match &cmd {
                        AnchorCommand::Persist(record) => match serde_json::to_vec(record) {
                            Ok(bytes) => {
                                rt.block_on(secrets.put(ANCHOR_NAMESPACE, ANCHOR_KEY, &bytes))
                            }
                            Err(e) => {
                                log::error!("audit anchor: serialize failed: {e}");
                                continue;
                            }
                        },
                        AnchorCommand::Clear => {
                            rt.block_on(secrets.delete(ANCHOR_NAMESPACE, ANCHOR_KEY))
                        }
                    };
                    match result {
                        Ok(()) => degraded_thread.store(false, Ordering::SeqCst),
                        Err(e) => {
                            // Loud, surfaced via `anchor_state()` in the
                            // audit status — but never blocks recording:
                            // the entry itself IS the safety record.
                            log::error!("audit anchor: persist failed: {e}");
                            degraded_thread.store(true, Ordering::SeqCst);
                        }
                    }
                }
            })
            .expect("spawn audit-anchor thread");

        // The writer thread always sends exactly one load reply.
        let loaded = load_rx.recv().unwrap_or_else(|_| {
            degraded.store(true, Ordering::SeqCst);
            None
        });

        let handle = Self {
            tx,
            degraded,
            current: Mutex::new(loaded.clone()),
        };
        (handle, loaded)
    }

    /// Advance the anchor after a successful append.
    pub(super) fn update(&self, seq: u64, hmac: String) {
        let record = AnchorRecord { seq, hmac };
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = Some(record.clone());
        if self.tx.send(AnchorCommand::Persist(record)).is_err() {
            log::error!("audit anchor: writer thread is gone");
            self.degraded.store(true, Ordering::SeqCst);
        }
    }

    /// Drop the anchor — used when the chain is deliberately reset
    /// (migration archive, quarantine), so the fresh chain doesn't
    /// trip a false truncation alarm.
    pub(super) fn clear(&self) {
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = None;
        if self.tx.send(AnchorCommand::Clear).is_err() {
            log::error!("audit anchor: writer thread is gone");
            self.degraded.store(true, Ordering::SeqCst);
        }
    }

    pub(super) fn current(&self) -> Option<AnchorRecord> {
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Stable state string for the audit-status DTO.
    pub(super) fn state_str(&self) -> &'static str {
        if self.degraded.load(Ordering::SeqCst) {
            "degraded"
        } else {
            "ok"
        }
    }
}
