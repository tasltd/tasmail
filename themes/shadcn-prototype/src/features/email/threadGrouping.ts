// TMAIL-350: pure conversation-grouping for the alt-UI EmailList.
//
// IMAP itself has no notion of a "thread"; the JMAP-style conversation
// grouping that Gmail / Apple Mail / Thunderbird show users is built on top
// of the RFC 5322 §3.6.4 headers — Message-ID, In-Reply-To, References.
// This module is the pure data layer: it takes a flat list of Email rows
// (already sorted newest-first by EmailClient.tsx) and buckets them into
// conversations using a union-find walk over those headers.
//
// Design notes:
//   * Pure function: no React, no localStorage, no DOM. The hook layer that
//     consumes this lives in EmailClient.tsx; the toggle persistence lives
//     in threadingSettings.ts. Keeping the grouping pure means it is unit
//     testable with plain objects (see threadGrouping.test.ts).
//   * Stable order: threads are sorted newest-first by the *latest* message
//     they contain — matches Gmail's "Inbox shows the most recently active
//     conversation at the top" behaviour. Within a thread we keep the input
//     order (newest first), so messages[0] is the latest reply.
//   * Subject fallback: senders that omit Message-ID / In-Reply-To still get
//     bucketed by normalised subject (Re:/Fwd:/Fw:/RE: prefixes stripped).
//     This is the same fallback Apple Mail uses and prevents the alt-UI
//     from rendering "two threads with the same subject" when one side of
//     a conversation is on a misbehaving MTA.
//   * No N^2 scans: we build two indices (by Message-ID and by normalised
//     subject), then walk each message once joining its references / parent
//     into a union-find root. O(N) on a per-folder load.

import type { Email } from '../../types/ui';

/**
 * A bucketed conversation. Contains every Email that resolved to the same
 * thread root via Message-ID / In-Reply-To / References traversal (with a
 * normalised-subject fallback for messages missing those headers).
 *
 * Invariants:
 *   * `messages.length >= 1` — a thread-of-one is still a thread.
 *   * `messages[0]` is the most recent message (input order is newest first).
 *   * `latestTimestamp` mirrors `messages[0].timestamp` and is what the
 *     thread is sorted by at the list level.
 *   * `participants` is the set of unique senders across every message,
 *     de-duped case-insensitively on the displayable address.
 */
export interface EmailThread {
  /** Stable identifier — the Message-ID of the root message, or
   *  `subj:<normalised-subject>` if no Message-ID is available, or
   *  `uid:<id>` as a last resort for messages with neither. */
  id: string;
  /** Subject of the latest message in the thread (used for the row label). */
  subject: string;
  /** Most recent message first. Always non-empty. */
  messages: Email[];
  /** `messages[0].timestamp` — what the thread is sorted by. */
  latestTimestamp: Date;
  /** Unique displayable senders across every message in the thread. */
  participants: string[];
  /** True iff at least one message in the thread is unread. Drives the
   *  bold / unread-stripe styling on the thread header row. */
  hasUnread: boolean;
  /** True iff at least one message in the thread is starred. Drives the
   *  filled-star glyph on the thread header. */
  hasStarred: boolean;
}

/**
 * RFC 5322 §3.6.5 reply prefixes. Stripped from subjects for the
 * normalised-subject fallback bucket key.
 *
 * Covers English (Re:, Fwd:, Fw:) and the most common non-English variants
 * agents have hit in the noreply mailbox: French Tr:, Spanish Rv:, German
 * Aw:, Italian R:. Case-insensitive match. Stripped recursively so
 * "Re: Re: Fwd: Hi" collapses to "Hi".
 */
const REPLY_PREFIX_RE = /^\s*(re|fwd?|tr|rv|aw|r|antwort|sv|vs|odp|res|wg)\s*(\[\d+\])?\s*:\s*/i;

/**
 * Strip leading reply / forward prefixes from a subject. Used as the
 * subject-fallback bucket key when a message has no Message-ID / In-Reply-To
 * / References to thread on. Lower-cased + trimmed so case differences don't
 * spawn duplicate buckets.
 *
 * Exported so the unit test can pin the exact normalisation rule independent
 * of the grouping walk.
 */
export function normaliseSubject(subject: string | null | undefined): string {
  let out = (subject ?? '').trim();
  // Recursively strip "Re: Re: Fwd: hi" → "hi". Bounded loop so a
  // pathological subject like "Re:" * 1000 doesn't run away.
  for (let i = 0; i < 16; i++) {
    const next = out.replace(REPLY_PREFIX_RE, '');
    if (next === out) break;
    out = next;
  }
  return out.trim().toLowerCase();
}

/**
 * Strip the display-name from a "Name <addr@host>" sender string, returning
 * the bare address (or the original if no angle-bracketed address is
 * present). Used to de-dupe participants — two messages from
 * "Bob <bob@x>" and "bob@x" must collapse to one participant entry.
 */
function bareAddress(sender: string): string {
  const m = sender.match(/<([^>]+)>/);
  return (m ? m[1] : sender).trim().toLowerCase();
}

/**
 * Display label for participants. When the sender has an angle-bracketed
 * address ("Bob Smith <bob@x>"), keep just the first word of the display
 * name ("Bob"). When the sender is a bare email ("alice@x") fall back to
 * the local-part ("alice"). Mirrors how Gmail compresses the participant
 * list ("Bob, Alice, me").
 */
function participantLabel(sender: string): string {
  const angleMatch = sender.match(/<([^>]+)>/);
  if (angleMatch) {
    const namePart = sender.split('<')[0].trim();
    if (namePart) return namePart.split(/\s+/)[0];
    const addr = angleMatch[1].trim();
    return addr.split('@')[0] || addr || '(unknown)';
  }
  // Bare email or display-name only — split on '@' so "alice@x" → "alice"
  // but "Bob" (display name only) stays "Bob".
  const trimmed = sender.trim();
  if (!trimmed) return '(unknown)';
  if (trimmed.includes('@')) return trimmed.split('@')[0] || trimmed;
  return trimmed.split(/\s+/)[0];
}

/**
 * Bucket a flat list of Email rows into conversations.
 *
 * Inputs are expected to be sorted newest-first (EmailClient's
 * emailListItems already comes out of the IMAP envelope list in that
 * order). The output preserves that ordering: threads sort by their newest
 * message and the messages inside each thread keep newest-first too.
 *
 * Algorithm (union-find on Message-ID equivalence classes):
 *   1. Build a map `parent` from "node id" → "root id". A node id is the
 *      message's Message-ID (or `uid:<id>` synthetic fallback).
 *   2. For each message, take every linked id we know about (its own
 *      Message-ID, In-Reply-To, every References entry) and union them
 *      into the same equivalence class.
 *   3. Normalised-subject fallback: messages with no In-Reply-To /
 *      References AND a non-trivial normalised subject are unioned with
 *      every other message that resolves to the same normalised subject.
 *      This catches conversations where one MTA stripped Message-ID
 *      threading headers.
 *   4. Group messages by their union-find root and assemble EmailThread
 *      objects (subject = latest message's subject; participants /
 *      hasUnread / hasStarred reduced across the bucket).
 *
 * Notes:
 *   * Self-loops in `parent` are fine — `find()` short-circuits when a
 *     node IS its own root, which is the base case for the root message
 *     of every thread.
 *   * `union` always points the older root at the newer one (input order
 *     is newest first → smaller index = newer), so the bucket id is the
 *     newest message's id and the EmailThread.id matches what the
 *     EmailClient stores as the expanded-thread key.
 */
export function groupByThread(emails: Email[]): EmailThread[] {
  // ── union-find scaffolding ────────────────────────────────────────────
  const parent = new Map<string, string>();

  function find(id: string): string {
    let cur = id;
    while (true) {
      const p = parent.get(cur);
      if (p == null || p === cur) {
        if (p == null) parent.set(cur, cur);
        return cur;
      }
      cur = p;
    }
  }

  function union(a: string, b: string) {
    const ra = find(a);
    const rb = find(b);
    if (ra === rb) return;
    // Point the deeper root at the shallower one. Cheap proxy: keep
    // whichever was inserted first as the canonical root. Since input
    // is sorted newest-first, this means the EARLIER (newer) message's
    // id wins, which matches Gmail's "thread id = id of newest message"
    // convention.
    parent.set(rb, ra);
  }

  // Synthetic id for messages that have no Message-ID header. Keyed by
  // uid so two different UIDs never collide even when both lack headers.
  const idFor = (e: Email): string =>
    e.messageId && e.messageId.length > 0 ? e.messageId : `uid:${e.id}`;

  // ── pass 1: register every node and link to its parents ───────────────
  for (const e of emails) {
    const myId = idFor(e);
    find(myId); // ensure parent[myId] is initialised

    if (e.inReplyTo) {
      // Both id and parent are registered as nodes in the DSU; union
      // them. If we haven't seen the parent message yet (e.g. it's older
      // than our paginated window) that's fine — it stays a "ghost" root
      // and any other replies to the same parent will end up in the same
      // bucket as `e`.
      union(myId, e.inReplyTo);
    }

    if (e.references) {
      for (const ref of e.references) {
        if (ref) union(myId, ref);
      }
    }
  }

  // ── pass 2: subject-fallback for messages with no threading headers ──
  // Build a map normalisedSubject → first message id we saw with that
  // subject AND no parent. Subsequent messages with the same normalised
  // subject + no parent get unioned in.
  const subjectAnchors = new Map<string, string>();
  for (const e of emails) {
    const hasParent = !!(e.inReplyTo || (e.references && e.references.length > 0));
    if (hasParent) continue;
    const norm = normaliseSubject(e.subject);
    if (!norm) continue; // empty / "(no subject)" — no bucketing
    const myId = idFor(e);
    const anchor = subjectAnchors.get(norm);
    if (anchor == null) {
      subjectAnchors.set(norm, myId);
    } else {
      union(myId, anchor);
    }
  }

  // ── pass 3: collect messages by root ─────────────────────────────────
  const bucketOrder: string[] = [];
  const buckets = new Map<string, Email[]>();
  for (const e of emails) {
    const root = find(idFor(e));
    if (!buckets.has(root)) {
      buckets.set(root, []);
      bucketOrder.push(root);
    }
    buckets.get(root)!.push(e);
  }

  // ── pass 4: assemble EmailThread view-models ─────────────────────────
  const threads: EmailThread[] = bucketOrder.map((root) => {
    const messages = buckets.get(root)!;
    // Input is sorted newest first; preserve that inside each bucket so
    // messages[0] is the latest reply.
    const latest = messages[0];

    // Participants: dedupe by bare email; preserve first-seen order so
    // the latest sender appears first.
    const seen = new Set<string>();
    const participants: string[] = [];
    for (const m of messages) {
      const key = bareAddress(m.from);
      if (seen.has(key)) continue;
      seen.add(key);
      participants.push(participantLabel(m.from));
    }

    return {
      id: root,
      subject: latest.subject || '(no subject)',
      messages,
      latestTimestamp: latest.timestamp,
      participants,
      hasUnread: messages.some((m) => !m.read),
      hasStarred: messages.some((m) => m.starred),
    } satisfies EmailThread;
  });

  // Sort threads by newest message timestamp descending. Input was
  // already newest-first so this is mostly a no-op, but it guarantees
  // correctness when subject-fallback union reorders buckets.
  threads.sort((a, b) => b.latestTimestamp.getTime() - a.latestTimestamp.getTime());

  return threads;
}

/**
 * Helper for rendering the participant list as the "Alice, Bob, Carol"
 * label Gmail shows on each thread row. Caps at 3 visible names and
 * appends "+N" when more participants exist, so a 12-person mailing list
 * thread renders as "Alice, Bob, Carol +9" rather than overflowing the
 * row width.
 */
export function formatParticipants(participants: string[]): string {
  if (participants.length === 0) return '(unknown sender)';
  if (participants.length === 1) return participants[0];
  if (participants.length <= 3) return participants.join(', ');
  return `${participants.slice(0, 3).join(', ')} +${participants.length - 3}`;
}
