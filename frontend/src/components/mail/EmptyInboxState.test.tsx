// TMAIL-401: EmptyInboxState renders the user's IMAP address (or a
// fallback when no IMAP config exists yet).

import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { EmptyInboxState } from './EmptyInboxState';
import type { ImapConfig } from '../../api/byok';

function wrap(children: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

const baseConfig = (overrides: Partial<ImapConfig>): ImapConfig => ({
  id: 'cfg-1',
  name: 'Default',
  host: 'imap.example.com',
  port: 993,
  username: 'alice',
  encryption: 'ssl',
  is_default: true,
  verified: true,
  last_tested_at: null,
  last_error: null,
  ...overrides,
});

describe('EmptyInboxState', () => {
  it('renders the inbox-empty headline', () => {
    render(wrap(<EmptyInboxState defaultImapConfig={null} />));
    expect(screen.getByText('Your inbox is empty')).toBeInTheDocument();
  });

  it('renders the user@host pulled from the supplied IMAP config', () => {
    render(
      wrap(
        <EmptyInboxState
          defaultImapConfig={baseConfig({ username: 'dom', host: 'imap.mailbox.org' })}
        />,
      ),
    );
    const address = screen.getByTestId('empty-inbox-state__address');
    expect(address).toHaveTextContent('dom@imap.mailbox.org');
  });

  it('falls back to a Settings hint when no IMAP config is present', () => {
    render(wrap(<EmptyInboxState defaultImapConfig={null} />));
    expect(screen.queryByTestId('empty-inbox-state__address')).not.toBeInTheDocument();
    expect(
      screen.getByText(/Add an IMAP server in Settings/i),
    ).toBeInTheDocument();
  });

  it('exposes the empty-inbox-state container under a stable testid', () => {
    render(wrap(<EmptyInboxState defaultImapConfig={null} />));
    expect(screen.getByTestId('empty-inbox-state')).toBeInTheDocument();
  });
});
