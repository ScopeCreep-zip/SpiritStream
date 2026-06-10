use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use chrono::Local;
use log::warn;
use tokio::sync::{mpsc, oneshot};

use crate::errors::CoreError;
use crate::models::ChatMessage;

/// Internal command queue for the background chat-log writer task.
pub(crate) enum ChatLogCommand {
    StartSession,
    EndSession,
    Log(Box<ChatMessage>),
    Flush(oneshot::Sender<()>),
}

impl super::ChatManager {
    /// Start the chat log writer background task.
    pub(super) fn start_log_writer(
        &self,
        mut log_rx: mpsc::Receiver<ChatLogCommand>,
        log_dir: PathBuf,
    ) {
        let handle = tokio::spawn(async move {
            let _ = std::fs::create_dir_all(&log_dir);
            let mut state = ChatLogState::new(log_dir);

            while let Some(cmd) = log_rx.recv().await {
                match cmd {
                    ChatLogCommand::StartSession => {
                        state.start_session();
                    }
                    ChatLogCommand::EndSession => {
                        state.end_session();
                    }
                    ChatLogCommand::Log(message) => {
                        state.write_message(&message);
                    }
                    ChatLogCommand::Flush(tx) => {
                        state.flush();
                        let _ = tx.send(());
                    }
                }
            }
        });
        if let Ok(mut slot) = self.log_writer_handle.lock() {
            *slot = Some(handle);
        }
    }

    /// Start a new log session (stream start). Sync `pub fn` API so
    /// callers don't need to await; uses `try_send` to stay non-blocking.
    /// Drops on Full (5000-cmd capacity = ~166s of writes at 30 cmd/s,
    /// so Full means the writer is wedged and one more dropped command
    /// is the least of the problems).
    pub fn start_log_session(&self) {
        let now = Local::now().timestamp_millis();
        self.log_session_start_ms.store(now, Ordering::Relaxed);
        let _ = self.log_tx.try_send(ChatLogCommand::StartSession);
    }

    /// End the current log session (stream end).
    pub fn end_log_session(&self) {
        self.log_session_start_ms.store(0, Ordering::Relaxed);
        let _ = self.log_tx.try_send(ChatLogCommand::EndSession);
    }

    /// Log a message to disk (best effort). Applies the
    /// anonymous-mode pseudonymizer to the username field before
    /// queueing, so the log writer never sees plaintext usernames
    /// when anonymous mode is on. A message that cannot be
    /// pseudonymised is DROPPED from the log — never written with its
    /// real username.
    pub fn log_message(&self, message: ChatMessage) {
        match self.apply_anonymous_policy_to_message(message) {
            Ok(message) => {
                let _ = self.log_tx.try_send(ChatLogCommand::Log(Box::new(message)));
            }
            Err(e) => {
                log::error!(
                    "anonymous mode active but pseudonymization failed; \
                     dropping chat-log entry: {e}"
                );
                self.event_sink.emit(
                    "anonymous_mode_error",
                    serde_json::json!({ "reason": "salt_invalid", "dropped": true }),
                );
            }
        }
    }

    /// Hot path helper: apply the cached anonymous-mode policy to a
    /// `ChatMessage` if active. The policy lives behind a
    /// `std::sync::RwLock` (readers never block each other; the only
    /// writer is human-paced profile activation), so this sync path
    /// always sees the policy — the previous async-mutex `try_lock`
    /// fallback could write a PLAINTEXT username on contention.
    fn apply_anonymous_policy_to_message(
        &self,
        mut message: ChatMessage,
    ) -> Result<ChatMessage, CoreError> {
        let guard = self
            .anonymous_policy
            .read()
            .unwrap_or_else(|e| e.into_inner());
        if let Some((enabled, salt)) = guard.as_ref() {
            if *enabled {
                message.username =
                    crate::services::pseudonymizer::pseudonymize(&message.username, salt)?;
            }
        }
        Ok(message)
    }

    /// Flush pending log writes to disk.
    pub async fn flush_chat_logs(&self) -> Result<(), CoreError> {
        let (tx, rx) = oneshot::channel();
        self.log_tx
            .send(ChatLogCommand::Flush(tx))
            .await
            .map_err(|_| CoreError::Internal {
                context: "Chat log writer is not available".into(),
            })?;
        rx.await.map_err(|_| CoreError::Internal {
            context: "Failed to flush chat logs".into(),
        })
    }

    /// Return the current log session start timestamp (ms), if active.
    pub fn log_session_start_ms(&self) -> Option<i64> {
        let value = self.log_session_start_ms.load(Ordering::Relaxed);
        if value > 0 {
            Some(value)
        } else {
            None
        }
    }
}

struct ChatLogState {
    log_dir: PathBuf,
    active: bool,
    current_hour_key: Option<String>,
    writer: Option<BufWriter<File>>,
}

impl ChatLogState {
    fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            active: false,
            current_hour_key: None,
            writer: None,
        }
    }

    fn start_session(&mut self) {
        self.active = true;
        self.current_hour_key = None;
        self.writer = None;
    }

    fn end_session(&mut self) {
        self.active = false;
        self.flush();
        self.writer = None;
        self.current_hour_key = None;
    }

    fn write_message(&mut self, message: &ChatMessage) {
        if !self.active {
            return;
        }

        let hour_key = Local::now().format("%Y%m%d-%H").to_string();
        if self.current_hour_key.as_deref() != Some(&hour_key) {
            if let Err(e) = self.rotate_file(&hour_key) {
                warn!("Failed to rotate chat log file: {}", e);
                return;
            }
        }

        if let Some(writer) = self.writer.as_mut() {
            if let Ok(line) = serde_json::to_string(message) {
                if let Err(e) = writer.write_all(line.as_bytes()) {
                    warn!("Failed to write chat log line: {}", e);
                    return;
                }
                let _ = writer.write_all(b"\n");
            }
        }
    }

    fn rotate_file(&mut self, hour_key: &str) -> Result<(), CoreError> {
        let path = self.log_dir.join(format!("chatlog_{}.jsonl", hour_key));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| CoreError::Internal {
                context: format!("Failed to open chat log file: {e}"),
            })?;

        self.writer = Some(BufWriter::new(file));
        self.current_hour_key = Some(hour_key.to_string());
        Ok(())
    }

    fn flush(&mut self) {
        if let Some(writer) = self.writer.as_mut() {
            let _ = writer.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChatLogState;
    use crate::models::{ChatMessage, ChatPlatform};
    use std::fs;
    use tempfile::TempDir;

    fn sample(text: &str) -> ChatMessage {
        ChatMessage::new(ChatPlatform::Twitch, "viewer".into(), text.into())
    }

    fn jsonl_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("jsonl"))
            .collect()
    }

    #[test]
    fn inactive_state_never_opens_a_file() {
        let dir = TempDir::new().unwrap();
        let mut state = ChatLogState::new(dir.path().to_path_buf());
        state.write_message(&sample("dropped while inactive"));
        assert!(jsonl_files(dir.path()).is_empty());
    }

    #[test]
    fn active_session_writes_one_jsonl_line_per_message() {
        let dir = TempDir::new().unwrap();
        let mut state = ChatLogState::new(dir.path().to_path_buf());
        state.start_session();
        state.write_message(&sample("hello"));
        state.write_message(&sample("world"));
        state.end_session();

        let files = jsonl_files(dir.path());
        assert_eq!(files.len(), 1, "one hour-keyed file");
        let body = fs::read_to_string(&files[0]).unwrap();
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(body.contains("hello"));
        assert!(body.contains("world"));
    }

    #[test]
    fn messages_after_end_session_are_dropped() {
        let dir = TempDir::new().unwrap();
        let mut state = ChatLogState::new(dir.path().to_path_buf());
        state.start_session();
        state.write_message(&sample("kept"));
        state.end_session();
        state.write_message(&sample("after-end"));

        let files = jsonl_files(dir.path());
        let body = fs::read_to_string(&files[0]).unwrap();
        assert!(body.contains("kept"));
        assert!(!body.contains("after-end"));
    }
}
