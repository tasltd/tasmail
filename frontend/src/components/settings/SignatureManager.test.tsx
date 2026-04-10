import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SignatureManager } from './SignatureManager';

const mockFetchSignatures = vi.fn();
const mockCreateSignature = vi.fn();
const mockUpdateSignature = vi.fn();
const mockDeleteSignature = vi.fn();

vi.mock('../../api/signatures', () => ({
  fetchSignatures: () => mockFetchSignatures(),
  createSignature: (...args: unknown[]) => mockCreateSignature(...args),
  updateSignature: (...args: unknown[]) => mockUpdateSignature(...args),
  deleteSignature: (...args: unknown[]) => mockDeleteSignature(...args),
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

describe('SignatureManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading skeleton while fetching', () => {
    mockFetchSignatures.mockReturnValue(new Promise(() => {})); // never resolves
    render(<SignatureManager />, { wrapper: createWrapper() });
    // LoadingSkeleton renders placeholder divs
    expect(screen.queryByText('Email Signatures')).not.toBeInTheDocument();
  });

  it('renders header and New Signature button', async () => {
    mockFetchSignatures.mockResolvedValue([]);
    render(<SignatureManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Email Signatures')).toBeInTheDocument();
    });
    expect(screen.getByText('New Signature')).toBeInTheDocument();
  });

  it('renders empty state message when no signatures', async () => {
    mockFetchSignatures.mockResolvedValue([]);
    render(<SignatureManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No signatures yet. Create one to get started.')).toBeInTheDocument();
    });
  });

  it('renders list of signatures', async () => {
    mockFetchSignatures.mockResolvedValue([
      { id: '1', name: 'Work', html_body: '<p>Best,</p>', text_body: 'Best regards, John', is_default: true },
      { id: '2', name: 'Personal', html_body: '<p>Cheers</p>', text_body: 'Cheers, John', is_default: false },
    ]);
    render(<SignatureManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Work')).toBeInTheDocument();
      expect(screen.getByText('Personal')).toBeInTheDocument();
    });
    expect(screen.getByText('Default')).toBeInTheDocument();
  });

  it('shows create form when New Signature is clicked', async () => {
    mockFetchSignatures.mockResolvedValue([]);
    render(<SignatureManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('New Signature')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('New Signature'));

    expect(screen.getByText('New Signature', { selector: 'h3' })).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Signature name')).toBeInTheDocument();
  });

  it('navigates back when back button is clicked', async () => {
    mockFetchSignatures.mockResolvedValue([]);
    render(<SignatureManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });

  it('shows truncated preview of signature text', async () => {
    const longText = 'A'.repeat(100);
    mockFetchSignatures.mockResolvedValue([
      { id: '1', name: 'Long Sig', html_body: '', text_body: longText, is_default: false },
    ]);
    render(<SignatureManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Long Sig')).toBeInTheDocument();
    });
    // Should show truncated text with ellipsis
    const preview = screen.getByText(/A{80}\.\.\./);
    expect(preview).toBeInTheDocument();
  });
});
