import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { VacationResponder } from './VacationResponder';

const mockGet = vi.fn();
const mockSet = vi.fn();

vi.mock('../../api/auto-reply', () => ({
  autoReplyApi: {
    get: () => mockGet(),
    set: (...args: unknown[]) => mockSet(...args),
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

describe('VacationResponder', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading and enable checkbox', async () => {
    mockGet.mockResolvedValue(null);
    render(<VacationResponder />, { wrapper: createWrapper() });

    expect(screen.getByText('Vacation Responder')).toBeInTheDocument();
    expect(screen.getByText('Enable vacation responder')).toBeInTheDocument();
  });

  it('renders subject field with default value', async () => {
    mockGet.mockResolvedValue(null);
    render(<VacationResponder />, { wrapper: createWrapper() });

    const subjectInput = screen.getByDisplayValue('Out of Office');
    expect(subjectInput).toBeInTheDocument();
  });

  it('renders save button', async () => {
    mockGet.mockResolvedValue(null);
    render(<VacationResponder />, { wrapper: createWrapper() });

    expect(screen.getByText('Save Settings')).toBeInTheDocument();
  });

  it('renders mailing list and reply-to-all checkboxes', async () => {
    mockGet.mockResolvedValue(null);
    render(<VacationResponder />, { wrapper: createWrapper() });

    expect(screen.getByText('Skip mailing lists')).toBeInTheDocument();
    expect(screen.getByText('Reply to all recipients')).toBeInTheDocument();
  });

  it('populates form when existing rule loads', async () => {
    mockGet.mockResolvedValue({
      id: 'rule-1',
      mailbox_id: 'mb-1',
      enabled: true,
      subject: 'On Holiday',
      body_text: 'I am away until Monday',
      body_html: null,
      start_date: '2026-04-15T00:00:00Z',
      end_date: '2026-04-22T00:00:00Z',
      reply_to_all: true,
      exclude_lists: false,
      created_at: '2026-04-01T00:00:00Z',
      updated_at: '2026-04-01T00:00:00Z',
    });
    render(<VacationResponder />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByDisplayValue('On Holiday')).toBeInTheDocument();
    });
  });

  it('has date range fields', () => {
    mockGet.mockResolvedValue(null);
    render(<VacationResponder />, { wrapper: createWrapper() });

    expect(screen.getByText('Start date (optional)')).toBeInTheDocument();
    expect(screen.getByText('End date (optional)')).toBeInTheDocument();
  });

  it('has message textarea', () => {
    mockGet.mockResolvedValue(null);
    render(<VacationResponder />, { wrapper: createWrapper() });

    expect(screen.getByPlaceholderText("I'm currently out of the office...")).toBeInTheDocument();
  });
});
