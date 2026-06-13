//! Chat history persistence — encrypted, append-structured, at rest.
//!
//! Each message is one **length-prefixed encrypted record** in a per-hour
//! `chatlog_{YYYYMMDD-HH}.enc` file: `[u32-LE len][ciphertext]`, where the
//! ciphertext is `Encryption::encrypt_bytes_with_machine_key_aad` over the
//! JSON `ChatMessage`. The AAD binds each record to its file so records
//! can't be spliced between files. Plaintext stranger chat on disk would
//! be the one gap in an AES-256-GCM-SIV-everywhere app, and inbound chat
//! is sensitive (harassment/doxxing + PII) for this population — so it is
//! encrypted at rest (OWASP ASVS/MASVS) and pseudonymized in anonymous
//! mode (the username is already rewritten before it reaches this writer).
//!
//! Files are owner-only (0600 on Unix) and appended to. A crash mid-write
//! leaves at most a truncated final record, which the reader detects via
//! the length prefix and stops cleanly. Retention is age-bounded
//! elsewhere (`log_manager`), satisfying storage-limitation.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use chrono::Local;
use log::warn;
use tokio::sync::{mpsc, oneshot};

use crate::errors::CoreError;
use crate::models::ChatMessage;
use crate::services::Encryption;

/// Domain-separated AAD for a record, bound to its hour-file so a record
/// can't be moved to another file and still decrypt.
fn record_aad(hour_key: &str) -> Vec<u8> {
    format!("spiritstream/chat-log/v1/{hour_key}").into_bytes()
}

/// `chatlog_{hour}.enc` → `hour` (the AAD-binding key). `None` if the
/// filename doesn't match the expected shape.
fn hour_key_from_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_prefix("chatlog_")?.strip_suffix(".enc")?;
    Some(stem.to_string())
}

/// Internal command queue for the background chat-log writer task.
pub(crate) enum ChatLogCommand {
    StartSession,
    EndSession,
    Log(Box<ChatMessage>),
    Flush(oneshot::Sender<()>),
    /// Panic wipe: close the writer and delete every `chatlog_*.enc`
    /// file. For this population, panic means "make it disappear" — the
    /// on-disk stranger chat (possibly doxxing content) goes too,
    /// alongside the in-memory ring clear and secret wipe.
    Purge,
}

impl super::ChatManager {
    /// Start the chat log writer background task.
    pub(super) fn start_log_writer(
        &self,
        mut log_rx: mpsc::Receiver<ChatLogCommand>,
        log_dir: PathBuf,
        app_data_dir: PathBuf,
    ) {
        let handle = tokio::spawn(async move {
            let _ = std::fs::create_dir_all(&log_dir);
            let mut state = ChatLogState::new(log_dir, app_data_dir);

            while let Some(cmd) = log_rx.recv().await {
                match cmd {
                    ChatLogCommand::StartSession => state.start_session(),
                    ChatLogCommand::EndSession => state.end_session(),
                    ChatLogCommand::Log(message) => state.write_message(&message),
                    ChatLogCommand::Flush(tx) => {
                        state.flush();
                        let _ = tx.send(());
                    }
                    ChatLogCommand::Purge => state.purge(),
                }
            }
        });
        if let Ok(mut slot) = self.log_writer_handle.lock() {
            *slot = Some(handle);
        }
    }

    /// Start a new log session. Sync `pub fn` so callers don't await;
    /// `try_send` stays non-blocking (drop-on-Full = writer wedged).
    /// Now driven by chat-connect / activation, not stream start — chat
    /// is decoupled from streaming.
    pub fn start_log_session(&self) {
        let now = Local::now().timestamp_millis();
        self.log_session_start_ms.store(now, Ordering::Relaxed);
        let _ = self.log_tx.try_send(ChatLogCommand::StartSession);
    }

    /// End the current log session.
    pub fn end_log_session(&self) {
        self.log_session_start_ms.store(0, Ordering::Relaxed);
        let _ = self.log_tx.try_send(ChatLogCommand::EndSession);
    }

    /// Log a message to disk (best effort). Applies the anonymous-mode
    /// pseudonymizer before queueing, so the writer never sees plaintext
    /// usernames when anonymous mode is on; an un-pseudonymizable message
    /// is DROPPED, never written with its real username.
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

    /// Hot-path helper: apply the cached anonymous-mode policy to a
    /// `ChatMessage` if active. The policy lives behind a
    /// `std::sync::RwLock` (readers never block; the only writer is
    /// human-paced profile activation), so this sync path always sees it.
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

    /// Current log session start timestamp (ms), if active.
    pub fn log_session_start_ms(&self) -> Option<i64> {
        let value = self.log_session_start_ms.load(Ordering::Relaxed);
        (value > 0).then_some(value)
    }

    /// Panic wipe of the on-disk chat history (non-blocking; the writer
    /// task owns the files). Pairs with `clear_recent_messages()` for the
    /// in-memory ring.
    pub fn purge_chat_history(&self) {
        let _ = self.log_tx.try_send(ChatLogCommand::Purge);
    }
}

/// Decrypt every record in one `chatlog_*.enc` file, oldest→newest.
/// Used by export/search (per hour-key) and the recent-tail reader.
/// Stops cleanly at EOF or a truncated final record (crash tolerance).
pub fn read_messages_from_file(path: &Path, app_data_dir: &Path) -> Vec<ChatMessage> {
    let Some(hour_key) = hour_key_from_path(path) else {
        return Vec::new();
    };
    let aad = record_aad(&hour_key);
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let mut messages = Vec::new();
    loop {
        let mut len_buf = [0u8; 4];
        if file.read_exact(&mut len_buf).is_err() {
            break; // EOF or truncated length
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        // Guard against a corrupt/huge length (truncated/garbage record).
        if len == 0 || len > 16 * 1024 * 1024 {
            break;
        }
        let mut ct = vec![0u8; len];
        if file.read_exact(&mut ct).is_err() {
            break; // truncated final record
        }
        match Encryption::decrypt_bytes_with_machine_key_aad(&ct, &aad, app_data_dir) {
            Ok(plain) => {
                if let Ok(msg) = serde_json::from_slice::<ChatMessage>(&plain) {
                    messages.push(msg);
                }
            }
            Err(e) => {
                warn!("chat-log record failed to decrypt (skipping): {e}");
            }
        }
    }
    messages
}

/// The last `n` persisted messages across all `.enc` files, oldest→newest.
/// Reads newest files first and stops once it has enough — used to seed
/// the in-memory ring on boot (full-restart replay).
pub fn read_recent(log_dir: &Path, app_data_dir: &Path, n: usize) -> Vec<ChatMessage> {
    if n == 0 {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(log_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| hour_key_from_path(p).is_some())
        .collect();
    // Hour keys sort lexicographically == chronologically.
    files.sort();
    let mut collected: Vec<ChatMessage> = Vec::new();
    for path in files.iter().rev() {
        let mut msgs = read_messages_from_file(path, app_data_dir);
        msgs.append(&mut collected);
        collected = msgs;
        if collected.len() >= n {
            break;
        }
    }
    let drop = collected.len().saturating_sub(n);
    collected.split_off(drop)
}

struct ChatLogState {
    log_dir: PathBuf,
    app_data_dir: PathBuf,
    active: bool,
    current_hour_key: Option<String>,
    writer: Option<BufWriter<File>>,
}

impl ChatLogState {
    fn new(log_dir: PathBuf, app_data_dir: PathBuf) -> Self {
        Self {
            log_dir,
            app_data_dir,
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
                warn!("Failed to rotate chat log file: {e}");
                return;
            }
        }
        let json = match serde_json::to_vec(message) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to serialize chat message for log: {e}");
                return;
            }
        };
        let aad = record_aad(&hour_key);
        let ct = match Encryption::encrypt_bytes_with_machine_key_aad(
            &json,
            &aad,
            &self.app_data_dir,
        ) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to encrypt chat log record: {e}");
                return;
            }
        };
        if let Some(writer) = self.writer.as_mut() {
            let len = ct.len() as u32;
            if writer.write_all(&len.to_le_bytes()).is_err() || writer.write_all(&ct).is_err() {
                warn!("Failed to write chat log record");
            }
        }
    }

    fn rotate_file(&mut self, hour_key: &str) -> Result<(), CoreError> {
        let path = self.log_dir.join(format!("chatlog_{hour_key}.enc"));
        let mut opts = OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600); // owner-only at rest
        }
        let file = opts.open(&path).map_err(|e| CoreError::Internal {
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

    /// Panic wipe: close the active writer and delete every history file.
    fn purge(&mut self) {
        self.writer = None;
        self.current_hour_key = None;
        if let Ok(entries) = std::fs::read_dir(&self.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if hour_key_from_path(&path).is_some() {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{read_messages_from_file, read_recent, ChatLogState};
    use crate::models::{ChatMessage, ChatPlatform};
    use std::path::Path;
    use tempfile::TempDir;

    fn sample(text: &str) -> ChatMessage {
        ChatMessage::new(ChatPlatform::Twitch, "viewer".into(), text.into())
    }

    fn enc_files(dir: &Path) -> Vec<std::path::PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("enc"))
            .collect()
    }

    #[test]
    fn inactive_state_never_opens_a_file() {
        let dir = TempDir::new().unwrap();
        let mut state = ChatLogState::new(dir.path().to_path_buf(), dir.path().to_path_buf());
        state.write_message(&sample("dropped while inactive"));
        assert!(enc_files(dir.path()).is_empty());
    }

    #[test]
    fn records_round_trip_through_encryption() {
        let dir = TempDir::new().unwrap();
        let mut state = ChatLogState::new(dir.path().to_path_buf(), dir.path().to_path_buf());
        state.start_session();
        state.write_message(&sample("hello"));
        state.write_message(&sample("world"));
        state.end_session();

        let files = enc_files(dir.path());
        assert_eq!(files.len(), 1, "one hour-keyed encrypted file");
        // On-disk bytes are ciphertext, NOT plaintext.
        let raw = std::fs::read(&files[0]).unwrap();
        assert!(
            !raw.windows(5).any(|w| w == b"hello"),
            "must be encrypted at rest"
        );

        let msgs = read_messages_from_file(&files[0], dir.path());
        let texts: Vec<&str> = msgs.iter().map(|m| m.message.as_str()).collect();
        assert_eq!(texts, vec!["hello", "world"]);
    }

    #[test]
    fn foreign_key_fails_closed() {
        let dir = TempDir::new().unwrap();
        let mut state = ChatLogState::new(dir.path().to_path_buf(), dir.path().to_path_buf());
        state.start_session();
        state.write_message(&sample("secret"));
        state.end_session();
        let path = enc_files(dir.path()).pop().unwrap();
        // Decrypting under a different machine key (different data dir)
        // yields nothing — records are never readable as plaintext.
        let other = TempDir::new().unwrap();
        let msgs = read_messages_from_file(&path, other.path());
        assert!(msgs.is_empty(), "foreign key must not decrypt records");
    }

    #[test]
    fn read_recent_returns_last_n_in_order() {
        let dir = TempDir::new().unwrap();
        let mut state = ChatLogState::new(dir.path().to_path_buf(), dir.path().to_path_buf());
        state.start_session();
        for i in 0..10 {
            state.write_message(&sample(&format!("m{i}")));
        }
        state.end_session();

        let recent = read_recent(dir.path(), dir.path(), 3);
        let texts: Vec<&str> = recent.iter().map(|m| m.message.as_str()).collect();
        assert_eq!(texts, vec!["m7", "m8", "m9"]);
    }
}
