// TMAIL-350: unit tests for the alt-UI conversation grouping helper.
//
// The helper lives in themes/shadcn-prototype/src/features/email/
// threadGrouping.ts (the modern UI bundle). The shadcn-prototype package
// doesn't ship its own test runner, so we host the tests here in the
// classic frontend's vitest project — same pattern as
// themes-bulkSelection.test.ts and themes-replyContext.test.ts.
//
// What we're proving:
//   * groupByThread buckets a flat input by Message-ID + In-Reply-To +
//     References (the RFC 5322 §3.6.4 chain).
//   * Subject-fallback bucketing works when threading headers are missing.
//   * Thread-of-one buckets are still returned (no message gets dropped).
//   * Participants are de-duped case-insensitively and labelled.
//   * `latestTimestamp` + thread ordering pick the newest message.
//   * `hasUnread` and `hasStarred` are reduced across the thread bucket.
//   * normaliseSubject strips Re:/Fwd:/Fw:/Tr:/Rv: prefixes recursively.
//   * formatParticipants caps at 3 names + overflow badge.
import { describe, expect, it } from 'vitest';
import type { Email } from '../../../themes/shadcn-prototype/src/types/ui';
import {
  formatParticipants,
  groupByThread,
  normaliseSubject,
} from '../../../themes/shadcn-prototype/src/features/email/threadGrouping';

// Test-only factory. EmailClient produces Email view-models with these
// shapes via emailListItems.map(); a minimal version here keeps the
// individual cases readable.
function mk(
  id: string,
  subject: string,
  from: string,
  opts: Partial<Email> = {},
): Email {
  return {
    id,
    from,
    fromEmail: from,
    to: '',
    subject,
    preview: 'preview',
    body: '',
    timestamp: opts.timestamp ?? new Date('2026-05-30T10:00:00Z'),
    read: opts.read ?? true,
    starred: opts.starred ?? false,
    folder: 'INBOX',
    attachments: opts.attachments,
    messageId: opts.messageId ?? null,
    inReplyTo: opts.inReplyTo ?? null,
    references: opts.references ?? [],
  };
}

describe('normaliseSubject', () => {
  it('strips common reply / forward prefixes case-insensitively', () => {
    expect(normaliseSubject('Re: hello')).toBe('hello');
    expect(normaliseSubject('RE: Hello')).toBe('hello');
    expect(normaliseSubject('Fwd: Project plan')).toBe('project plan');
    expect(normaliseSubject('FW: project plan')).toBe('project plan');
  });

  it('strips reply prefixes recursively', () => {
    expect(normaliseSubject('Re: Re: Fwd: greetings')).toBe('greetings');
  });

  it('handles non-English variants', () => {
    expect(normaliseSubject('Tr: bonjour')).toBe('bonjour');
    expect(normaliseSubject('Aw: hallo')).toBe('hallo');
  });

  it('returns empty string for null / undefined / blank', () => {
    expect(normaliseSubject(null)).toBe('');
    expect(normaliseSubject(undefined)).toBe('');
    expect(normaliseSubject('   ')).toBe('');
  });
});

describe('groupByThread', () => {
  it('groups by In-Reply-To chain', () => {
    const newest = mk('3', 'Re: Re: hi', 'alice@x', {
      timestamp: new Date('2026-05-30T12:00:00Z'),
      messageId: '<m3@x>',
      inReplyTo: '<m2@x>',
      references: ['<m1@x>', '<m2@x>'],
    });
    const middle = mk('2', 'Re: hi', 'bob@x', {
      timestamp: new Date('2026-05-30T11:00:00Z'),
      messageId: '<m2@x>',
      inReplyTo: '<m1@x>',
      references: ['<m1@x>'],
    });
    const root = mk('1', 'hi', 'alice@x', {
      timestamp: new Date('2026-05-30T10:00:00Z'),
      messageId: '<m1@x>',
    });

    // Input is newest-first per EmailClient's ordering.
    const threads = groupByThread([newest, middle, root]);

    expect(threads).toHaveLength(1);
    expect(threads[0].messages).toHaveLength(3);
    expect(threads[0].subject).toBe('Re: Re: hi'); // newest message's subject
    expect(threads[0].latestTimestamp).toEqual(newest.timestamp);
    // Inside the thread, newest-first is preserved.
    expect(threads[0].messages.map((m) => m.id)).toEqual(['3', '2', '1']);
  });

  it('groups by References when In-Reply-To is missing', () => {
    const reply = mk('2', 'Re: hi', 'bob@x', {
      messageId: '<m2@x>',
      references: ['<m1@x>'],
    });
    const root = mk('1', 'hi', 'alice@x', { messageId: '<m1@x>' });
    const threads = groupByThread([reply, root]);
    expect(threads).toHaveLength(1);
    expect(threads[0].messages.map((m) => m.id)).toEqual(['2', '1']);
  });

  it('falls back to normalised subject when threading headers are absent', () => {
    // Two messages with no Message-ID / In-Reply-To / References but the
    // same normalised subject — should land in the same thread bucket.
    const a = mk('a', 'Re: project status', 'alice@x', {
      timestamp: new Date('2026-05-30T12:00:00Z'),
    });
    const b = mk('b', 'project status', 'bob@x', {
      timestamp: new Date('2026-05-30T11:00:00Z'),
    });
    const threads = groupByThread([a, b]);
    expect(threads).toHaveLength(1);
    expect(threads[0].messages.map((m) => m.id).sort()).toEqual(['a', 'b']);
  });

  it('keeps unrelated subjects in separate threads', () => {
    const a = mk('a', 'hello there', 'alice@x', { messageId: '<a@x>' });
    const b = mk('b', 'totally different', 'bob@x', { messageId: '<b@x>' });
    const threads = groupByThread([a, b]);
    expect(threads).toHaveLength(2);
  });

  it('returns thread-of-one buckets unchanged', () => {
    const solo = mk('s', 'only one', 'alice@x', { messageId: '<s@x>' });
    const threads = groupByThread([solo]);
    expect(threads).toHaveLength(1);
    expect(threads[0].messages).toHaveLength(1);
    expect(threads[0].id).toBe('<s@x>');
  });

  it('handles messages with no Message-ID via uid fallback', () => {
    const a = mk('42', 'no headers here', 'alice@x');
    const threads = groupByThread([a]);
    expect(threads).toHaveLength(1);
    expect(threads[0].id).toBe('uid:42');
  });

  it('de-duplicates participants by bare address case-insensitively', () => {
    const a = mk('1', 'hi', 'Alice <alice@x>', { messageId: '<m1@x>' });
    const b = mk('2', 'Re: hi', 'alice@x', {
      messageId: '<m2@x>',
      inReplyTo: '<m1@x>',
    });
    const c = mk('3', 'Re: Re: hi', 'Bob Smith <bob@x>', {
      messageId: '<m3@x>',
      inReplyTo: '<m2@x>',
    });
    // Newest-first input order: [c (newest), b, a (oldest)].
    const threads = groupByThread([c, b, a]);
    expect(threads).toHaveLength(1);
    // Bob is unique. Alice appears twice ("alice@x" then "Alice <alice@x>"),
    // both with bare address "alice@x" → deduped. First-seen wins so the
    // label is "alice" (from the bare-email form in message b).
    expect(threads[0].participants).toEqual(['Bob', 'alice']);
  });

  it('keeps display-name label when display-named form appears first', () => {
    // Reverse of the case above: when "Alice <alice@x>" appears before
    // the bare-email form, the display-name label wins.
    const a = mk('1', 'hi', 'Alice <alice@x>', {
      messageId: '<m1@x>',
      timestamp: new Date('2026-05-30T12:00:00Z'),
    });
    const b = mk('2', 'Re: hi', 'alice@x', {
      messageId: '<m2@x>',
      inReplyTo: '<m1@x>',
      timestamp: new Date('2026-05-30T11:00:00Z'),
    });
    // Newest-first: [a, b].
    const threads = groupByThread([a, b]);
    expect(threads[0].participants).toEqual(['Alice']);
  });

  it('reduces hasUnread + hasStarred across the bucket', () => {
    const a = mk('1', 'hi', 'alice@x', {
      messageId: '<a@x>',
      read: true,
      starred: false,
    });
    const b = mk('2', 'Re: hi', 'bob@x', {
      messageId: '<b@x>',
      inReplyTo: '<a@x>',
      read: false, // one unread message in the thread
      starred: true, // one starred message
    });
    const threads = groupByThread([b, a]);
    expect(threads).toHaveLength(1);
    expect(threads[0].hasUnread).toBe(true);
    expect(threads[0].hasStarred).toBe(true);
  });

  it('sorts threads by the newest message across buckets', () => {
    const oldRoot = mk('1', 'first', 'alice@x', {
      messageId: '<a@x>',
      timestamp: new Date('2026-05-29T10:00:00Z'),
    });
    const newRoot = mk('2', 'second', 'bob@x', {
      messageId: '<b@x>',
      timestamp: new Date('2026-05-30T10:00:00Z'),
    });
    // Pass them in reverse order to make sure sort kicks in.
    const threads = groupByThread([oldRoot, newRoot]);
    expect(threads[0].id).toBe('<b@x>');
    expect(threads[1].id).toBe('<a@x>');
  });

  it('returns an empty array for an empty input', () => {
    expect(groupByThread([])).toEqual([]);
  });
});

describe('formatParticipants', () => {
  it('returns the single name when there is exactly one participant', () => {
    expect(formatParticipants(['Alice'])).toBe('Alice');
  });

  it('joins up to 3 names with commas', () => {
    expect(formatParticipants(['Alice', 'Bob'])).toBe('Alice, Bob');
    expect(formatParticipants(['Alice', 'Bob', 'Carol'])).toBe('Alice, Bob, Carol');
  });

  it('caps at 3 names + overflow badge for larger threads', () => {
    expect(formatParticipants(['Alice', 'Bob', 'Carol', 'Dave', 'Eve'])).toBe(
      'Alice, Bob, Carol +2',
    );
  });

  it('handles an empty participant list defensively', () => {
    expect(formatParticipants([])).toBe('(unknown sender)');
  });
});
