// Added: ContactsApp component tests for TMAIL-119
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ContactsApp } from './ContactsApp';

const mockFetchContacts = vi.fn();
const mockListGroups = vi.fn();
const mockCreateGroup = vi.fn();
const mockDeleteGroup = vi.fn();
const mockListContactsInGroup = vi.fn();
const mockImportVcard = vi.fn();
const mockExportVcard = vi.fn();
const mockMergeContacts = vi.fn();
const mockAddContactToGroup = vi.fn();
const mockRemoveContactFromGroup = vi.fn();

vi.mock('../../api/contacts', () => ({
  fetchContacts: () => mockFetchContacts(),
}));

vi.mock('../../api/contact-groups', () => ({
  listContactGroups: () => mockListGroups(),
  createContactGroup: (...args: unknown[]) => mockCreateGroup(...args),
  deleteContactGroup: (...args: unknown[]) => mockDeleteGroup(...args),
  listContactsInGroup: (...args: unknown[]) => mockListContactsInGroup(...args),
  importVcard: (...args: unknown[]) => mockImportVcard(...args),
  exportVcard: () => mockExportVcard(),
  mergeContacts: (...args: unknown[]) => mockMergeContacts(...args),
  addContactToGroup: (...args: unknown[]) => mockAddContactToGroup(...args),
  removeContactFromGroup: (...args: unknown[]) => mockRemoveContactFromGroup(...args),
}));

function createWrapper() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  );
}

describe('ContactsApp', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading state', () => {
    mockFetchContacts.mockReturnValue(new Promise(() => {}));
    mockListGroups.mockReturnValue(new Promise(() => {}));
    render(<ContactsApp />, { wrapper: createWrapper() });
    expect(screen.getByText('Loading contacts...')).toBeInTheDocument();
  });

  it('renders header and All Contacts button', async () => {
    mockFetchContacts.mockResolvedValue([]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Contacts')).toBeInTheDocument();
    });
    expect(screen.getByText('All Contacts (0)')).toBeInTheDocument();
  });

  it('renders empty contacts message', async () => {
    mockFetchContacts.mockResolvedValue([]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No contacts found.')).toBeInTheDocument();
    });
  });

  it('renders contact list', async () => {
    mockFetchContacts.mockResolvedValue([
      { id: 'c1', email: 'alice@test.com', display_name: 'Alice', company: 'Acme', phone: null, notes: null, mailbox_id: 'u1', created_at: '', updated_at: '' },
      { id: 'c2', email: 'bob@test.com', display_name: 'Bob', company: null, phone: null, notes: null, mailbox_id: 'u1', created_at: '', updated_at: '' },
    ]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Alice')).toBeInTheDocument();
      expect(screen.getByText('Bob')).toBeInTheDocument();
    });
    expect(screen.getByText('Acme')).toBeInTheDocument();
  });

  it('renders group list', async () => {
    mockFetchContacts.mockResolvedValue([]);
    mockListGroups.mockResolvedValue([
      { id: 'g1', name: 'Work', color: '#ff0000', user_id: 'u1', created_at: '' },
      { id: 'g2', name: 'Friends', color: '#00ff00', user_id: 'u1', created_at: '' },
    ]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Work')).toBeInTheDocument();
      expect(screen.getByText('Friends')).toBeInTheDocument();
    });
  });

  it('shows New Group button and create form', async () => {
    mockFetchContacts.mockResolvedValue([]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('New Group')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('New Group'));
    expect(screen.getByPlaceholderText('Group name')).toBeInTheDocument();
    expect(screen.getByText('Create')).toBeInTheDocument();
  });

  it('shows import and export buttons', async () => {
    mockFetchContacts.mockResolvedValue([]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Import')).toBeInTheDocument();
      expect(screen.getByText('Export')).toBeInTheDocument();
    });
  });

  it('shows import dialog when Import is clicked', async () => {
    mockFetchContacts.mockResolvedValue([]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      // NOTE: there are two "Import" labels once the dialog opens (sidebar button + submit
      // button), so guard the click on the sidebar button by looking at all matches.
      expect(screen.getAllByText('Import').length).toBeGreaterThan(0);
    });

    fireEvent.click(screen.getAllByText('Import')[0]);
    // TMAIL-119: dialog now hosts both vCard and CSV tabs, default tab is vCard.
    expect(screen.getByText('Import Contacts')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Paste vCard text here (BEGIN:VCARD ... END:VCARD)')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /vCard/ })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('tab', { name: 'CSV' })).toHaveAttribute('aria-selected', 'false');
  });

  // Added: TMAIL-119 — switching to CSV swaps the textarea hint to match the format.
  it('switches placeholder when CSV tab is selected', async () => {
    mockFetchContacts.mockResolvedValue([]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getAllByText('Import').length).toBeGreaterThan(0);
    });

    fireEvent.click(screen.getAllByText('Import')[0]);
    fireEvent.click(screen.getByRole('tab', { name: 'CSV' }));
    expect(screen.getByRole('tab', { name: 'CSV' })).toHaveAttribute('aria-selected', 'true');
    expect(
      screen.getByPlaceholderText('Paste CSV here. First row: email,display_name,company,phone,notes'),
    ).toBeInTheDocument();
  });

  it('filters contacts with search input', async () => {
    mockFetchContacts.mockResolvedValue([
      { id: 'c1', email: 'alice@test.com', display_name: 'Alice', company: null, phone: null, notes: null, mailbox_id: 'u1', created_at: '', updated_at: '' },
      { id: 'c2', email: 'bob@test.com', display_name: 'Bob', company: null, phone: null, notes: null, mailbox_id: 'u1', created_at: '', updated_at: '' },
    ]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Alice')).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText('Search contacts...'), { target: { value: 'bob' } });

    expect(screen.queryByText('Alice')).not.toBeInTheDocument();
    expect(screen.getByText('Bob')).toBeInTheDocument();
  });

  it('shows contact detail when contact is clicked', async () => {
    mockFetchContacts.mockResolvedValue([
      { id: 'c1', email: 'alice@test.com', display_name: 'Alice Smith', company: 'Acme Corp', phone: '+233201234567', notes: 'VIP', mailbox_id: 'u1', created_at: '', updated_at: '' },
    ]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Alice Smith')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Alice Smith'));

    // NOTE: Detail view should show all contact fields
    await waitFor(() => {
      expect(screen.getByText('alice@test.com')).toBeInTheDocument();
      expect(screen.getByText('+233201234567')).toBeInTheDocument();
      expect(screen.getByText('Acme Corp')).toBeInTheDocument();
      expect(screen.getByText('VIP')).toBeInTheDocument();
    });
  });

  it('shows merge button when duplicates exist', async () => {
    mockFetchContacts.mockResolvedValue([
      { id: 'c1', email: 'dupe@test.com', display_name: 'Dupe 1', company: null, phone: null, notes: null, mailbox_id: 'u1', created_at: '', updated_at: '' },
      { id: 'c2', email: 'dupe@test.com', display_name: 'Dupe 2', company: null, phone: null, notes: null, mailbox_id: 'u1', created_at: '', updated_at: '' },
    ]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Merge (1)')).toBeInTheDocument();
    });
  });

  it('navigates back from detail to list', async () => {
    mockFetchContacts.mockResolvedValue([
      { id: 'c1', email: 'alice@test.com', display_name: 'Alice', company: null, phone: null, notes: null, mailbox_id: 'u1', created_at: '', updated_at: '' },
    ]);
    mockListGroups.mockResolvedValue([]);
    render(<ContactsApp />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Alice')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Alice'));

    await waitFor(() => {
      expect(screen.getByText('← Back to list')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('← Back to list'));

    await waitFor(() => {
      // NOTE: Should be back to list view showing all contacts
      expect(screen.getByText('All Contacts (1)')).toBeInTheDocument();
    });
  });
});
