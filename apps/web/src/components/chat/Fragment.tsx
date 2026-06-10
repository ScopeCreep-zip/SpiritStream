// Typed dispatch over `MessageFragment`. Every parsing decision — emote
// resolution, mention detection, link extraction, bits-prefix matching —
// happens in `crates/core` before the message reaches the renderer.
// This component **must not** introduce regex, fragment splitting, or
// `dangerouslySetInnerHTML`. If a regex would help here, the right fix
// is a missing fragment variant on the backend.
//
// Rules (from the plan, enforced by review):
//   1. No regex. Missing parsing belongs in `crates/core`.
//   2. No dangerouslySetInnerHTML. Every fragment renders its own primitive.
//   3. No fragment merge/split in JSX. Render `fragments.length` components in order.
//   4. Message-level framing reads `flags` / `elevatedTier` / `highlightColor` / `author`.
//      Never derived by scanning fragments.

import type { ReactElement } from 'react';
import { cn } from '@/lib/cn';
import type { FragmentColor, MessageFragment } from '@spiritstream/types';

function colorStyle(color: FragmentColor | null | undefined): React.CSSProperties | undefined {
  return color ? { color: color.hex } : undefined;
}

function TextFragment({
  content,
  color,
  style,
}: Extract<MessageFragment, { kind: 'text' }>): ReactElement {
  let styleClass: string | undefined;
  if (style === 'Bold') styleClass = 'font-bold';
  else if (style === 'Italic') styleClass = 'italic';
  else if (style === 'Monospace') styleClass = 'font-mono';
  return (
    <span className={cn('chat-frag chat-frag--text', styleClass)} style={colorStyle(color)}>
      {content}
    </span>
  );
}

function MentionFragment({
  login,
  displayName,
  userColor,
}: Extract<MessageFragment, { kind: 'mention' }>): ReactElement {
  return (
    <span
      className="chat-frag chat-frag--mention font-semibold"
      data-login={login}
      style={colorStyle(userColor)}
    >
      @{displayName}
    </span>
  );
}

function LinkFragment({
  url,
  display,
  isSafeBrowsingFlagged,
}: Extract<MessageFragment, { kind: 'link' }>): ReactElement {
  // `noreferrer` is non-negotiable so the host site doesn't see the
  // SpiritStream origin. `rel=noopener` keeps the new tab from
  // back-channeling via `window.opener`.
  return (
    <a
      className={cn(
        'chat-frag chat-frag--link underline underline-offset-2 break-all',
        isSafeBrowsingFlagged && 'chat-frag--link-flagged'
      )}
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      data-flagged={isSafeBrowsingFlagged ? 'true' : undefined}
    >
      {display}
    </a>
  );
}

function EmoteFragment({
  id,
  name,
  animated,
  zeroWidth,
  url1x,
  url2x,
}: Extract<MessageFragment, { kind: 'emote' }>): ReactElement {
  return (
    <img
      className={cn(
        'chat-frag chat-frag--emote inline-block align-middle',
        zeroWidth && 'chat-frag--emote-overlay'
      )}
      src={url1x}
      srcSet={`${url1x} 1x, ${url2x} 2x`}
      alt={name}
      data-emote-id={id}
      data-animated={animated ? 'true' : undefined}
      loading="lazy"
      decoding="async"
    />
  );
}

function LayeredEmoteFragment({
  base,
  overlays,
}: Extract<MessageFragment, { kind: 'layeredEmote' }>): ReactElement {
  // Layered emotes are a base emote with zero-width overlays (Twitch +
  // 7TV). The CSS absolute-positions each overlay over the base.
  // `base` is always an Emote fragment per the core contract.
  return (
    <span className="chat-frag chat-frag--layered relative inline-block">
      <Fragment fragment={base} />
      {overlays.map((overlay, i) => (
        <span
          key={i}
          className="chat-frag chat-frag--layered-overlay absolute inset-0 pointer-events-none"
        >
          <Fragment fragment={overlay} />
        </span>
      ))}
    </span>
  );
}

function BadgeFragment({
  id,
  title,
  url1x,
  url2x,
  tint,
}: Extract<MessageFragment, { kind: 'badge' }>): ReactElement {
  return (
    <img
      className="chat-frag chat-frag--badge inline-block align-middle h-4 w-4"
      src={url1x}
      srcSet={`${url1x} 1x, ${url2x} 2x`}
      alt={title}
      title={title}
      data-badge-id={id}
      style={tint ? { filter: undefined, backgroundColor: tint.hex } : undefined}
      loading="lazy"
      decoding="async"
    />
  );
}

function CheermoteFragment({
  prefix,
  amount,
  tierColor,
  url1x,
  url2x,
}: Extract<MessageFragment, { kind: 'cheermote' }>): ReactElement {
  return (
    <span className="chat-frag chat-frag--cheermote inline-flex items-baseline gap-1">
      <img
        className="inline-block align-middle h-5 w-5"
        src={url1x}
        srcSet={`${url1x} 1x, ${url2x} 2x`}
        alt={`${prefix}${amount}`}
        loading="lazy"
        decoding="async"
      />
      <span
        className="font-semibold tabular-nums text-[var(--cheer-tier)]"
        // Cheermote tier color is per-message data from the platform, not a
        // static token — inject as a CSS var (Modal.tsx pattern).
        style={{ '--cheer-tier': tierColor.hex } as React.CSSProperties}
      >
        {amount}
      </span>
    </span>
  );
}

function TimestampFragment({
  formatted,
}: Extract<MessageFragment, { kind: 'timestamp' }>): ReactElement {
  // Backend has already formatted the string per the user's locale +
  // timezone — render verbatim. No `new Date()` work here.
  return (
    <span className="chat-frag chat-frag--timestamp text-text-tertiary text-[0.7rem] tabular-nums shrink-0">
      {formatted}
    </span>
  );
}

function ReplyPreviewFragment({
  parentDisplayName,
  parentTextPreview,
}: Extract<MessageFragment, { kind: 'replyPreview' }>): ReactElement {
  return (
    <span className="chat-frag chat-frag--reply-preview block text-text-tertiary text-xs italic mb-1">
      ↪ <span className="font-semibold">@{parentDisplayName}</span>: {parentTextPreview}
    </span>
  );
}

export function Fragment({ fragment }: { fragment: MessageFragment }): ReactElement | null {
  switch (fragment.kind) {
    case 'text':
      return <TextFragment {...fragment} />;
    case 'mention':
      return <MentionFragment {...fragment} />;
    case 'link':
      return <LinkFragment {...fragment} />;
    case 'emote':
      return <EmoteFragment {...fragment} />;
    case 'layeredEmote':
      return <LayeredEmoteFragment {...fragment} />;
    case 'badge':
      return <BadgeFragment {...fragment} />;
    case 'cheermote':
      return <CheermoteFragment {...fragment} />;
    case 'timestamp':
      return <TimestampFragment {...fragment} />;
    case 'replyPreview':
      return <ReplyPreviewFragment {...fragment} />;
    case 'linebreak':
      return <br />;
    default: {
      // Exhaustiveness check — if a new variant lands on the backend
      // without a renderer arm here, TS fails the build.
      const _exhaustive: never = fragment;
      return _exhaustive;
    }
  }
}
