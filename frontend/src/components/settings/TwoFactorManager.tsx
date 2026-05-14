import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Shield, ShieldCheck, ShieldOff, Copy, Check, Phone } from 'lucide-react';
import { twoFactorApi } from '../../api/two-factor';
import type { TwoFactorStatus, EnrollResponse } from '../../api/two-factor';
// TMAIL-209: SMS OTP as an alternate / supplementary 2FA factor.
import { smsOtpApi, type SmsOtpStatus } from '../../api/sms-otp';

export function TwoFactorManager() {
  const queryClient = useQueryClient();
  const [enrollData, setEnrollData] = useState<EnrollResponse | null>(null);
  const [verifyCode, setVerifyCode] = useState('');
  const [disableCode, setDisableCode] = useState('');
  const [error, setError] = useState('');
  const [copiedCodes, setCopiedCodes] = useState(false);

  const { data: status } = useQuery<TwoFactorStatus>({
    queryKey: ['2fa-status'],
    queryFn: twoFactorApi.getStatus,
  });

  const enrollMutation = useMutation({
    mutationFn: twoFactorApi.enroll,
    onSuccess: (data) => {
      setEnrollData(data);
      setError('');
    },
    onError: (err: Error) => setError(err.message),
  });

  const verifyMutation = useMutation({
    mutationFn: () => twoFactorApi.verify(verifyCode),
    onSuccess: () => {
      setEnrollData(null);
      setVerifyCode('');
      queryClient.invalidateQueries({ queryKey: ['2fa-status'] });
    },
    onError: (err: Error) => setError(err.message),
  });

  const disableMutation = useMutation({
    mutationFn: () => twoFactorApi.disable(disableCode),
    onSuccess: () => {
      setDisableCode('');
      queryClient.invalidateQueries({ queryKey: ['2fa-status'] });
    },
    onError: (err: Error) => setError(err.message),
  });

  const copyBackupCodes = () => {
    if (!enrollData) return;
    navigator.clipboard.writeText(enrollData.backup_codes.join('\n'));
    setCopiedCodes(true);
    setTimeout(() => setCopiedCodes(false), 2000);
  };

  return (
    <div style={{ padding: '24px', maxWidth: '600px' }}>
      <h2 style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '16px' }}>
        <Shield size={24} />
        Two-Factor Authentication
      </h2>

      {error && (
        <div style={{ padding: '8px 12px', background: 'var(--color-error-bg, #ffeaea)', color: 'var(--color-error, #dc3545)', borderRadius: '4px', marginBottom: '12px' }}>
          {error}
        </div>
      )}

      {status?.enabled ? (
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '12px', background: 'var(--color-success-bg, #e8f5e9)', borderRadius: '8px', marginBottom: '16px' }}>
            <ShieldCheck size={20} color="var(--color-success, #28a745)" />
            <div>
              <strong>2FA is enabled</strong>
              <div style={{ fontSize: '13px', color: 'var(--color-text-secondary)' }}>
                {status.backup_codes_remaining} backup codes remaining
              </div>
            </div>
          </div>

          <div style={{ marginTop: '16px' }}>
            <h4>Disable 2FA</h4>
            <p style={{ fontSize: '13px', color: 'var(--color-text-secondary)', marginBottom: '8px' }}>
              Enter your current TOTP code to disable two-factor authentication.
            </p>
            <div style={{ display: 'flex', gap: '8px' }}>
              <input
                type="text"
                value={disableCode}
                onChange={(e) => setDisableCode(e.target.value)}
                placeholder="Enter 6-digit code"
                maxLength={6}
                style={{ width: '160px', padding: '8px 12px', fontSize: '16px', letterSpacing: '4px', textAlign: 'center' }}
              />
              <button
                className="btn btn--danger"
                onClick={() => disableMutation.mutate()}
                disabled={disableCode.length !== 6 || disableMutation.isPending}
              >
                <ShieldOff size={16} />
                Disable
              </button>
            </div>
          </div>
        </div>
      ) : enrollData ? (
        <div>
          <h3 style={{ marginBottom: '12px' }}>Step 1: Scan QR Code</h3>
          <p style={{ fontSize: '13px', color: 'var(--color-text-secondary)', marginBottom: '8px' }}>
            Scan this QR code with your authenticator app (Google Authenticator, Authy, etc.)
          </p>
          <div style={{ padding: '16px', background: '#fff', borderRadius: '8px', display: 'inline-block', marginBottom: '16px' }}>
            <img
              src={`https://api.qrserver.com/v1/create-qr-code/?size=200x200&data=${encodeURIComponent(enrollData.otpauth_url)}`}
              alt="TOTP QR Code"
              width={200}
              height={200}
            />
          </div>
          <div style={{ marginBottom: '16px' }}>
            <strong style={{ fontSize: '13px' }}>Manual entry key:</strong>
            <code style={{ display: 'block', padding: '8px', background: 'var(--color-bg-secondary)', borderRadius: '4px', fontFamily: 'monospace', fontSize: '14px', wordBreak: 'break-all', marginTop: '4px' }}>
              {enrollData.secret}
            </code>
          </div>

          <h3 style={{ marginBottom: '8px' }}>Step 2: Save Backup Codes</h3>
          <p style={{ fontSize: '13px', color: 'var(--color-text-secondary)', marginBottom: '8px' }}>
            Save these codes in a safe place. Each code can only be used once.
          </p>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: '4px', padding: '12px', background: 'var(--color-bg-secondary)', borderRadius: '8px', fontFamily: 'monospace', marginBottom: '8px' }}>
            {enrollData.backup_codes.map((code) => (
              <div key={code} style={{ padding: '4px 8px' }}>{code}</div>
            ))}
          </div>
          <button className="btn btn--secondary btn--sm" onClick={copyBackupCodes} style={{ marginBottom: '16px' }}>
            {copiedCodes ? <Check size={14} /> : <Copy size={14} />}
            {copiedCodes ? 'Copied!' : 'Copy Codes'}
          </button>

          <h3 style={{ marginBottom: '8px' }}>Step 3: Verify</h3>
          <p style={{ fontSize: '13px', color: 'var(--color-text-secondary)', marginBottom: '8px' }}>
            Enter the 6-digit code from your authenticator app to complete setup.
          </p>
          <div style={{ display: 'flex', gap: '8px' }}>
            <input
              type="text"
              value={verifyCode}
              onChange={(e) => setVerifyCode(e.target.value)}
              placeholder="000000"
              maxLength={6}
              style={{ width: '160px', padding: '8px 12px', fontSize: '16px', letterSpacing: '4px', textAlign: 'center' }}
            />
            <button
              className="btn btn--primary"
              onClick={() => verifyMutation.mutate()}
              disabled={verifyCode.length !== 6 || verifyMutation.isPending}
            >
              Verify & Enable
            </button>
          </div>
        </div>
      ) : (
        <div>
          <p style={{ marginBottom: '16px', color: 'var(--color-text-secondary)' }}>
            Add an extra layer of security to your account by enabling two-factor authentication with a TOTP authenticator app.
          </p>
          <button
            className="btn btn--primary"
            onClick={() => enrollMutation.mutate()}
            disabled={enrollMutation.isPending}
          >
            <Shield size={16} />
            {enrollMutation.isPending ? 'Setting up...' : 'Enable 2FA'}
          </button>
        </div>
      )}

      {/* TMAIL-209: SMS OTP as a supplementary 2FA factor — independent of TOTP. */}
      <SmsOtpSection />
    </div>
  );
}

// TMAIL-209 — SMS OTP enroll/verify/disable.
function SmsOtpSection() {
  const queryClient = useQueryClient();
  const [phone, setPhone] = useState('');
  const [provider, setProvider] = useState<'hubtel' | 'africastalking'>('hubtel');
  const [pendingCode, setPendingCode] = useState('');
  const [enrolledHint, setEnrolledHint] = useState<string | null>(null);
  const [error, setError] = useState('');

  const status = useQuery<SmsOtpStatus>({
    queryKey: ['sms-otp-status'],
    queryFn: () => smsOtpApi.getStatus(),
  });

  const enrollMut = useMutation({
    mutationFn: () => smsOtpApi.enroll({ phone_number: phone.trim(), provider }),
    onSuccess: (resp) => {
      setError('');
      // In TASMAIL_SMS_TEST_MODE the backend returns the OTP; pre-fill the
      // verify field so demo / E2E flows don't have to round-trip a phone.
      setEnrolledHint(resp.test_code ? `Test mode: code is ${resp.test_code}` : 'Enter the 6-digit code we just sent.');
      if (resp.test_code) setPendingCode(resp.test_code);
    },
    onError: (err: Error) => setError(err.message || 'Could not send SMS code.'),
  });

  const verifyMut = useMutation({
    mutationFn: (code: string) => smsOtpApi.verify(code),
    onSuccess: () => {
      setPendingCode('');
      setEnrolledHint(null);
      setPhone('');
      setError('');
      queryClient.invalidateQueries({ queryKey: ['sms-otp-status'] });
    },
    onError: (err: Error) => setError(err.message || 'Code did not verify.'),
  });

  const resendMut = useMutation({
    mutationFn: () => smsOtpApi.resend(),
    onSuccess: (resp) => {
      setError('');
      setEnrolledHint(resp.test_code ? `Test mode: code is ${resp.test_code}` : 'New code sent.');
      if (resp.test_code) setPendingCode(resp.test_code);
    },
    onError: (err: Error) => setError(err.message || 'Could not resend code.'),
  });

  const disableMut = useMutation({
    mutationFn: () => smsOtpApi.disable(),
    onSuccess: () => {
      setError('');
      queryClient.invalidateQueries({ queryKey: ['sms-otp-status'] });
    },
    onError: (err: Error) => setError(err.message || 'Could not disable SMS OTP.'),
  });

  return (
    <section style={{ marginTop: '32px', borderTop: '1px solid var(--color-border, #e5e7eb)', paddingTop: '24px' }}>
      <h2 style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '16px' }}>
        <Phone size={20} /> SMS one-time codes
      </h2>

      {error && (
        <div role="alert" style={{ padding: '8px 12px', background: 'var(--color-error-bg, #ffeaea)', color: 'var(--color-error, #dc3545)', borderRadius: '4px', marginBottom: '12px' }}>
          {error}
        </div>
      )}

      {status.data?.enabled ? (
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '12px', background: 'var(--color-success-bg, #e8f5e9)', borderRadius: '8px', marginBottom: '16px' }}>
            <ShieldCheck size={20} color="var(--color-success, #28a745)" />
            <div>
              <strong>SMS codes enabled</strong>
              <div style={{ fontSize: '13px', color: 'var(--color-text-secondary)' }}>
                Sending to <code>{status.data.phone_number ?? '(unknown)'}</code> via {status.data.provider ?? 'hubtel'}
              </div>
            </div>
          </div>
          <button
            className="btn btn--danger"
            onClick={() => { if (confirm('Turn off SMS OTP for this account?')) disableMut.mutate(); }}
            disabled={disableMut.isPending}
          >
            <ShieldOff size={16} /> Disable SMS OTP
          </button>
        </div>
      ) : enrolledHint ? (
        <div>
          <p style={{ fontSize: '13px', color: 'var(--color-text-secondary)', marginBottom: '8px' }}>{enrolledHint}</p>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
            <input
              type="text"
              value={pendingCode}
              onChange={(e) => setPendingCode(e.target.value)}
              placeholder="000000"
              maxLength={6}
              style={{ width: '160px', padding: '8px 12px', fontSize: '16px', letterSpacing: '4px', textAlign: 'center' }}
            />
            <button
              className="btn btn--primary"
              onClick={() => verifyMut.mutate(pendingCode)}
              disabled={pendingCode.length !== 6 || verifyMut.isPending}
            >
              {verifyMut.isPending ? 'Verifying…' : 'Verify & enable'}
            </button>
            <button
              className="btn btn--ghost"
              onClick={() => resendMut.mutate()}
              disabled={resendMut.isPending}
            >
              Resend code
            </button>
          </div>
        </div>
      ) : (
        <div>
          <p style={{ marginBottom: '12px', color: 'var(--color-text-secondary)' }}>
            Receive a 6-digit code by SMS at sign-in. Works alongside (or instead of) a TOTP app — enabling both gives you a fallback if you lose your phone.
          </p>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
            <input
              type="tel"
              value={phone}
              onChange={(e) => setPhone(e.target.value)}
              placeholder="+233 24 123 4567"
              style={{ width: '220px', padding: '8px 12px', fontSize: '14px' }}
            />
            <select
              value={provider}
              onChange={(e) => setProvider(e.target.value as 'hubtel' | 'africastalking')}
              style={{ padding: '8px 10px', borderRadius: 6 }}
            >
              <option value="hubtel">Hubtel</option>
              <option value="africastalking">Africa's Talking</option>
            </select>
            <button
              className="btn btn--primary"
              onClick={() => enrollMut.mutate()}
              disabled={enrollMut.isPending || phone.trim().length < 10}
            >
              <Phone size={16} /> {enrollMut.isPending ? 'Sending…' : 'Send code'}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
