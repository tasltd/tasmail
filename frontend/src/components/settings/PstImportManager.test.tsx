// Added: PST import manager component tests for TMAIL-115
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { PstImportManager } from './PstImportManager';
import type { PstImport } from '../../types/pst-import';

const mockList = vi.fn();
const mockUpload = vi.fn();
const mockDelete = vi.fn();

vi.mock('../../api/pst-import', () => ({
  pstImportApi: {
    list: (...args: unknown[]) => mockList(...args),
    upload: (...args: unknown[]) => mockUpload(...args),
    delete: (...args: unknown[]) => mockDelete(...args),
    get: vi.fn(),
  },
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe('PstImportManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders PST Import heading', async () => {
    mockList.mockResolvedValue([]);
    render(<PstImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('PST Import (Outlook)')).toBeInTheDocument();
    });
  });

  it('shows upload area with instructions', async () => {
    mockList.mockResolvedValue([]);
    render(<PstImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('Drag & drop a .pst file here, or click to select'),
      ).toBeInTheDocument();
    });
  });

  it('renders file input with .pst accept filter', async () => {
    mockList.mockResolvedValue([]);
    render(<PstImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      const fileInput = screen.getByTestId('pst-file-input');
      expect(fileInput).toBeInTheDocument();
      expect(fileInput).toHaveAttribute('accept', '.pst');
    });
  });

  it('shows target folder selector with INBOX default', async () => {
    mockList.mockResolvedValue([]);
    render(<PstImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      const folderSelect = screen.getByTestId('pst-target-folder');
      expect(folderSelect).toBeInTheDocument();
      expect(folderSelect).toHaveValue('INBOX');
    });
  });

  it('renders import history when imports exist', async () => {
    const mockImports: PstImport[] = [
      {
        id: 'imp-1',
        user_id: 'user-1',
        filename: 'outlook-backup.pst',
        file_size: 52428800,
        status: 'completed',
        target_folder: 'INBOX',
        messages_found: 200,
        messages_imported: 200,
        error_message: null,
        started_at: '2026-04-14T10:00:00Z',
        completed_at: '2026-04-14T10:05:00Z',
        created_at: '2026-04-14T09:59:00Z',
      },
    ];
    mockList.mockResolvedValue(mockImports);
    render(<PstImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('PST Import History')).toBeInTheDocument();
    });
    expect(screen.getByText('outlook-backup.pst')).toBeInTheDocument();
  });

  it('shows status badges on import cards', async () => {
    const mockImports: PstImport[] = [
      {
        id: 'imp-2',
        user_id: 'user-1',
        filename: 'mail.pst',
        file_size: 1024000,
        status: 'pending',
        target_folder: 'INBOX',
        messages_found: null,
        messages_imported: null,
        error_message: null,
        started_at: null,
        completed_at: null,
        created_at: '2026-04-14T10:00:00Z',
      },
    ];
    mockList.mockResolvedValue(mockImports);
    render(<PstImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('pending')).toBeInTheDocument();
    });
  });

  it('shows progress count for processing imports', async () => {
    const mockImports: PstImport[] = [
      {
        id: 'imp-3',
        user_id: 'user-1',
        filename: 'archive.pst',
        file_size: 10485760,
        status: 'processing',
        target_folder: 'Archive',
        messages_found: 500,
        messages_imported: 125,
        error_message: null,
        started_at: '2026-04-14T10:00:00Z',
        completed_at: null,
        created_at: '2026-04-14T09:59:00Z',
      },
    ];
    mockList.mockResolvedValue(mockImports);
    render(<PstImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('125/500 messages (25%)')).toBeInTheDocument();
    });
  });

  it('shows cancel button for pending imports', async () => {
    const mockImports: PstImport[] = [
      {
        id: 'imp-4',
        user_id: 'user-1',
        filename: 'old-mail.pst',
        file_size: 2048000,
        status: 'pending',
        target_folder: 'INBOX',
        messages_found: null,
        messages_imported: null,
        error_message: null,
        started_at: null,
        completed_at: null,
        created_at: '2026-04-14T10:00:00Z',
      },
    ];
    mockList.mockResolvedValue(mockImports);
    render(<PstImportManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Cancel import')).toBeInTheDocument();
    });
  });
});
