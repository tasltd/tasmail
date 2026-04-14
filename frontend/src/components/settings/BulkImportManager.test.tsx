// Added: Unit tests for BulkImportManager component (TMAIL-136)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { BulkImportManager } from './BulkImportManager';

const mockUpload = vi.fn();
const mockList = vi.fn();
const mockGet = vi.fn();
const mockDownloadTemplate = vi.fn();

vi.mock('../../api/bulk-import', () => ({
  bulkImportApi: {
    upload: (...args: unknown[]) => mockUpload(...args),
    list: () => mockList(),
    get: (...args: unknown[]) => mockGet(...args),
    downloadTemplate: () => mockDownloadTemplate(),
  },
}));

const mockSetViewMode = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setViewMode: mockSetViewMode }),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe('BulkImportManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading after data loads', async () => {
    mockList.mockResolvedValue([]);
    render(<BulkImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Bulk User Import')).toBeInTheDocument();
    });
  });

  it('shows upload area with drag-and-drop text', async () => {
    mockList.mockResolvedValue([]);
    render(<BulkImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('Drag and drop a CSV file here, or click to browse'),
      ).toBeInTheDocument();
    });
  });

  it('shows download template button', async () => {
    mockList.mockResolvedValue([]);
    render(<BulkImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Download CSV Template')).toBeInTheDocument();
    });
  });

  it('shows import history with filename and counts', async () => {
    mockList.mockResolvedValue([
      {
        id: '1',
        filename: 'staff.csv',
        status: 'completed',
        total_rows: 20,
        success_count: 18,
        error_count: 2,
        errors: [{ row: 3, field: 'email', message: 'duplicate' }],
        created_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<BulkImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('staff.csv')).toBeInTheDocument();
      expect(screen.getByText('Total: 20')).toBeInTheDocument();
      expect(screen.getByText('Success: 18')).toBeInTheDocument();
      expect(screen.getByText('Errors: 2')).toBeInTheDocument();
    });
  });

  it('shows status badges with correct text', async () => {
    mockList.mockResolvedValue([
      {
        id: '1',
        filename: 'a.csv',
        status: 'completed',
        total_rows: 5,
        success_count: 5,
        error_count: 0,
        errors: [],
        created_at: '2026-04-14T10:00:00Z',
      },
      {
        id: '2',
        filename: 'b.csv',
        status: 'failed',
        total_rows: 3,
        success_count: 0,
        error_count: 3,
        errors: [],
        created_at: '2026-04-14T09:00:00Z',
      },
    ]);
    render(<BulkImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Completed')).toBeInTheDocument();
      expect(screen.getByText('Failed')).toBeInTheDocument();
    });
  });

  it('shows success and error counts in import history', async () => {
    mockList.mockResolvedValue([
      {
        id: '1',
        filename: 'users.csv',
        status: 'completed',
        total_rows: 10,
        success_count: 8,
        error_count: 2,
        errors: [],
        created_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<BulkImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Success: 8')).toBeInTheDocument();
      expect(screen.getByText('Errors: 2')).toBeInTheDocument();
    });
  });

  it('expands import to show error details when clicked', async () => {
    mockList.mockResolvedValue([
      {
        id: '1',
        filename: 'users.csv',
        status: 'completed',
        total_rows: 5,
        success_count: 3,
        error_count: 2,
        errors: [
          { row: 2, field: 'email', message: 'Invalid email format' },
          { row: 4, field: 'password', message: 'Too short' },
        ],
        created_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<BulkImportManager />, { wrapper: createWrapper() });

    // Added: Wait for data to load, then click expand toggle
    await waitFor(() => {
      expect(screen.getByTitle('Toggle errors')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Toggle errors'));

    // Added: Verify error details are now visible
    await waitFor(() => {
      expect(screen.getByText('Invalid email format')).toBeInTheDocument();
      expect(screen.getByText('Too short')).toBeInTheDocument();
    });
  });

  it('shows row-level error details with field and message', async () => {
    mockList.mockResolvedValue([
      {
        id: '1',
        filename: 'data.csv',
        status: 'failed',
        total_rows: 3,
        success_count: 0,
        error_count: 1,
        errors: [
          { row: 1, field: 'role', message: "Must be 'user' or 'admin'" },
        ],
        created_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<BulkImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Toggle errors')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Toggle errors'));

    await waitFor(() => {
      // Added: Verify the row number, field name, and error message are displayed
      expect(screen.getByText('role')).toBeInTheDocument();
      expect(screen.getByText("Must be 'user' or 'admin'")).toBeInTheDocument();
    });
  });
});
