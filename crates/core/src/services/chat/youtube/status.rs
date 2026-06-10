use crate::models::ChatConnectionStatus;

// H7: `status_to_u8` + `status_from_u8` cross the `AtomicU8` boundary
// the connector uses to share connection state between its background
// poll task and the public `status()` getter. Both directions are
// exhaustive matches keyed on the [`ChatConnectionStatus`] enum, so
// adding a variant fails to compile until every site is updated.
// The previous `from_u8` used a wildcard fall-through that silently
// bucketed any unknown value to `Disconnected`.
pub(super) fn status_to_u8(status: ChatConnectionStatus) -> u8 {
    match status {
        ChatConnectionStatus::Disconnected => 0,
        ChatConnectionStatus::Connecting => 1,
        ChatConnectionStatus::Connected => 2,
        ChatConnectionStatus::Error => 3,
    }
}

pub(super) fn status_from_u8(value: u8) -> ChatConnectionStatus {
    match value {
        0 => ChatConnectionStatus::Disconnected,
        1 => ChatConnectionStatus::Connecting,
        2 => ChatConnectionStatus::Connected,
        3 => ChatConnectionStatus::Error,
        // An out-of-range u8 can only appear via memory corruption or
        // a mismatch between `status_to_u8` writers and `status_from_u8`
        // readers introduced by a future bug. Default to Error so the
        // caller treats the connector as broken rather than silently
        // happy.
        _ => ChatConnectionStatus::Error,
    }
}
