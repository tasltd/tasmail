import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { GroupManager } from './GroupManager';

const mockList = vi.fn();
const mockCreate = vi.fn();
const mockDelete = vi.fn();
const mockListMembers = vi.fn();
const mockAddMember = vi.fn();
const mockRemoveMember = vi.fn();

vi.mock('../../api/groups', () => ({
  groupsApi: {
    list: () => mockList(),
    create: (...args: unknown[]) => mockCreate(...args),
    delete: (...args: unknown[]) => mockDelete(...args),
    listMembers: (...args: unknown[]) => mockListMembers(...args),
    addMember: (...args: unknown[]) => mockAddMember(...args),
    removeMember: (...args: unknown[]) => mockRemoveMember(...args),
  },
}));

function createWrapper() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  );
}

describe('GroupManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading state', () => {
    mockList.mockReturnValue(new Promise(() => {}));
    render(<GroupManager />, { wrapper: createWrapper() });
    expect(screen.getByText('Loading groups...')).toBeInTheDocument();
  });

  it('renders header and New Group button', async () => {
    mockList.mockResolvedValue([]);
    render(<GroupManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Distribution Groups')).toBeInTheDocument();
    });
    expect(screen.getByText('New Group')).toBeInTheDocument();
  });

  it('renders empty state', async () => {
    mockList.mockResolvedValue([]);
    render(<GroupManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No distribution groups yet. Create one to get started.')).toBeInTheDocument();
    });
  });

  it('renders group list', async () => {
    mockList.mockResolvedValue([
      { id: '1', name: 'Engineering', address: 'eng@test.com', description: 'Dev team', active: true },
      { id: '2', name: 'Marketing', address: 'mkt@test.com', description: null, active: true },
    ]);
    render(<GroupManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Engineering')).toBeInTheDocument();
      expect(screen.getByText('Marketing')).toBeInTheDocument();
    });
    expect(screen.getByText('eng@test.com')).toBeInTheDocument();
    expect(screen.getByText('Dev team')).toBeInTheDocument();
  });

  it('shows create form when New Group is clicked', async () => {
    mockList.mockResolvedValue([]);
    render(<GroupManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('New Group')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('New Group'));

    expect(screen.getByPlaceholderText('Engineering Team')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('engineering@example.com')).toBeInTheDocument();
    expect(screen.getByText('Create Group')).toBeInTheDocument();
  });

  it('shows inactive badge for inactive groups', async () => {
    mockList.mockResolvedValue([
      { id: '1', name: 'Old Team', address: 'old@test.com', description: null, active: false },
    ]);
    render(<GroupManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Inactive')).toBeInTheDocument();
    });
  });

  it('expands group to show members', async () => {
    mockList.mockResolvedValue([
      { id: 'g1', name: 'Team', address: 'team@test.com', description: null, active: true },
    ]);
    mockListMembers.mockResolvedValue([
      { id: 'm1', member_address: 'alice@test.com' },
      { id: 'm2', member_address: 'bob@test.com' },
    ]);
    render(<GroupManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Team')).toBeInTheDocument();
    });

    // Click to expand
    fireEvent.click(screen.getByText('Team'));

    await waitFor(() => {
      expect(screen.getByText('alice@test.com')).toBeInTheDocument();
      expect(screen.getByText('bob@test.com')).toBeInTheDocument();
    });
  });

  it('shows add member input when expanded', async () => {
    mockList.mockResolvedValue([
      { id: 'g1', name: 'Team', address: 'team@test.com', description: null, active: true },
    ]);
    mockListMembers.mockResolvedValue([]);
    render(<GroupManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Team')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Team'));

    await waitFor(() => {
      expect(screen.getByPlaceholderText('Add member email...')).toBeInTheDocument();
    });
  });
});
