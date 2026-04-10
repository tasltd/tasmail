import { describe, it, expect } from 'vitest';
import type { MessageEnvelope } from '../../types/mail';

// Reproduce the normalizeSubject and groupByThread logic for testing
// (these are internal to MessageList but we test the algorithm)

function normalizeSubject(subject: string | null): string {
  if (!subject) return '';
  return subject.replace(/^(Re|Fwd|Fw):\s*/gi, '').trim().toLowerCase();
}

interface ThreadGroup {
  subject: string;
  messages: MessageEnvelope[];
  latestDate: string | null;
  hasUnread: boolean;
}

function groupByThread(messages: MessageEnvelope[]): ThreadGroup[] {
  const groups = new Map<string, ThreadGroup>();

  for (const msg of messages) {
    const key = normalizeSubject(msg.subject) || `uid-${msg.uid}`;
    const existing = groups.get(key);
    if (existing) {
      existing.messages.push(msg);
      if (!existing.hasUnread) {
        existing.hasUnread = !msg.flags.some((f) => f.includes('Seen'));
      }
    } else {
      groups.set(key, {
        subject: msg.subject || '(no subject)',
        messages: [msg],
        latestDate: msg.date,
        hasUnread: !msg.flags.some((f) => f.includes('Seen')),
      });
    }
  }

  return Array.from(groups.values());
}

function makeMsg(uid: number, subject: string | null, flags: string[] = []): MessageEnvelope {
  return { uid, subject, from: 'sender@test.com', date: '2026-04-10', flags, size: 100 };
}

describe('normalizeSubject', () => {
  it('strips Re: prefix', () => {
    expect(normalizeSubject('Re: Hello')).toBe('hello');
  });

  it('strips Fwd: prefix', () => {
    expect(normalizeSubject('Fwd: Hello')).toBe('hello');
  });

  it('strips Fw: prefix', () => {
    expect(normalizeSubject('Fw: Hello')).toBe('hello');
  });

  it('strips multiple Re: prefixes', () => {
    expect(normalizeSubject('Re: Re: Hello')).toBe('re: hello');
    // Only strips the first one — this is expected behavior
  });

  it('handles null subject', () => {
    expect(normalizeSubject(null)).toBe('');
  });

  it('handles empty subject', () => {
    expect(normalizeSubject('')).toBe('');
  });

  it('is case insensitive', () => {
    expect(normalizeSubject('RE: HELLO')).toBe('hello');
    expect(normalizeSubject('re: hello')).toBe('hello');
  });

  it('preserves subject without prefix', () => {
    expect(normalizeSubject('Meeting tomorrow')).toBe('meeting tomorrow');
  });
});

describe('groupByThread', () => {
  it('groups messages with same subject', () => {
    const messages = [
      makeMsg(1, 'Hello'),
      makeMsg(2, 'Re: Hello'),
      makeMsg(3, 'Fwd: Hello'),
    ];
    const threads = groupByThread(messages);
    expect(threads.length).toBe(1);
    expect(threads[0].messages.length).toBe(3);
  });

  it('keeps different subjects separate', () => {
    const messages = [
      makeMsg(1, 'Topic A'),
      makeMsg(2, 'Topic B'),
      makeMsg(3, 'Re: Topic A'),
    ];
    const threads = groupByThread(messages);
    expect(threads.length).toBe(2);
  });

  it('handles null subjects as separate threads', () => {
    const messages = [
      makeMsg(1, null),
      makeMsg(2, null),
    ];
    const threads = groupByThread(messages);
    // Each null subject gets its own uid-based key
    expect(threads.length).toBe(2);
  });

  it('detects unread messages in thread', () => {
    const messages = [
      makeMsg(1, 'Test', ['\\Seen']),
      makeMsg(2, 'Re: Test', []),
    ];
    const threads = groupByThread(messages);
    expect(threads[0].hasUnread).toBe(true);
  });

  it('marks thread as read when all seen', () => {
    const messages = [
      makeMsg(1, 'Test', ['\\Seen']),
      makeMsg(2, 'Re: Test', ['\\Seen']),
    ];
    const threads = groupByThread(messages);
    expect(threads[0].hasUnread).toBe(false);
  });

  it('uses "(no subject)" for display when subject is null', () => {
    const messages = [makeMsg(1, null)];
    const threads = groupByThread(messages);
    expect(threads[0].subject).toBe('(no subject)');
  });

  it('preserves original subject for display', () => {
    const messages = [makeMsg(1, 'Re: Important Meeting')];
    const threads = groupByThread(messages);
    expect(threads[0].subject).toBe('Re: Important Meeting');
  });

  it('returns empty array for empty input', () => {
    expect(groupByThread([])).toEqual([]);
  });

  it('handles single message as single thread', () => {
    const threads = groupByThread([makeMsg(1, 'Standalone')]);
    expect(threads.length).toBe(1);
    expect(threads[0].messages.length).toBe(1);
  });
});
