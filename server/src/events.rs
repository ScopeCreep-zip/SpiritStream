use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;

use spiritstream_server::services::EventSink;

#[derive(Clone, Serialize)]
pub(crate) struct ServerEvent {
    pub event: String,
    pub payload: Value,
}

#[derive(Clone)]
pub(crate) struct EventBus {
    pub sender: broadcast::Sender<ServerEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.sender.subscribe()
    }
}

impl EventSink for EventBus {
    fn emit(&self, event: &str, payload: Value) {
        let _ = self.sender.send(ServerEvent {
            event: event.to_string(),
            payload,
        });
    }
}
