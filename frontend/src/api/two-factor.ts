import { apiClient } from './client';

export interface EnrollResponse {
  secret: string;
  otpauth_url: string;
  backup_codes: string[];
}

export interface TwoFactorStatus {
  enabled: boolean;
  verified_at: string | null;
  backup_codes_remaining: number;
}

export const twoFactorApi = {
  enroll: () => apiClient.post<EnrollResponse>('/2fa/enroll'),
  verify: (code: string) => apiClient.post<void>('/2fa/verify', { code }),
  disable: (code: string) => apiClient.delete<void>('/2fa', { code }),
  getStatus: () => apiClient.get<TwoFactorStatus>('/2fa/status'),
};
