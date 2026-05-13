// TMAIL-204: push notification device manager.
//
// Lists every push device the user has registered (mobile devices come from
// the Flutter app; web subscriptions come later when VAPID lands). Lets the
// user unregister a device and fire a test notification.
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Bell, Smartphone, Trash2, Send } from 'lucide-react';
import { pushApi, type PushDevice, type PushPlatform, type TestNotificationResponse } from '../../api/push';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

function platformLabel(p: PushPlatform): string {
  switch (p) {
    case 'fcm': return 'Android (FCM)';
    case 'apns': return 'iOS (APNs)';
    case 'web': return 'Web browser';
  }
}

function formatDate(iso: string | null): string {
  if (!iso) return '—';
  return new Date(iso).toLocaleString();
}

export function PushDevicesManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [testResult, setTestResult] = useState<string | null>(null);

  const devicesQuery = useQuery<PushDevice[]>({
    queryKey: ['push-devices'],
    queryFn: () => pushApi.list(),
  });

  const unregisterMut = useMutation({
    mutationFn: (id: string) => pushApi.unregister(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['push-devices'] }),
  });

  const testMut = useMutation<TestNotificationResponse>({
    mutationFn: () => pushApi.test(),
    onSuccess: (res) => {
      setTestResult(
        `Sent test to ${res.devices_notified} device${res.devices_notified === 1 ? '' : 's'} — ` +
        `${res.successes} delivered, ${res.failures} failed.`
      );
    },
    onError: (err: Error) => {
      setTestResult(`Test failed: ${err.message || 'unknown error'}`);
    },
  });

  return (
    <div className="settings-manager">
      <div className="settings-manager__header">
        <button className="btn btn--ghost btn--sm" onClick={() => setViewMode('list')}>
          <ArrowLeft size={16} /> Back
        </button>
        <h2><Bell size={20} style={{ verticalAlign: 'middle', marginRight: 6 }} />Notifications</h2>
      </div>

      <p className="settings-manager__subtitle" style={{ color: 'var(--color-text-secondary)', marginTop: 0 }}>
        Mobile devices registered via the TASMail Flutter app receive push
        notifications for new mail. Web browser push notifications require
        VAPID keys on the backend (not yet enabled — track via the backend
        push service note).
      </p>

      <div style={{ margin: '16px 0' }}>
        <button
          className="btn btn--primary"
          onClick={() => {
            setTestResult(null);
            testMut.mutate();
          }}
          disabled={testMut.isPending || (devicesQuery.data?.length ?? 0) === 0}
        >
          <Send size={16} />
          {testMut.isPending ? 'Sending…' : 'Send test notification'}
        </button>
        {testResult && (
          <div
            role="status"
            style={{
              marginTop: 8,
              padding: '8px 12px',
              background: 'var(--color-bg-elevated, #f8fafc)',
              border: '1px solid var(--color-border, #e5e7eb)',
              borderRadius: 6,
              fontSize: 13,
            }}
          >
            {testResult}
          </div>
        )}
      </div>

      {devicesQuery.isLoading && <LoadingSkeleton />}
      {devicesQuery.isError && (
        <div role="alert" className="settings-manager__error">
          Couldn't load devices: {(devicesQuery.error as Error)?.message ?? 'unknown error'}
        </div>
      )}
      {devicesQuery.data && devicesQuery.data.length === 0 && (
        <div className="settings-manager__empty" style={{ padding: '24px', textAlign: 'center', color: 'var(--color-text-secondary)' }}>
          <Smartphone size={32} style={{ opacity: 0.5, marginBottom: 8 }} />
          <p>No devices registered yet.</p>
          <p style={{ fontSize: 13 }}>Install the TASMail mobile app and sign in to register a device.</p>
        </div>
      )}
      {devicesQuery.data && devicesQuery.data.length > 0 && (
        <table className="settings-table" style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead>
            <tr>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Device</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Platform</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>App version</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Registered</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Last notified</th>
              <th style={{ textAlign: 'right', padding: '8px 12px' }}></th>
            </tr>
          </thead>
          <tbody>
            {devicesQuery.data.map((d) => (
              <tr key={d.id} style={{ borderTop: '1px solid var(--color-border, #e5e7eb)' }}>
                <td style={{ padding: '8px 12px' }}>{d.device_name || '(unnamed)'}</td>
                <td style={{ padding: '8px 12px' }}>{platformLabel(d.platform)}</td>
                <td style={{ padding: '8px 12px' }}>{d.app_version || '—'}</td>
                <td style={{ padding: '8px 12px' }}>{formatDate(d.created_at)}</td>
                <td style={{ padding: '8px 12px' }}>{formatDate(d.last_notified_at)}</td>
                <td style={{ padding: '8px 12px', textAlign: 'right' }}>
                  <button
                    className="btn btn--icon btn--danger"
                    onClick={() => {
                      if (confirm(`Unregister ${d.device_name || 'this device'}?`)) {
                        unregisterMut.mutate(d.id);
                      }
                    }}
                    disabled={unregisterMut.isPending}
                    title="Unregister"
                    aria-label={`Unregister ${d.device_name || 'device'}`}
                  >
                    <Trash2 size={16} />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
