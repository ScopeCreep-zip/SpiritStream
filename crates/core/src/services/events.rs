use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: Value);
}

pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: &str, _payload: Value) {}
}

/// Blanket impl: an `Arc<dyn EventSink>` is itself an `EventSink`. Lets
/// transports that hold the canonical `Arc<dyn EventSink>` on their
/// `ServiceRegistry` pass it to service methods that take generic
/// `E: EventSink` without having to wrap it manually.
impl EventSink for Arc<dyn EventSink> {
    fn emit(&self, event: &str, payload: Value) {
        (**self).emit(event, payload);
    }
}

pub fn emit_event<T: Serialize>(sink: &dyn EventSink, event: &str, payload: &T) {
    if let Ok(value) = serde_json::to_value(payload) {
        sink.emit(event, value);
    }
}
