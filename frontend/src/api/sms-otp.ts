import { apiClient } from './client';

export interface SmsOtpStatus {
  enabled: boolean;
  phone_number: string | null;
  provider: string | null;
}

export interface EnrollSmsRequest {
  phone_number: string;
  provider?: 'hubtel' | 'africastalking';
}

export const smsOtpApi = {
  enroll: (data: EnrollSmsRequest) =>
    apiClient.post('/sms-otp/enroll', data),

  verify: (code: string) =>
    apiClient.post('/sms-otp/verify', { code }),

  disable: () => apiClient.delete('/sms-otp'),

  getStatus: () => apiClient.get<SmsOtpStatus>('/sms-otp/status'),

  resend: () => apiClient.post('/sms-otp/resend', {}),
};
