import { describe, it, expect, vi, beforeEach } from 'vitest';
import { smsOtpApi } from './sms-otp';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('sms-otp API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('enrolls SMS OTP with phone number', async () => {
    vi.mocked(apiClient.post).mockResolvedValue({ message: 'OTP sent' });

    await smsOtpApi.enroll({ phone_number: '+233241234567', provider: 'hubtel' });

    expect(apiClient.post).toHaveBeenCalledWith('/sms-otp/enroll', {
      phone_number: '+233241234567',
      provider: 'hubtel',
    });
  });

  it('verifies SMS OTP code', async () => {
    vi.mocked(apiClient.post).mockResolvedValue(undefined);

    await smsOtpApi.verify('654321');

    expect(apiClient.post).toHaveBeenCalledWith('/sms-otp/verify', { code: '654321' });
  });

  it('disables SMS OTP', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);

    await smsOtpApi.disable();

    expect(apiClient.delete).toHaveBeenCalledWith('/sms-otp');
  });

  it('gets SMS OTP status', async () => {
    const mockStatus = { enabled: true, phone_number: '+233241234567', provider: 'hubtel' };
    vi.mocked(apiClient.get).mockResolvedValue(mockStatus);

    const result = await smsOtpApi.getStatus();

    expect(apiClient.get).toHaveBeenCalledWith('/sms-otp/status');
    expect(result.enabled).toBe(true);
  });

  it('resends OTP code', async () => {
    vi.mocked(apiClient.post).mockResolvedValue(undefined);

    await smsOtpApi.resend();

    expect(apiClient.post).toHaveBeenCalledWith('/sms-otp/resend', {});
  });
});
