use serde::Serialize;
use serde_json::Value;
use tauri::Emitter;

pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: Value);
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

pub struct TauriEventSink<R: tauri::Runtime> {
    app_handle: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> TauriEventSink<R> {
    pub fn new(app_handle: tauri::AppHandle<R>) -> Self {
        Self { app_handle }
    }
}

impl<R: tauri::Runtime> EventSink for TauriEventSink<R> {
    fn emit(&self, event: &str, payload: Value) {
        let _ = self.app_handle.emit(event, payload);
    }
}
