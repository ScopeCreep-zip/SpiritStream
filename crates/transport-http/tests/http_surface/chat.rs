//! Chat history replay surface: the recent-messages endpoint that
//! repopulates chat after a webview refresh / WS reconnect.

use super::common::{boot, get_with_header};
use serde_json::Value;

#[test]
fn recent_messages_returns_array_with_no_store() {
    let server = boot();
    let (status, cache_control, body) =
        get_with_header(&server, "/api/v1/chat/messages/recent", "cache-control");
    assert_eq!(status, 200, "recent endpoint must answer: {body}");
    // Sensitive chat must not be cached by the webview (OWASP ASVS).
    assert!(
        cache_control.contains("no-store"),
        "recent messages must be Cache-Control: no-store, got '{cache_control}'"
    );
    // Fresh boot with no connected chat → an empty JSON array (the ring
    // seeds from encrypted history, which is empty here).
    let json: Value = serde_json::from_str(&body).unwrap();
    assert!(json.is_array(), "recent messages must be a JSON array: {body}");
}
