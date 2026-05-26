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
