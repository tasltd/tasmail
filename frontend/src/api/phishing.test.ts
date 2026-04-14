// Added: Unit tests for phishing API functions (TMAIL-124)
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { getPhishingReport, scanMessage, updatePhishingAction } from './phishing';
import type { PhishingReport, ScanRequest } from './phishing';

// NOTE: Mock the apiClient module to intercept HTTP calls without a real backend
vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
  },
}));

import { apiClient } from './client';

const mockGet = vi.mocked(apiClient.get);
const mockPost = vi.mocked(apiClient.post);
const mockPut = vi.mocked(apiClient.put);

// Added: Sample phishing report used across multiple tests
const sampleReport: PhishingReport = {
  id: '550e8400-e29b-41d4-a716-446655440000',
  mailbox_id: '550e8400-e29b-41d4-a716-446655440001',
  message_uid: 42,
  folder: 'INBOX',
  suspicious_links: [
    { url: 'https://evil.com/steal', display_text: 'paypal.com', reasons: ['Mismatched display text'] },
  ],
  suspicious_sender: true,
  spoofed_display_name: true,
  risk_score: 75,
  user_action: 'none',
  created_at: '2026-04-14T10:00:00Z',
};

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('getPhishingReport', () => {
  it('calls GET with correct URL for folder and UID', async () => {
    mockGet.mockResolvedValue(sampleReport);

    const result = await getPhishingReport('INBOX', 42);

    expect(mockGet).toHaveBeenCalledWith('/folders/INBOX/messages/42/phishing');
    expect(result).toEqual(sampleReport);
  });

  it('returns null when message has not been scanned', async () => {
    mockGet.mockResolvedValue(null);

    const result = await getPhishingReport('INBOX', 99);

    expect(result).toBeNull();
  });

  it('encodes folder names with special characters', async () => {
    mockGet.mockResolvedValue(null);

    await getPhishingReport('Sent Items', 10);

    expect(mockGet).toHaveBeenCalledWith('/folders/Sent%20Items/messages/10/phishing');
  });
});

describe('scanMessage', () => {
  it('calls POST with scan request body and returns report', async () => {
    mockPost.mockResolvedValue(sampleReport);

    const scanRequest: ScanRequest = {
      html_body: '<a href="https://evil.com">paypal.com</a>',
      sender_display_name: 'PayPal',
      sender_email: 'scam@evil.com',
    };

    const result = await scanMessage('INBOX', 42, scanRequest);

    expect(mockPost).toHaveBeenCalledWith(
      '/folders/INBOX/messages/42/phishing/scan',
      scanRequest,
    );
    expect(result.risk_score).toBe(75);
    expect(result.suspicious_links).toHaveLength(1);
  });
});

describe('updatePhishingAction', () => {
  it('calls PUT with correct report ID and action', async () => {
    mockPut.mockResolvedValue(undefined);

    await updatePhishingAction('550e8400-e29b-41d4-a716-446655440000', {
      action: 'dismissed',
    });

    expect(mockPut).toHaveBeenCalledWith(
      '/phishing/550e8400-e29b-41d4-a716-446655440000/action',
      { action: 'dismissed' },
    );
  });

  it('supports all valid action types', async () => {
    mockPut.mockResolvedValue(undefined);

    const validActions = ['dismissed', 'reported', 'confirmed_safe'] as const;
    for (const action of validActions) {
      await updatePhishingAction('test-id', { action });
      expect(mockPut).toHaveBeenCalledWith('/phishing/test-id/action', { action });
    }
  });
});
