use serde::Serialize;
use serde_json::Value;

pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: Value);

    /// Emit a binary payload over the event bus.
    /// Default implementation is a no-op for sinks that don't support binary.
    fn emit_binary(&self, _tag: &str, _data: bytes::Bytes) {}
}

pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: &str, _payload: Value) {}
}

pub fn emit_event<T: Serialize>(sink: &dyn EventSink, event: &str, payload: &T) {
    if let Ok(value) = serde_json::to_value(payload) {
        sink.emit(event, value);
    }
}
