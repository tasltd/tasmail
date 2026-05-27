// Added: TMAIL-119 — unit tests for the recipient autocomplete component.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { RecipientAutocomplete, splitRecipientTokens, formatContactToken } from './RecipientAutocomplete';
import * as contactsApi from '../../api/contacts';
import type { Contact } from '../../api/contacts';

vi.mock('../../api/contacts', () => ({
  fetchContacts: vi.fn(),
}));

const sampleContact = (overrides: Partial<Contact> = {}): Contact => ({
  id: 'c1',
  mailbox_id: 'm1',
  email: 'alice@example.com',
  display_name: 'Alice Smith',
  company: null,
  phone: null,
  notes: null,
  created_at: '',
  updated_at: '',
  ...overrides,
});

describe('splitRecipientTokens', () => {
  it('treats whole input as active when no comma', () => {
    expect(splitRecipientTokens('alic')).toEqual({ committed: '', active: 'alic' });
  });

  it('splits on last comma', () => {
    expect(splitRecipientTokens('bob@x.com, alic')).toEqual({ committed: 'bob@x.com,', active: ' alic' });
  });

  it('preserves multi-token committed prefix', () => {
    expect(splitRecipientTokens('a@x.com, b@x.com, c')).toEqual({
      committed: 'a@x.com, b@x.com,',
      active: ' c',
    });
  });

  it('handles empty input', () => {
    expect(splitRecipientTokens('')).toEqual({ committed: '', active: '' });
  });
});

describe('formatContactToken', () => {
  it('returns email-only when display name is missing', () => {
    expect(formatContactToken(sampleContact({ display_name: null }))).toBe('alice@example.com');
  });

  it('renders Name <email> when both present', () => {
    expect(formatContactToken(sampleContact())).toBe('Alice Smith <alice@example.com>');
  });

  it('quotes display name containing comma', () => {
    expect(formatContactToken(sampleContact({ display_name: 'Smith, Alice' }))).toBe(
      '"Smith, Alice" <alice@example.com>',
    );
  });

  it('returns email-only when display name is whitespace', () => {
    // Whitespace-only display names are treated as missing and stripped, so the
    // token is just the email — no stray "<name>" wrapping survives in the To: line.
    expect(formatContactToken(sampleContact({ display_name: '   ' }))).toBe('alice@example.com');
  });
});

describe('RecipientAutocomplete component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('does not fetch when query is under 2 chars', async () => {
    vi.mocked(contactsApi.fetchContacts).mockResolvedValue([]);
    const onChange = vi.fn();
    render(<RecipientAutocomplete value="a" onChange={onChange} />);
    // Wait beyond debounce
    await new Promise((r) => setTimeout(r, 250));
    expect(contactsApi.fetchContacts).not.toHaveBeenCalled();
  });

  it('fetches contacts with current token after debounce', async () => {
    vi.mocked(contactsApi.fetchContacts).mockResolvedValue([sampleContact()]);
    render(<RecipientAutocomplete value="alic" onChange={vi.fn()} />);
    await waitFor(() => expect(contactsApi.fetchContacts).toHaveBeenCalledWith('alic'), { timeout: 1000 });
  });

  it('only sends the active token (after last comma) as the query', async () => {
    vi.mocked(contactsApi.fetchContacts).mockResolvedValue([]);
    render(<RecipientAutocomplete value="bob@x.com, alic" onChange={vi.fn()} />);
    await waitFor(() => expect(contactsApi.fetchContacts).toHaveBeenCalledWith('alic'), { timeout: 1000 });
  });

  it('selecting a suggestion replaces the active token and appends comma', async () => {
    vi.mocked(contactsApi.fetchContacts).mockResolvedValue([sampleContact()]);
    const onChange = vi.fn();
    render(<RecipientAutocomplete value="alic" onChange={onChange} />);
    const item = await screen.findByRole('option', { name: /Alice Smith/ });
    fireEvent.mouseDown(item);
    expect(onChange).toHaveBeenCalledWith('Alice Smith <alice@example.com>, ');
  });

  it('selecting a suggestion preserves committed tokens', async () => {
    vi.mocked(contactsApi.fetchContacts).mockResolvedValue([sampleContact()]);
    const onChange = vi.fn();
    render(<RecipientAutocomplete value="bob@x.com, alic" onChange={onChange} />);
    const item = await screen.findByRole('option', { name: /Alice Smith/ });
    fireEvent.mouseDown(item);
    expect(onChange).toHaveBeenCalledWith('bob@x.com, Alice Smith <alice@example.com>, ');
  });

  it('Escape closes the suggestions list', async () => {
    vi.mocked(contactsApi.fetchContacts).mockResolvedValue([sampleContact()]);
    render(<RecipientAutocomplete value="alic" onChange={vi.fn()} inputId="t" />);
    await screen.findByRole('listbox');
    fireEvent.keyDown(screen.getByRole('combobox'), { key: 'Escape' });
    await waitFor(() => {
      expect(screen.queryByRole('listbox')).toBeNull();
    });
  });

  it('Enter selects the highlighted suggestion', async () => {
    vi.mocked(contactsApi.fetchContacts).mockResolvedValue([
      sampleContact(),
      sampleContact({ id: 'c2', email: 'aaron@example.com', display_name: 'Aaron' }),
    ]);
    const onChange = vi.fn();
    render(<RecipientAutocomplete value="al" onChange={onChange} />);
    await screen.findByRole('listbox');
    const input = screen.getByRole('combobox');
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onChange).toHaveBeenCalledWith('Aaron <aaron@example.com>, ');
  });
});
