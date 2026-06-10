//! Filesystem watcher — emits `themes_updated` events when the user
//! edits files in the themes directory. Skips self-fires from
//! `sync_project_themes` (tracked via `recently_synced`) and debounces
//! at most one emission per second.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use crate::services::{emit_event, EventSink};

use super::{ThemeManager, WATCHER_SELF_FIRE_GRACE};

impl ThemeManager {
    pub fn start_watcher(&self, event_sink: Arc<dyn EventSink>) {
        let manager = self.clone();
        let themes_dir = self.themes_dir.clone();
        let event_sink = Arc::clone(&event_sink);
        thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            let mut watcher = match notify::recommended_watcher(tx) {
                Ok(watcher) => watcher,
                Err(error) => {
                    log::warn!("Theme watcher failed to start: {error}");
                    return;
                }
            };

            if let Err(error) = watcher.watch(&themes_dir, RecursiveMode::NonRecursive) {
                log::warn!("Failed to watch themes directory: {error}");
                return;
            }

            let mut last_update = std::time::Instant::now();
            for event in rx {
                let event = match event {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                // Suppress events whose paths were just written by
                // `sync_project_themes`. The OS delivers an inotify /
                // FSEvent for self-writes too — without this filter,
                // every boot triggers a spurious `themes_updated` push
                // to the WebSocket that the React side then re-renders
                // for no reason.
                let now = std::time::Instant::now();
                let from_self = if let Ok(mut map) = manager.recently_synced.lock() {
                    // Expire entries past the grace window first so the
                    // map doesn't grow without bound.
                    map.retain(|_, t| now.duration_since(*t) < WATCHER_SELF_FIRE_GRACE);
                    !event.paths.is_empty()
                        && event.paths.iter().all(|p| {
                            map.get(p)
                                .is_some_and(|t| now.duration_since(*t) < WATCHER_SELF_FIRE_GRACE)
                        })
                } else {
                    false
                };
                if from_self {
                    continue;
                }

                // Debounce: only emit theme updates at most once per second
                if now.duration_since(last_update) < Duration::from_secs(1) {
                    continue;
                }
                last_update = now;

                let themes = manager.list_themes();
                emit_event(event_sink.as_ref(), "themes_updated", &themes);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Condvar, Mutex};
    use std::time::Instant;
    use tempfile::TempDir;

    /// Counts `themes_updated` emissions and lets a test block until the
    /// first one arrives. The Condvar wakes the waiter the instant the
    /// watcher thread emits, so the test returns promptly once the OS
    /// delivers the FS event and still fails loud if it never does.
    struct SignalSink {
        count: Mutex<usize>,
        cv: Condvar,
    }

    impl EventSink for SignalSink {
        fn emit(&self, event: &str, _payload: serde_json::Value) {
            if event == "themes_updated" {
                let mut c = self.count.lock().unwrap();
                *c += 1;
                self.cv.notify_all();
            }
        }
    }

    fn manager(app: &TempDir, proj: &TempDir) -> ThemeManager {
        // ThemeManager::new already creates themes_dir; this guards the
        // invariant the watcher relies on (the dir must exist before the
        // notify watch call, or the thread bails early).
        let mgr = ThemeManager::new(app.path().to_path_buf(), proj.path().to_path_buf());
        std::fs::create_dir_all(&mgr.themes_dir).unwrap();
        mgr
    }

    #[test]
    fn watcher_emits_on_external_theme_edit() {
        let app = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        let mgr = manager(&app, &proj);

        let sink = Arc::new(SignalSink {
            count: Mutex::new(0),
            cv: Condvar::new(),
        });
        mgr.start_watcher(Arc::clone(&sink) as Arc<dyn EventSink>);

        // Re-write the theme file on a cadence rather than once. macOS
        // FSEvents has a startup latency before a fresh watch begins
        // reporting (observed up to ~2s warm, longer under CI load); a
        // single write right after `start_watcher` races that warmup and
        // is silently dropped. Linux inotify has no such warmup and
        // catches the first write. Each rewrite carries unique bytes so
        // it always produces a fresh event, and the 1.1s cadence clears
        // the watcher's 1s debounce so a landed write always emits. The
        // Condvar wait returns the instant an emission lands, so this is
        // fast in the common case and only burns the full budget on a
        // genuine failure.
        let edited = mgr.themes_dir.join("user-edit.jsonc");
        let deadline = Duration::from_secs(15);
        let started = Instant::now();
        let mut guard = sink.count.lock().unwrap();
        let mut iteration = 0u32;
        while *guard == 0 && started.elapsed() < deadline {
            drop(guard);
            std::fs::write(&edited, format!(r#"{{ "id": "user-edit-{iteration}" }}"#)).unwrap();
            iteration += 1;
            guard = sink.count.lock().unwrap();
            let (next, _timeout) = sink
                .cv
                .wait_timeout_while(guard, Duration::from_millis(1100), |c| *c == 0)
                .unwrap();
            guard = next;
        }

        assert!(
            *guard >= 1,
            "watcher emitted no themes_updated within {deadline:?} of repeated external edits (waited {:?})",
            started.elapsed(),
        );
    }
}
