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
        mut log_rx: mpsc::UnboundedReceiver<ChatLogCommand>,
        log_dir: PathBuf,
    ) {
        tokio::spawn(async move {
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
    }

    /// Start a new log session (stream start).
    pub fn start_log_session(&self) {
        let now = Local::now().timestamp_millis();
        self.log_session_start_ms.store(now, Ordering::Relaxed);
        let _ = self.log_tx.send(ChatLogCommand::StartSession);
    }

    /// End the current log session (stream end).
    pub fn end_log_session(&self) {
        self.log_session_start_ms.store(0, Ordering::Relaxed);
        let _ = self.log_tx.send(ChatLogCommand::EndSession);
    }

    /// Log a message to disk (best effort). Applies the
    /// anonymous-mode pseudonymizer to the username field before
    /// queueing, so the log writer never sees plaintext usernames
    /// when anonymous mode is on.
    pub fn log_message(&self, message: ChatMessage) {
        let message = self.apply_anonymous_policy_to_message(message);
        let _ = self.log_tx.send(ChatLogCommand::Log(Box::new(message)));
    }

    /// Hot path helper: apply the cached anonymous-mode policy to a
    /// `ChatMessage` if active. Uses a blocking lock (`try_lock`)
    /// because `log_message` is non-async by contract — if the policy
    /// mutex is contended at this exact moment, we let the message
    /// through unchanged rather than blocking. Frequency: contention
    /// only at profile-activate-time, which is human-paced.
    fn apply_anonymous_policy_to_message(&self, mut message: ChatMessage) -> ChatMessage {
        if let Ok(guard) = self.anonymous_policy.try_lock() {
            if let Some((enabled, salt)) = guard.as_ref() {
                if *enabled && !salt.is_empty() {
                    message.username =
                        crate::services::pseudonymizer::pseudonymize(&message.username, salt);
                }
            }
        }
        message
    }

    /// Flush pending log writes to disk.
    pub async fn flush_chat_logs(&self) -> Result<(), CoreError> {
        let (tx, rx) = oneshot::channel();
        self.log_tx
            .send(ChatLogCommand::Flush(tx))
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
