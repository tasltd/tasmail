// TMAIL-319: pure helpers that turn an open `FullMessage` into the prefill
// payload the ComposeModal opens with for Reply / Reply All / Forward, plus
// the RFC 5322 `In-Reply-To` / `References` headers that the backend
// /api/messages/schedule endpoint now persists (migration 077) and the email
// scheduler stamps onto the outbound message (smtp_service::build_outgoing_message).
//
// Why pure functions in their own module:
//   * Keeps ComposeModal + EmailReader focused on presentation.
//   * Lets the build logic be unit-tested without standing up React + lettre.
//   * Sidesteps the "ComposeModal owns 9 useState calls and is fragile to
//     diff" problem — the prefill payload is a single immutable object passed
//     in via props, and ComposeModal reseeds its form state when it changes.
//
// Reply / Reply All / Forward semantics follow the same conventions Gmail,
// Outlook, and the classic SPA's existing tests assume:
//   * Subject prefix:   Re:  on Reply / Reply All (only if not already present)
//                       Fwd: on Forward          (only if not already present)
//   * Recipients:       Reply       → from
//                       Reply All   → from + original to (minus self) + original cc
//                       Forward     → empty (user picks the recipient)
//   * Quoted body:      "On <date>, <from> wrote:" then `> `-prefixed source body
//   * Threading hdrs:   Reply / Reply All / Forward all set in_reply_to + references
//                       Forward keeps the original chain so the new thread can be
//                       re-attached if the recipient happens to be on the chain.
// Relative-path type import (rather than the `@/types/mail` alias) so the
// helper is importable by unit tests in the classic frontend's vitest
// project, which doesn't know about the shadcn alias.
import type { FullMessage } from '../../types/mail';

/** Discriminator for the three button-driven entry points. */
export type ReplyKind = 'reply' | 'replyAll' | 'forward';

/**
 * Prefill payload passed into ComposeModal via `replyContext`. ComposeModal
 * reseeds its To / Cc / Subject / Body / threading headers from this object
 * the moment it opens (or whenever the object identity changes), and clears
 * back to a blank compose when it's null.
 */
export interface ReplyContext {
  kind: ReplyKind;
  to: string[];
  cc: string[];
  subject: string;
  /** Plain-text body, including the `> `-quoted source. */
  body: string;
  /** Value for the outbound `In-Reply-To` header (the source message's id). */
  inReplyTo: string | null;
  /** Full chain for the outbound `References` header — never empty for a real reply. */
  references: string[];
}

const ADDR_REGEX = /<([^<>@\s]+@[^<>@\s]+)>/;

/** Strip a display-name wrapper from an address. `"Alice <a@x>"` → `"a@x"`. */
export function extractAddress(raw: string): string {
  const trimmed = raw.trim();
  const match = trimmed.match(ADDR_REGEX);
  if (match) return match[1].toLowerCase();
  return trimmed.toLowerCase();
}

/** Strip an existing `Re:` / `Fwd:` prefix (case-insensitive, repeated). */
export function stripSubjectPrefix(subject: string): string {
  let s = (subject ?? '').trim();
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const m = s.match(/^(re|fw|fwd)\s*:\s*/i);
    if (!m) return s;
    s = s.slice(m[0].length);
  }
}

/** Re-prefix a subject, avoiding double-prefixing. */
export function applySubjectPrefix(subject: string, prefix: 'Re' | 'Fwd'): string {
  const bare = stripSubjectPrefix(subject || '');
  const display = bare.length > 0 ? bare : '(no subject)';
  return `${prefix}: ${display}`;
}

/**
 * Build the `> `-quoted body block. Mirrors the de-facto Gmail / classic
 * Thunderbird convention: blank line, attribution line, then every source
 * line prefixed with `> ` (an empty source line becomes `>`). Falls back to
 * the HTML body stripped of tags when only an HTML part is present so reply
 * bodies don't end up empty for HTML-only newsletters.
 */
export function quoteBody(message: FullMessage): string {
  const date = message.date ? new Date(message.date) : null;
  const dateStr = date && !isNaN(date.getTime())
    ? date.toLocaleString(undefined, {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
      })
    : 'an earlier date';
  const from = message.from?.trim() || '(unknown sender)';
  const attribution = `On ${dateStr}, ${from} wrote:`;

  let source = message.text_body ?? '';
  if (!source && message.html_body) {
    // Crude HTML-to-text fallback: drop tags, decode the common entities, and
    // collapse runs of blank lines. Good enough for the quoted block; the
    // real mail body still ships in the `html_body` field when needed.
    source = message.html_body
      .replace(/<br\s*\/?>/gi, '\n')
      .replace(/<\/p>/gi, '\n\n')
      .replace(/<[^>]+>/g, '')
      .replace(/&nbsp;/g, ' ')
      .replace(/&amp;/g, '&')
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>')
      .replace(/&quot;/g, '"')
      .replace(/\n{3,}/g, '\n\n')
      .trim();
  }

  const quoted = (source || '').split('\n').map((line) => `> ${line}`.trimEnd()).join('\n');
  return `\n\n${attribution}\n${quoted}`;
}

/**
 * Build the outbound `References` chain.
 *
 * RFC 5322 §3.6.4 says: take the source message's existing `References`
 * (which is the thread's history) and append the source's `Message-Id`. If
 * the source had no `References` (it was a thread root), use its
 * `Message-Id` alone. Drops empty / whitespace-only entries defensively so a
 * malformed source can't produce an empty header element downstream.
 */
export function buildReferences(message: FullMessage): string[] {
  const existing = (message.references ?? [])
    .map((r) => r.trim())
    .filter((r) => r.length > 0);
  const id = message.message_id?.trim() ?? '';
  if (id && !existing.includes(id)) {
    return [...existing, id];
  }
  return existing;
}

/**
 * Build the Reply All recipient lists from the source message. `selfAddress`
 * is filtered out from both `to` and `cc` so the user doesn't email
 * themselves a copy of their own reply.
 */
function buildReplyAllRecipients(
  message: FullMessage,
  selfAddress: string | null,
): { to: string[]; cc: string[] } {
  const self = selfAddress?.toLowerCase() ?? null;
  const fromAddr = message.from ? extractAddress(message.from) : null;
  const isSelf = (addr: string) => self != null && extractAddress(addr) === self;
  const isFrom = (addr: string) => fromAddr != null && extractAddress(addr) === fromAddr;

  const dedupe = (list: string[]): string[] => {
    const seen = new Set<string>();
    const out: string[] = [];
    for (const raw of list) {
      const norm = extractAddress(raw);
      if (norm.length === 0 || seen.has(norm)) continue;
      seen.add(norm);
      out.push(raw);
    }
    return out;
  };

  const to = dedupe([
    message.from ?? '',
    ...(message.to ?? []),
  ].filter((addr) => addr.trim().length > 0 && !isSelf(addr)));

  const cc = dedupe(
    (message.cc ?? []).filter((addr) => addr.trim().length > 0 && !isSelf(addr) && !isFrom(addr)),
  );

  return { to, cc };
}

/**
 * Top-level factory — turns an open message into a `ReplyContext` for the
 * ComposeModal. `kind` picks the variant; `selfAddress` is the logged-in
 * mailbox (used to filter the Reply All recipient list); both inputs are
 * optional so the call site can pass `null` for an unauthenticated /
 * about-to-load state without crashing.
 */
export function buildReplyContext(
  message: FullMessage,
  kind: ReplyKind,
  selfAddress: string | null = null,
): ReplyContext {
  const references = buildReferences(message);
  const inReplyTo = message.message_id?.trim() || null;
  const quoted = quoteBody(message);

  if (kind === 'forward') {
    return {
      kind,
      to: [],
      cc: [],
      subject: applySubjectPrefix(message.subject ?? '', 'Fwd'),
      body: quoted,
      inReplyTo,
      references,
    };
  }

  if (kind === 'replyAll') {
    const { to, cc } = buildReplyAllRecipients(message, selfAddress);
    return {
      kind,
      to,
      cc,
      subject: applySubjectPrefix(message.subject ?? '', 'Re'),
      body: quoted,
      inReplyTo,
      references,
    };
  }

  return {
    kind: 'reply',
    to: message.from ? [message.from] : [],
    cc: [],
    subject: applySubjectPrefix(message.subject ?? '', 'Re'),
    body: quoted,
    inReplyTo,
    references,
  };
}
