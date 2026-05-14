// TMAIL-209: SMS OTP integration in TwoFactorManager.
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

export interface SmsEnrollResponse {
  sent: boolean;
  /** Only populated when the backend is started with TASMAIL_SMS_TEST_MODE=true.
   *  In production this field is omitted; the user reads the OTP off their phone. */
  test_code?: string;
}

export const smsOtpApi = {
  enroll: (data: EnrollSmsRequest) =>
    apiClient.post<SmsEnrollResponse>('/sms-otp/enroll', data),

  verify: (code: string) =>
    apiClient.post<void>('/sms-otp/verify', { code }),

  disable: () => apiClient.delete<void>('/sms-otp'),

  getStatus: () => apiClient.get<SmsOtpStatus>('/sms-otp/status'),

  resend: () => apiClient.post<SmsEnrollResponse>('/sms-otp/resend', {}),
};
