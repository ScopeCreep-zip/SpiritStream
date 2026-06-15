/**
 * Runtime validation for untrusted inbound chat messages.
 *
 * The generated OpenAPI types are compile-time only — they can't stop a
 * malformed or hostile payload (wrong types, missing fields, injected
 * shapes) from reaching the renderer at runtime. Chat messages are the
 * highest-risk inbound surface for this app's threat model: they originate
 * from arbitrary third parties on the streaming platforms.
 *
 * This guards the fields the UI actually renders as scalars (so a non-string
 * can never reach the message renderer) while letting the structured/nested
 * fields (`author`, `fragments`, `event`, …) flow through — they have their
 * own typed consumers downstream. A single malformed message is dropped and
 * logged rather than failing the whole replay batch.
 */
import { z } from 'zod';
import type { ChatMessage } from '@spiritstream/types';

const chatMessageGuard = z.object({
  id: z.string(),
  platform: z.string(),
  username: z.string(),
  message: z.string(),
  timestamp: z.number(),
  flags: z.number(),
  direction: z.string(),
});

/**
 * Validate a single inbound chat message (the live WebSocket push path).
 * Returns the message on success, or `null` if it fails the scalar-field
 * guard — the caller drops it rather than rendering an unvalidated payload.
 */
export function parseChatMessage(data: unknown): ChatMessage | null {
  if (chatMessageGuard.safeParse(data).success) {
    return data as ChatMessage;
  }
  // eslint-disable-next-line no-console
  console.warn('[api-client] dropped malformed inbound chat message');
  return null;
}

/**
 * Validate an inbound chat-message array. Non-array payloads yield `[]`;
 * individual messages that fail the scalar-field guard are dropped (logged),
 * never thrown — one bad message must not blank the whole chat view.
 */
export function parseChatMessages(data: unknown): ChatMessage[] {
  if (!Array.isArray(data)) {
    if (data != null) {
      // eslint-disable-next-line no-console
      console.warn('[api-client] chat messages payload was not an array; dropping');
    }
    return [];
  }
  const out: ChatMessage[] = [];
  let dropped = 0;
  for (const item of data) {
    if (chatMessageGuard.safeParse(item).success) {
      out.push(item as ChatMessage);
    } else {
      dropped += 1;
    }
  }
  if (dropped > 0) {
    // eslint-disable-next-line no-console
    console.warn(`[api-client] dropped ${dropped} malformed inbound chat message(s)`);
  }
  return out;
}
