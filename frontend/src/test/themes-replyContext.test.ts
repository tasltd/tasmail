// TMAIL-319: unit tests for the alt-UI Reply / Reply All / Forward prefill
// helpers. The helpers live in themes/shadcn-prototype/src/features/email/
// replyContext.ts (the modern UI bundle), but the shadcn-prototype package
// doesn't ship its own test runner — so we host the tests here in the classic
// frontend's vitest project (already configured for the rest of the SPA).
// The helper file imports the FullMessage type via relative path for exactly
// this reason. See the helper's leading comment for the rules-of-the-road.
import { describe, expect, it } from 'vitest';
import {
  applySubjectPrefix,
  buildReferences,
  buildReplyContext,
  extractAddress,
  quoteBody,
  stripSubjectPrefix,
} from '../../../themes/shadcn-prototype/src/features/email/replyContext';

function makeMessage(overrides: Partial<{
  uid: number;
  subject: string | null;
  from: string | null;
  to: string[];
  cc: string[];
  date: string | null;
  flags: string[];
  text_body: string | null;
  html_body: string | null;
  message_id: string | null;
  in_reply_to: string | null;
  references: string[];
}> = {}) {
  return {
    uid: 1,
    subject: 'Project update',
    from: 'Alice <alice@example.com>',
    to: ['me@example.com'],
    cc: [],
    date: '2026-05-30T12:00:00Z',
    flags: ['\\Seen'],
    text_body: 'Hello there',
    html_body: null,
    attachments: [],
    message_id: '<orig-1@example.com>',
    in_reply_to: null,
    references: [],
    ...overrides,
  };
}

describe('stripSubjectPrefix', () => {
  it('strips a single Re: prefix case-insensitively', () => {
    expect(stripSubjectPrefix('Re: Hello')).toBe('Hello');
    expect(stripSubjectPrefix('RE: Hello')).toBe('Hello');
    expect(stripSubjectPrefix('re:Hello')).toBe('Hello');
  });

  it('strips Fwd:, FW:, Fwd: variants', () => {
    expect(stripSubjectPrefix('Fwd: Hi')).toBe('Hi');
    expect(stripSubjectPrefix('FW: Hi')).toBe('Hi');
    expect(stripSubjectPrefix('Fw:Hi')).toBe('Hi');
  });

  it('strips nested Re: Re: chains', () => {
    expect(stripSubjectPrefix('Re: Re: Fwd: Hello')).toBe('Hello');
  });

  it('leaves an unprefixed subject untouched', () => {
    expect(stripSubjectPrefix('Project update')).toBe('Project update');
  });
});

describe('applySubjectPrefix', () => {
  it('prepends Re: when the subject has no prefix', () => {
    expect(applySubjectPrefix('Hello', 'Re')).toBe('Re: Hello');
  });

  it('avoids double-prefixing', () => {
    expect(applySubjectPrefix('Re: Hello', 'Re')).toBe('Re: Hello');
    expect(applySubjectPrefix('Fwd: Hello', 'Fwd')).toBe('Fwd: Hello');
  });

  it('switches between Re: and Fwd:', () => {
    expect(applySubjectPrefix('Re: Hello', 'Fwd')).toBe('Fwd: Hello');
    expect(applySubjectPrefix('Fwd: Hello', 'Re')).toBe('Re: Hello');
  });

  it('falls back to (no subject) for empty input', () => {
    expect(applySubjectPrefix('', 'Re')).toBe('Re: (no subject)');
    expect(applySubjectPrefix('   ', 'Re')).toBe('Re: (no subject)');
  });
});

describe('extractAddress', () => {
  it('pulls the bare address out of a display-name wrapper', () => {
    expect(extractAddress('Alice <alice@example.com>')).toBe('alice@example.com');
    expect(extractAddress('"Bob B." <bob@x.io>')).toBe('bob@x.io');
  });

  it('lower-cases the result so equality checks are stable', () => {
    expect(extractAddress('CAROL@Example.com')).toBe('carol@example.com');
  });
});

describe('buildReferences', () => {
  it('appends the source Message-Id to the existing References chain', () => {
    const msg = makeMessage({
      message_id: '<orig-1@example.com>',
      references: ['<thread-root@example.com>', '<prev@example.com>'],
    });
    expect(buildReferences(msg)).toEqual([
      '<thread-root@example.com>',
      '<prev@example.com>',
      '<orig-1@example.com>',
    ]);
  });

  it('uses just the Message-Id when the source has no existing References', () => {
    const msg = makeMessage({ references: [], message_id: '<orig-1@example.com>' });
    expect(buildReferences(msg)).toEqual(['<orig-1@example.com>']);
  });

  it('does not duplicate the Message-Id if the source already lists itself in References', () => {
    const msg = makeMessage({
      message_id: '<orig-1@example.com>',
      references: ['<thread-root@example.com>', '<orig-1@example.com>'],
    });
    expect(buildReferences(msg)).toEqual([
      '<thread-root@example.com>',
      '<orig-1@example.com>',
    ]);
  });

  it('drops blank entries defensively', () => {
    const msg = makeMessage({
      message_id: '<orig-1@example.com>',
      references: ['', '  ', '<thread@example.com>'],
    });
    expect(buildReferences(msg)).toEqual([
      '<thread@example.com>',
      '<orig-1@example.com>',
    ]);
  });
});

describe('quoteBody', () => {
  it('formats the attribution line with the source from + date', () => {
    const msg = makeMessage({
      from: 'Alice <alice@example.com>',
      date: '2026-05-30T12:00:00Z',
      text_body: 'Line one\nLine two',
    });
    const body = quoteBody(msg);
    expect(body).toMatch(/Alice <alice@example.com> wrote:/);
    expect(body).toMatch(/^> Line one$/m);
    expect(body).toMatch(/^> Line two$/m);
  });

  it('falls back to HTML body when text body is empty', () => {
    const msg = makeMessage({
      text_body: null,
      html_body: '<p>Hello</p><p>World &amp; Co.</p>',
    });
    const body = quoteBody(msg);
    expect(body).toMatch(/> Hello/);
    expect(body).toMatch(/> World & Co\./);
  });

  it('handles an unknown date gracefully', () => {
    const msg = makeMessage({ date: null });
    const body = quoteBody(msg);
    expect(body).toMatch(/an earlier date/);
  });
});

describe('buildReplyContext - reply', () => {
  it('addresses the reply to the original sender only', () => {
    const ctx = buildReplyContext(
      makeMessage({ from: 'Alice <alice@example.com>', to: ['me@example.com', 'bob@example.com'], cc: ['carol@example.com'] }),
      'reply',
      'me@example.com',
    );
    expect(ctx.kind).toBe('reply');
    expect(ctx.to).toEqual(['Alice <alice@example.com>']);
    expect(ctx.cc).toEqual([]);
    expect(ctx.subject).toBe('Re: Project update');
  });

  it('sets In-Reply-To to the source Message-Id and propagates the References chain', () => {
    const ctx = buildReplyContext(
      makeMessage({
        message_id: '<orig-1@example.com>',
        references: ['<thread-root@example.com>'],
      }),
      'reply',
      'me@example.com',
    );
    expect(ctx.inReplyTo).toBe('<orig-1@example.com>');
    expect(ctx.references).toEqual(['<thread-root@example.com>', '<orig-1@example.com>']);
  });

  it('quotes the source body inside the prefill', () => {
    const ctx = buildReplyContext(
      makeMessage({ text_body: 'Original body line' }),
      'reply',
      'me@example.com',
    );
    expect(ctx.body).toMatch(/> Original body line/);
  });
});

describe('buildReplyContext - replyAll', () => {
  it('keeps the original from + to + cc, minus the logged-in user', () => {
    const ctx = buildReplyContext(
      makeMessage({
        from: 'Alice <alice@example.com>',
        to: ['me@example.com', 'Bob <bob@example.com>'],
        cc: ['carol@example.com', 'Me Again <me@example.com>'],
      }),
      'replyAll',
      'me@example.com',
    );
    expect(ctx.to).toEqual(['Alice <alice@example.com>', 'Bob <bob@example.com>']);
    expect(ctx.cc).toEqual(['carol@example.com']);
  });

  it('does not double-add the original sender into Cc', () => {
    const ctx = buildReplyContext(
      makeMessage({
        from: 'Alice <alice@example.com>',
        to: ['me@example.com'],
        cc: ['alice@example.com', 'Carol <carol@example.com>'],
      }),
      'replyAll',
      'me@example.com',
    );
    expect(ctx.to).toEqual(['Alice <alice@example.com>']);
    expect(ctx.cc).toEqual(['Carol <carol@example.com>']);
  });

  it('sets In-Reply-To and references identically to reply', () => {
    const ctx = buildReplyContext(
      makeMessage({
        message_id: '<orig-1@example.com>',
        references: ['<root@example.com>'],
      }),
      'replyAll',
      'me@example.com',
    );
    expect(ctx.inReplyTo).toBe('<orig-1@example.com>');
    expect(ctx.references).toEqual(['<root@example.com>', '<orig-1@example.com>']);
  });
});

describe('buildReplyContext - forward', () => {
  it('clears recipients (user picks them) and uses the Fwd: prefix', () => {
    const ctx = buildReplyContext(
      makeMessage({ subject: 'Project update', to: ['me@example.com'] }),
      'forward',
      'me@example.com',
    );
    expect(ctx.kind).toBe('forward');
    expect(ctx.to).toEqual([]);
    expect(ctx.cc).toEqual([]);
    expect(ctx.subject).toBe('Fwd: Project update');
  });

  it('still carries the source threading headers so re-attachment works', () => {
    const ctx = buildReplyContext(
      makeMessage({
        message_id: '<orig-1@example.com>',
        references: ['<root@example.com>'],
      }),
      'forward',
      'me@example.com',
    );
    expect(ctx.inReplyTo).toBe('<orig-1@example.com>');
    expect(ctx.references).toEqual(['<root@example.com>', '<orig-1@example.com>']);
  });

  it('quotes the source body inside the forwarded prefill', () => {
    const ctx = buildReplyContext(
      makeMessage({ text_body: 'Forward me please' }),
      'forward',
      'me@example.com',
    );
    expect(ctx.body).toMatch(/> Forward me please/);
  });
});
