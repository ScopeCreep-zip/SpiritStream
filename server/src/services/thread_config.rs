// Thread Configuration Service
// Provides prioritized thread spawning for capture and encoding pipelines.
// Uses thread-priority crate for OS-level priority hints.
// Falls back gracefully to normal priority if OS rejects the request.

use thread_priority::{set_current_thread_priority, ThreadPriority};

/// Classification of threads in the capture/encoding pipeline.
/// Each kind maps to an appropriate OS thread priority.
pub enum CaptureThreadKind {
    /// Frame acquisition from hardware (highest priority)
    VideoCapture,
    /// Real-time audio input
    AudioCapture,
    /// FFmpeg stdin writer / encoding loop (latency-critical)
    Encoding,
    /// FFmpeg stderr reader (logging only)
    StderrReader,
    /// Health checks, lifecycle monitors
    Monitor,
}

impl CaptureThreadKind {
    /// Spawn a named thread with appropriate priority.
    /// Falls back to normal priority if the OS rejects the request.
    pub fn spawn<F, T>(self, name: &str, f: F) -> std::thread::JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let priority = self.priority();
        let thread_name = name.to_string();
        std::thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                if let Err(e) = set_current_thread_priority(priority) {
                    log::debug!("Thread '{}': priority not set: {:?}", thread_name, e);
                }
                f()
            })
            .expect("thread spawn failed")
    }

    fn priority(&self) -> ThreadPriority {
        match self {
            Self::VideoCapture | Self::AudioCapture => ThreadPriority::Max,
            Self::Encoding => ThreadPriority::Max,
            Self::StderrReader | Self::Monitor => ThreadPriority::Min,
        }
    }

    /// Human-readable label for logging
    pub fn label(&self) -> &'static str {
        match self {
            Self::VideoCapture => "video-capture",
            Self::AudioCapture => "audio-capture",
            Self::Encoding => "encoding",
            Self::StderrReader => "stderr-reader",
            Self::Monitor => "monitor",
        }
    }
}
