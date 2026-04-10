import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ContactManager } from './ContactManager';

const mockFetchContacts = vi.fn();
const mockCreateContact = vi.fn();
const mockUpdateContact = vi.fn();
const mockDeleteContact = vi.fn();

vi.mock('../../api/contacts', () => ({
  fetchContacts: (...args: unknown[]) => mockFetchContacts(...args),
  createContact: (...args: unknown[]) => mockCreateContact(...args),
  updateContact: (...args: unknown[]) => mockUpdateContact(...args),
  deleteContact: (...args: unknown[]) => mockDeleteContact(...args),
}));

const mockSetViewMode = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setViewMode: mockSetViewMode }),
}));

function createWrapper() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  );
}

describe('ContactManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders header and Add Contact button', async () => {
    mockFetchContacts.mockResolvedValue([]);
    render(<ContactManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Contacts')).toBeInTheDocument();
    });
    expect(screen.getByText('Add Contact')).toBeInTheDocument();
  });

  it('renders empty state message when no contacts', async () => {
    mockFetchContacts.mockResolvedValue([]);
    render(<ContactManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No contacts yet. Add one to get started.')).toBeInTheDocument();
    });
  });

  it('renders contact list with display names and emails', async () => {
    mockFetchContacts.mockResolvedValue([
      { id: '1', email: 'alice@test.com', display_name: 'Alice Johnson', company: 'TAS', phone: null, notes: null },
      { id: '2', email: 'bob@test.com', display_name: null, company: null, phone: null, notes: null },
    ]);
    render(<ContactManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Alice Johnson')).toBeInTheDocument();
    });
    // bob@test.com appears as both name and email subtitle
    expect(screen.getAllByText('bob@test.com')).toHaveLength(2);
    // Shows company inline
    expect(screen.getByText(/alice@test.com · TAS/)).toBeInTheDocument();
  });

  it('shows avatar initial from display name', async () => {
    mockFetchContacts.mockResolvedValue([
      { id: '1', email: 'alice@test.com', display_name: 'Alice', company: null, phone: null, notes: null },
    ]);
    render(<ContactManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('A')).toBeInTheDocument();
    });
  });

  it('shows create form when Add Contact is clicked', async () => {
    mockFetchContacts.mockResolvedValue([]);
    render(<ContactManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Contact')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Contact'));
    expect(screen.getByText('New Contact')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('user@example.com')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Display name')).toBeInTheDocument();
  });

  it('has search input for filtering contacts', async () => {
    mockFetchContacts.mockResolvedValue([]);
    render(<ContactManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByPlaceholderText('Search contacts...')).toBeInTheDocument();
    });
  });

  it('shows search empty state when no results', async () => {
    mockFetchContacts.mockResolvedValue([]);
    render(<ContactManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByPlaceholderText('Search contacts...')).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText('Search contacts...'), {
      target: { value: 'nonexistent' },
    });

    // The query will re-fetch with the search term
    mockFetchContacts.mockResolvedValue([]);

    await waitFor(() => {
      expect(screen.getByText('No contacts match your search')).toBeInTheDocument();
    });
  });

  it('navigates back when back button is clicked', async () => {
    mockFetchContacts.mockResolvedValue([]);
    render(<ContactManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });
});
