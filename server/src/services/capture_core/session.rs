// Capture Session Manager
//
// Generic session lifecycle for capture services. Replaces the duplicated
// HashMap<String, ActiveCapture> + AtomicBool + broadcast pattern found in
// screen_capture, camera_capture, audio_capture, and h264_capture.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use parking_lot::Mutex;
use tokio::sync::broadcast;

/// Metadata about a capture session (for logging and diagnostics)
#[derive(Debug)]
pub struct SessionMetadata {
    pub source_id: String,
    pub source_type: &'static str,
    pub started_at: Instant,
    /// Optional human-readable label (e.g., device name, display name)
    pub label: Option<String>,
}

/// An active capture session with stop signaling and broadcast output.
///
/// Generic over `T` — the frame type each service produces:
/// - `scap::Frame` for screen capture
/// - `VideoFrame` for camera capture
/// - `AudioBuffer` for audio capture
/// - `Bytes` for H264 encoded output
pub struct CaptureSession<T: Send + 'static> {
    pub stop_flag: Arc<AtomicBool>,
    pub tx: broadcast::Sender<Arc<T>>,
    pub metadata: SessionMetadata,
    /// Thread handle(s) — joined on stop for clean shutdown.
    /// Multiple handles supported (e.g., capture thread + stderr reader).
    handles: Vec<JoinHandle<()>>,
    /// Optional cleanup callback invoked after stop_flag is set.
    /// Used for service-specific teardown (e.g., killing FFmpeg child process,
    /// stopping underlying screen capture for H264 sessions).
    on_stop: Option<Box<dyn FnOnce() + Send>>,
}

impl<T: Send + 'static> CaptureSession<T> {
    /// Create a new capture session.
    ///
    /// # Arguments
    /// * `source_id` — unique identifier (e.g., source UUID, device ID)
    /// * `source_type` — static label for logging (e.g., "screen", "camera")
    /// * `channel_capacity` — broadcast channel buffer size
    pub fn new(
        source_id: impl Into<String>,
        source_type: &'static str,
        channel_capacity: usize,
    ) -> (Self, broadcast::Receiver<Arc<T>>) {
        let (tx, rx) = broadcast::channel(channel_capacity);
        let stop_flag = Arc::new(AtomicBool::new(false));

        let session = Self {
            stop_flag,
            tx,
            metadata: SessionMetadata {
                source_id: source_id.into(),
                source_type,
                started_at: Instant::now(),
                label: None,
            },
            handles: Vec::new(),
            on_stop: None,
        };

        (session, rx)
    }

    /// Set a human-readable label for this session
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.metadata.label = Some(label.into());
        self
    }

    /// Add a thread handle to be joined on stop
    pub fn with_handle(mut self, handle: JoinHandle<()>) -> Self {
        self.handles.push(handle);
        self
    }

    /// Add multiple thread handles
    pub fn with_handles(mut self, handles: Vec<JoinHandle<()>>) -> Self {
        self.handles.extend(handles);
        self
    }

    /// Set a cleanup callback invoked after the stop flag is raised
    pub fn with_on_stop(mut self, f: impl FnOnce() + Send + 'static) -> Self {
        self.on_stop = Some(Box::new(f));
        self
    }

    /// Get a clone of the stop flag for the capture thread
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop_flag.clone()
    }

    /// Get a clone of the broadcast sender for the capture thread
    pub fn sender(&self) -> broadcast::Sender<Arc<T>> {
        self.tx.clone()
    }

    /// Signal stop and run cleanup. Does NOT join threads (caller decides).
    fn signal_stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(on_stop) = self.on_stop.take() {
            on_stop();
        }
    }
}

/// Manages active capture sessions with O(1) lookup by ID.
///
/// Provides the common stop/stop_all/is_active/active_count operations
/// that were previously copy-pasted across all 4 capture services.
pub struct CaptureSessionManager<T: Send + 'static> {
    sessions: Mutex<HashMap<String, CaptureSession<T>>>,
    /// Static label for log messages (e.g., "screen", "camera", "audio", "h264")
    service_name: &'static str,
}

impl<T: Send + 'static> CaptureSessionManager<T> {
    pub fn new(service_name: &'static str) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            service_name,
        }
    }

    /// Insert a new session. Returns the previous session if one existed with the same ID
    /// (the previous session is stopped automatically).
    pub fn insert(&self, id: impl Into<String>, session: CaptureSession<T>) -> Option<CaptureSession<T>> {
        let id = id.into();
        let mut sessions = self.sessions.lock();

        let prev = sessions.remove(&id);
        if let Some(mut old) = prev {
            old.signal_stop();
            log::info!("[{}] Replaced existing session: {}", self.service_name, id);
            sessions.insert(id, session);
            return Some(old);
        }

        log::info!("[{}] Started session: {}", self.service_name, id);
        sessions.insert(id, session);
        None
    }

    /// Stop a session by ID. Sets the stop flag, runs cleanup, and joins threads.
    /// Returns Ok(()) if found, Err if not found.
    pub fn stop(&self, id: &str) -> Result<(), String> {
        let handles = {
            let mut sessions = self.sessions.lock();
            if let Some(mut session) = sessions.remove(id) {
                session.signal_stop();
                log::info!("[{}] Stopped session: {}", self.service_name, id);
                session.handles
            } else {
                return Err(format!(
                    "No active {} capture: {}",
                    self.service_name, id
                ));
            }
        };
        // Join threads outside the lock to avoid deadlock
        for handle in handles {
            let _ = handle.join();
        }
        Ok(())
    }

    /// Stop a session by ID without joining threads (non-blocking).
    /// Useful when caller needs to release the session quickly.
    pub fn stop_nonblocking(&self, id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock();
        if let Some(mut session) = sessions.remove(id) {
            session.signal_stop();
            log::info!("[{}] Stopped session (non-blocking): {}", self.service_name, id);
            Ok(())
        } else {
            Err(format!(
                "No active {} capture: {}",
                self.service_name, id
            ))
        }
    }

    /// Stop all active sessions. Sets stop flags and joins all threads.
    pub fn stop_all(&self) {
        let all_handles: Vec<_> = {
            let mut sessions = self.sessions.lock();
            sessions
                .drain()
                .flat_map(|(id, mut session)| {
                    session.signal_stop();
                    log::info!("[{}] Stopped session: {}", self.service_name, id);
                    session.handles
                })
                .collect()
        };
        // Join all threads outside the lock
        for handle in all_handles {
            let _ = handle.join();
        }
    }

    /// Stop all sessions without joining threads (non-blocking).
    pub fn stop_all_nonblocking(&self) {
        let mut sessions = self.sessions.lock();
        for (id, mut session) in sessions.drain() {
            session.signal_stop();
            log::info!("[{}] Stopped session (non-blocking): {}", self.service_name, id);
        }
    }

    /// Check if a session is active
    pub fn is_active(&self, id: &str) -> bool {
        self.sessions.lock().contains_key(id)
    }

    /// Get count of active sessions
    pub fn active_count(&self) -> usize {
        self.sessions.lock().len()
    }

    /// Get list of active session IDs
    pub fn active_ids(&self) -> Vec<String> {
        self.sessions.lock().keys().cloned().collect()
    }

    /// Subscribe to a session's broadcast channel.
    /// Returns None if the session doesn't exist.
    pub fn subscribe(&self, id: &str) -> Option<broadcast::Receiver<Arc<T>>> {
        self.sessions.lock().get(id).map(|s| s.tx.subscribe())
    }

    /// Get the broadcast sender for a session.
    /// Returns None if the session doesn't exist.
    pub fn get_sender(&self, id: &str) -> Option<broadcast::Sender<Arc<T>>> {
        self.sessions.lock().get(id).map(|s| s.tx.clone())
    }

    /// Access session metadata (source_type, started_at, label).
    /// Returns None if session doesn't exist.
    pub fn get_metadata(&self, id: &str) -> Option<(String, &'static str, Instant, Option<String>)> {
        self.sessions.lock().get(id).map(|s| {
            (
                s.metadata.source_id.clone(),
                s.metadata.source_type,
                s.metadata.started_at,
                s.metadata.label.clone(),
            )
        })
    }

    /// Execute a function with read access to a session.
    /// Useful for service-specific queries without exposing internals.
    pub fn with_session<R>(&self, id: &str, f: impl FnOnce(&CaptureSession<T>) -> R) -> Option<R> {
        self.sessions.lock().get(id).map(f)
    }

    /// Execute a function with mutable access to a session.
    pub fn with_session_mut<R>(&self, id: &str, f: impl FnOnce(&mut CaptureSession<T>) -> R) -> Option<R> {
        self.sessions.lock().get_mut(id).map(f)
    }

    /// Iterate over all sessions and collect results.
    /// Useful for building info/status responses.
    pub fn map_sessions<R>(&self, f: impl Fn(&str, &CaptureSession<T>) -> R) -> Vec<R> {
        self.sessions
            .lock()
            .iter()
            .map(|(id, session)| f(id, session))
            .collect()
    }
}

impl<T: Send + 'static> Drop for CaptureSessionManager<T> {
    fn drop(&mut self) {
        // Best-effort cleanup on drop — non-blocking since we're in destructor
        self.stop_all_nonblocking();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle() {
        let manager: CaptureSessionManager<Vec<u8>> = CaptureSessionManager::new("test");

        // Create and insert session
        let (session, _rx) = CaptureSession::new("test-1", "test", 16);
        manager.insert("test-1", session);

        assert!(manager.is_active("test-1"));
        assert_eq!(manager.active_count(), 1);
        assert_eq!(manager.active_ids(), vec!["test-1".to_string()]);

        // Subscribe
        assert!(manager.subscribe("test-1").is_some());
        assert!(manager.subscribe("nonexistent").is_none());

        // Stop
        assert!(manager.stop("test-1").is_ok());
        assert!(!manager.is_active("test-1"));
        assert_eq!(manager.active_count(), 0);

        // Stop nonexistent
        assert!(manager.stop("test-1").is_err());
    }

    #[test]
    fn test_stop_all() {
        let manager: CaptureSessionManager<Vec<u8>> = CaptureSessionManager::new("test");

        let (s1, _) = CaptureSession::new("a", "test", 16);
        let (s2, _) = CaptureSession::new("b", "test", 16);
        let (s3, _) = CaptureSession::new("c", "test", 16);

        manager.insert("a", s1);
        manager.insert("b", s2);
        manager.insert("c", s3);

        assert_eq!(manager.active_count(), 3);

        manager.stop_all();

        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_replace_existing_session() {
        let manager: CaptureSessionManager<Vec<u8>> = CaptureSessionManager::new("test");

        let (s1, _) = CaptureSession::new("id-1", "test", 16);
        let stop_flag_1 = s1.stop_flag.clone();

        manager.insert("id-1", s1);

        // Replace with new session
        let (s2, _) = CaptureSession::new("id-1", "test", 16);
        let old = manager.insert("id-1", s2);

        // Old session should have been stopped
        assert!(old.is_some());
        assert!(stop_flag_1.load(Ordering::Relaxed));
        assert_eq!(manager.active_count(), 1);
    }
}
