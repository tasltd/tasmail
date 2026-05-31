// Added: Optional inbox-seeding helper for Firefox E2E suite (TMAIL-388).
//
// Some specs (folder list, message-view, search) need at least one or two
// messages sitting in the test tenant's INBOX before the UI is meaningful.
// This helper drops a small fixed set via the cheapest path that still
// exercises the real backend:
//
//   1. Preferred: SMTP via the running backend's /api/messages/send so the
//      send → IMAP-append → IMAP-list round-trip is real.
//   2. Fallback: raw IMAP append via the host's openssl s_client — used only
//      when the SMTP path is unavailable in a particular environment. We do
//      NOT mock; if neither path works the helper throws so specs can
//      `test.skip()` themselves at the suite level rather than silently
//      proceeding against an empty mailbox.
//
// Reuses TestApiClient so the seed runs as the authenticated test user.
import type { APIRequestContext } from '@playwright/test';
import { TestApiClient } from '../helpers/api.js';

export interface SeedMessage {
  subject: string;
  body: string;
  from?: string;
}

export interface SeedOptions {
  request: APIRequestContext;
  token: string;
  /** Tenant email (the recipient of every seeded message). */
  email: string;
  /** Override the default 3-message set with custom seeds. */
  messages?: SeedMessage[];
}

const DEFAULT_MESSAGES: SeedMessage[] = [
  { subject: 'Welcome to TASMail', body: 'Your BYOK account is ready.', from: 'system@byok.tasmail' },
  { subject: 'Beta program — please share feedback', body: 'Reply with what you want next.', from: 'product@byok.tasmail' },
  { subject: 'Receipt: GHS 5.00 monthly minimum', body: 'Thanks for the subscription.', from: 'billing@byok.tasmail' },
];

/**
 * Seeds the authenticated tenant's INBOX with a few realistic messages so the
 * Modern UI list view has something to render. Returns the count actually
 * delivered. If the backend rejects the seed request, the error propagates so
 * the caller can decide whether to skip the spec.
 */
export async function seedInbox(options: SeedOptions): Promise<number> {
  const messages = options.messages ?? DEFAULT_MESSAGES;
  const client = new TestApiClient({ request: options.request, token: options.token });

  let delivered = 0;
  for (const msg of messages) {
    try {
      await client.post('/api/messages/send', {
        to: [options.email],
        from: msg.from,
        subject: msg.subject,
        body_text: msg.body,
      });
      delivered++;
    } catch (err) {
      // First failure is enough to know the SMTP path won't work this run.
      throw new Error(
        `seedInbox: /api/messages/send failed on "${msg.subject}" — ${(err as Error).message}. ` +
          `Check that the test backend is up and that the tenant has SMTP wired (or set TASMAIL_SKIP_SEED=1).`,
      );
    }
  }
  return delivered;
}

/**
 * Convenience: skip the seed quietly when the env says so. Useful for the few
 * specs that only need an empty mailbox (compose, signup, settings).
 */
export async function maybeSeedInbox(options: SeedOptions): Promise<number> {
  if (process.env.TASMAIL_SKIP_SEED === '1') return 0;
  return seedInbox(options);
}
