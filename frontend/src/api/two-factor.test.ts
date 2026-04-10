import { describe, it, expect, vi, beforeEach } from 'vitest';
import { twoFactorApi } from './two-factor';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('two-factor API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('enrolls 2FA via POST /2fa/enroll', async () => {
    const mockResponse = {
      secret: 'JBSWY3DPEHPK3PXP',
      otpauth_url: 'otpauth://totp/TASMail:user@test.com?secret=JBSWY3DPEHPK3PXP',
      backup_codes: ['abc123', 'def456'],
    };
    vi.mocked(apiClient.post).mockResolvedValue(mockResponse);

    const result = await twoFactorApi.enroll();

    expect(apiClient.post).toHaveBeenCalledWith('/2fa/enroll');
    expect(result.secret).toBe('JBSWY3DPEHPK3PXP');
    expect(result.backup_codes).toHaveLength(2);
  });

  it('verifies 2FA code via POST /2fa/verify', async () => {
    vi.mocked(apiClient.post).mockResolvedValue(undefined);

    await twoFactorApi.verify('123456');

    expect(apiClient.post).toHaveBeenCalledWith('/2fa/verify', { code: '123456' });
  });

  it('disables 2FA via DELETE /2fa', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);

    await twoFactorApi.disable('123456');

    expect(apiClient.delete).toHaveBeenCalledWith('/2fa', { code: '123456' });
  });

  it('gets 2FA status via GET /2fa/status', async () => {
    const mockStatus = {
      enabled: true,
      verified_at: '2026-01-01T00:00:00Z',
      backup_codes_remaining: 8,
    };
    vi.mocked(apiClient.get).mockResolvedValue(mockStatus);

    const result = await twoFactorApi.getStatus();

    expect(apiClient.get).toHaveBeenCalledWith('/2fa/status');
    expect(result.enabled).toBe(true);
    expect(result.backup_codes_remaining).toBe(8);
  });
});
