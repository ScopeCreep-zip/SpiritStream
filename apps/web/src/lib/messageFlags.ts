// Mirror of the Rust bitflags declared on `MessageFlags` in
// `crates/core/src/models/chat.rs`. The wire shape is a single `u64`
// integer (see the `Serialize`/`Deserialize` impl in that file), so
// frontend reads `ChatMessage.flags` as a number and bit-checks via
// `hasFlag(flags, MessageFlag.FIRST_MESSAGE)`. The constants below
// must stay byte-for-byte in sync with the Rust enum.
//
// Test pin: `crates/core/src/models/chat.rs::fragment_wire_shape::
// message_flags_serialize_as_integer_bits` asserts the wire format.

export const MessageFlag = {
  HIGHLIGHTED: 1 << 0,
  FIRST_MESSAGE: 1 << 1,
  ELEVATED_MESSAGE: 1 << 2,
  CHEER_MESSAGE: 1 << 3,
  REPLY_MESSAGE: 1 << 4,
  ACTION: 1 << 5,
  SYSTEM: 1 << 6,
  SUBSCRIPTION: 1 << 7,
  ANNOUNCEMENT: 1 << 8,
  WHISPER: 1 << 9,
  DISABLED: 1 << 10,
  TIMED_OUT_AUTHOR: 1 << 11,
  AUTOMOD_HELD: 1 << 12,
  RESTRICTED_AUTHOR: 1 << 13,
  MONITORED_AUTHOR: 1 << 14,
  SHARED_FROM_OTHER_CHANNEL: 1 << 15,
  REDEEMED_CHANNEL_POINT_REWARD: 1 << 16,
  // Author is the local user's own account (native-platform message). Rendered
  // as "you", like app-sent outbound messages.
  SELF_AUTHOR: 1 << 17,
} as const;

export type MessageFlagName = keyof typeof MessageFlag;

/**
 * Bitwise check against the `ChatMessage.flags` bitmask. Returns
 * true if every bit in `mask` is set.
 */
export function hasFlag(flags: number, mask: number): boolean {
  return (flags & mask) === mask;
}
