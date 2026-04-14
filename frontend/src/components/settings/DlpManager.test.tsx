// Added: DlpManager component tests for TMAIL-108

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { DlpManager } from './DlpManager';

const mockListDlpRules = vi.fn();
const mockCreateDlpRule = vi.fn();
const mockUpdateDlpRule = vi.fn();
const mockDeleteDlpRule = vi.fn();
const mockListDlpViolations = vi.fn();
const mockTestDlpScan = vi.fn();

vi.mock('../../api/dlp', () => ({
  listDlpRules: () => mockListDlpRules(),
  createDlpRule: (...args: unknown[]) => mockCreateDlpRule(...args),
  updateDlpRule: (...args: unknown[]) => mockUpdateDlpRule(...args),
  deleteDlpRule: (...args: unknown[]) => mockDeleteDlpRule(...args),
  listDlpViolations: (...args: unknown[]) => mockListDlpViolations(...args),
  testDlpScan: (...args: unknown[]) => mockTestDlpScan(...args),
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

describe('DlpManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders Data Loss Prevention heading after loading', async () => {
    mockListDlpRules.mockResolvedValue([]);
    render(<DlpManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Data Loss Prevention')).toBeInTheDocument();
    });
  });

  it('shows empty state when no DLP rules exist', async () => {
    mockListDlpRules.mockResolvedValue([]);
    render(<DlpManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No DLP rules configured. Add one to scan outgoing emails for sensitive data.'),
      ).toBeInTheDocument();
    });
  });

  it('renders rule list with name, severity badge, and action badge', async () => {
    mockListDlpRules.mockResolvedValue([
      {
        id: 'dlp-1',
        name: 'Credit Card Blocker',
        description: 'Blocks credit card numbers',
        pattern: '\\b\\d{4}[- ]?\\d{4}[- ]?\\d{4}[- ]?\\d{4}\\b',
        pattern_type: 'regex',
        action: 'block',
        severity: 'critical',
        apply_to_subject: true,
        apply_to_body: true,
        apply_to_attachments: false,
        active: true,
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
      {
        id: 'dlp-2',
        name: 'SSN Detector',
        description: null,
        pattern: '\\b\\d{3}-\\d{2}-\\d{4}\\b',
        pattern_type: 'regex',
        action: 'warn',
        severity: 'high',
        apply_to_subject: false,
        apply_to_body: true,
        apply_to_attachments: false,
        active: false,
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<DlpManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Credit Card Blocker')).toBeInTheDocument();
      expect(screen.getByText('SSN Detector')).toBeInTheDocument();
    });
    // NOTE: Check severity badges are rendered
    expect(screen.getByTestId('severity-critical')).toBeInTheDocument();
    expect(screen.getByTestId('severity-high')).toBeInTheDocument();
    // NOTE: Check active/inactive status
    expect(screen.getByText('Active')).toBeInTheDocument();
    expect(screen.getByText('Inactive')).toBeInTheDocument();
  });

  it('shows add rule form when Add Rule is clicked', async () => {
    mockListDlpRules.mockResolvedValue([]);
    render(<DlpManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Rule')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Rule'));

    expect(screen.getByText('New DLP Rule')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Credit Card Blocker')).toBeInTheDocument();
  });

  it('shows pattern type, action, and severity selects in create form', async () => {
    mockListDlpRules.mockResolvedValue([]);
    render(<DlpManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Rule')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Rule'));

    // NOTE: Check select options are rendered
    expect(screen.getByText('Regex')).toBeInTheDocument();
    expect(screen.getByText('Keyword')).toBeInTheDocument();
    expect(screen.getByText('Dictionary')).toBeInTheDocument();
    expect(screen.getByText('Block')).toBeInTheDocument();
    expect(screen.getByText('Quarantine')).toBeInTheDocument();
    expect(screen.getByText('Warn')).toBeInTheDocument();
    expect(screen.getByText('Log')).toBeInTheDocument();
  });

  it('renders delete and toggle buttons for each rule', async () => {
    mockListDlpRules.mockResolvedValue([
      {
        id: 'dlp-1',
        name: 'Rule A',
        description: null,
        pattern: 'test',
        pattern_type: 'keyword',
        action: 'warn',
        severity: 'low',
        apply_to_subject: true,
        apply_to_body: true,
        apply_to_attachments: false,
        active: true,
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
      {
        id: 'dlp-2',
        name: 'Rule B',
        description: null,
        pattern: 'test2',
        pattern_type: 'keyword',
        action: 'block',
        severity: 'critical',
        apply_to_subject: true,
        apply_to_body: true,
        apply_to_attachments: false,
        active: false,
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<DlpManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      const deleteButtons = screen.getAllByTitle('Delete');
      expect(deleteButtons).toHaveLength(2);
    });
    expect(screen.getByTestId('toggle-dlp-1')).toBeInTheDocument();
    expect(screen.getByTestId('toggle-dlp-2')).toBeInTheDocument();
  });

  it('switches to violations tab and shows empty state', async () => {
    mockListDlpRules.mockResolvedValue([]);
    mockListDlpViolations.mockResolvedValue([]);
    render(<DlpManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Data Loss Prevention')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Violations'));

    await waitFor(() => {
      expect(screen.getByText('No DLP violations recorded yet.')).toBeInTheDocument();
    });
  });

  it('switches to test scan tab and shows scan form', async () => {
    mockListDlpRules.mockResolvedValue([]);
    render(<DlpManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Data Loss Prevention')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Test Scan'));

    await waitFor(() => {
      expect(screen.getByTestId('test-scan-panel')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('Paste email body text to test against DLP rules...')).toBeInTheDocument();
      expect(screen.getByText('Run Scan')).toBeInTheDocument();
    });
  });
});
