import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { LowBandwidthSettings } from './LowBandwidthSettings';

const mockSetEnabled = vi.fn();
const mockSetAutoDetect = vi.fn();
const mockSetTextOnly = vi.fn();

vi.mock('../../hooks/useLowBandwidth', () => ({
  useLowBandwidthStore: () => ({
    enabled: false,
    autoDetect: true,
    textOnly: false,
    setEnabled: mockSetEnabled,
    setAutoDetect: mockSetAutoDetect,
    setTextOnly: mockSetTextOnly,
  }),
  isSlowConnection: () => false,
}));

describe('LowBandwidthSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders "Low-Bandwidth Mode" heading', () => {
    render(<LowBandwidthSettings />);
    expect(screen.getByText('Low-Bandwidth Mode')).toBeInTheDocument();
  });

  it('shows auto-detect checkbox', () => {
    render(<LowBandwidthSettings />);
    expect(screen.getByLabelText('Auto-detect slow connections')).toBeInTheDocument();
  });

  it('shows always-enable checkbox', () => {
    render(<LowBandwidthSettings />);
    expect(screen.getByLabelText('Always enable low-bandwidth mode')).toBeInTheDocument();
  });

  it('shows text-only emails checkbox', () => {
    render(<LowBandwidthSettings />);
    expect(screen.getByLabelText('Text-only emails')).toBeInTheDocument();
  });

  it('shows list of optimizations when active', () => {
    render(<LowBandwidthSettings />);
    expect(screen.getByText('When low-bandwidth mode is active:')).toBeInTheDocument();
    expect(screen.getByText('Inline images are not loaded automatically')).toBeInTheDocument();
    expect(screen.getByText('Attachment previews are disabled')).toBeInTheDocument();
    expect(screen.getByText('Emails show plain text instead of HTML (if text-only enabled)')).toBeInTheDocument();
    expect(screen.getByText('Page size reduced to 20 messages per page')).toBeInTheDocument();
    expect(screen.getByText('Offline cache TTL is extended for fewer network requests')).toBeInTheDocument();
  });
});
