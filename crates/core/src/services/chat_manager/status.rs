use std::sync::atomic::Ordering;

use log::{error, info, warn};
use serde_json::json;

use crate::models::{ChatConnectionStatus, ChatMessageDirection};

use super::log_writer::ChatLogCommand;

impl super::ChatManager {
    /// Start the message handler that forwards messages to the frontend.
    pub(super) fn start_message_handler(&self) {
        let event_sink = self.event_sink.clone();
        let message_rx = self.message_rx.clone();
        let log_tx = self.log_tx.clone();
        let platforms = self.platforms.clone();
        let crosspost_enabled = self.crosspost_enabled.clone();
        let send_enabled = self.send_enabled.clone();

        tokio::spawn(async move {
            use std::collections::{HashSet, VecDeque};
            const MAX_SEEN_IDS: usize = 5000;
            let mut seen_ids: HashSet<String> = HashSet::new();
            let mut seen_order: VecDeque<String> = VecDeque::new();

            let rx = {
                let mut guard = message_rx.lock().await;
                guard.take()
            };

            if let Some(mut receiver) = rx {
                info!("Chat message handler started");

                while let Some(message) = receiver.recv().await {
                    if seen_ids.contains(&message.id) {
                        continue;
                    }
                    seen_ids.insert(message.id.clone());
                    seen_order.push_back(message.id.clone());
                    if seen_order.len() > MAX_SEEN_IDS {
                        if let Some(old) = seen_order.pop_front() {
                            seen_ids.remove(&old);
                        }
                    }

                    // Log message to disk (best effort, non-blocking).
                    let _ = log_tx.send(ChatLogCommand::Log(Box::new(message.clone())));

                    // Emit message to frontend via EventSink.
                    if let Ok(payload) = serde_json::to_value(&message) {
                        event_sink.emit("chat_message", payload);
                    } else {
                        error!("Failed to serialize chat message");
                    }

                    // Crosspost inbound messages to other enabled platforms.
                    if message.direction == ChatMessageDirection::Inbound
                        && crosspost_enabled.load(Ordering::Relaxed)
                    {
                        let origin = message.platform;
                        let text = message.message.clone();
                        let targets = {
                            let enabled = send_enabled.lock().await;
                            let connectors = platforms.lock().await;
                            let mut list = Vec::new();

                            for (platform, connector) in connectors.iter() {
                                if *platform == origin {
                                    continue;
                                }
                                if !connector.can_send() {
                                    continue;
                                }
                                if !enabled.get(platform).copied().unwrap_or(false) {
                                    continue;
                                }
                                list.push(*platform);
                            }

                            list
                        };

                        if !targets.is_empty() {
                            let platforms = platforms.clone();
                            tokio::spawn(async move {
                                let mut connectors = platforms.lock().await;
                                for platform in targets {
                                    if let Some(connector) = connectors.get_mut(&platform) {
                                        if let Err(err) = connector.send_message(text.clone()).await
                                        {
                                            warn!(
                                                "Crosspost to {} failed: {}",
                                                platform.as_str(),
                                                err
                                            );
                                        }
                                    }
                                }
                            });
                        }
                    }
                }

                info!("Chat message handler stopped");
            } else {
                error!("Message receiver was already taken");
            }
        });
    }

    /// Monitor platform status transitions and emit connection events.
    pub(super) fn start_status_monitor(&self) {
        let platforms = self.platforms.clone();
        let event_sink = self.event_sink.clone();
        let last_statuses = self.last_statuses.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            loop {
                interval.tick().await;

                let snapshots = {
                    let platforms_guard = platforms.lock().await;
                    platforms_guard
                        .iter()
                        .map(|(platform, connector)| {
                            (*platform, connector.status(), connector.last_error())
                        })
                        .collect::<Vec<_>>()
                };

                let mut last = last_statuses.lock().await;

                for (platform, status, error) in snapshots {
                    let previous = last.get(&platform).copied();
                    if previous != Some(status) {
                        last.insert(platform, status);

                        if status == ChatConnectionStatus::Error {
                            let payload = json!({
                                "platform": platform.as_str(),
                                "error": error.unwrap_or_else(|| "Connection lost".to_string()),
                            });
                            event_sink.emit("chat_connection_lost", payload);
                        }
                        if status == ChatConnectionStatus::Connected
                            && previous == Some(ChatConnectionStatus::Error)
                        {
                            let payload = json!({
                                "platform": platform.as_str(),
                            });
                            event_sink.emit("chat_connection_restored", payload);
                        }
                    }
                }
            }
        });
    }
}
