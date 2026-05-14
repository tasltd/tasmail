// TMAIL-203: IP warm-up admin page.
//
// Two panels:
//  - Currently tracked IPs from /admin/warmup/status, with progress bars
//    against the daily limit pulled from the schedule.
//  - The static 8-week schedule from /admin/warmup/schedule.
//  - "Start tracking" form posts to /admin/warmup/start with an IP address.
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Activity, Play } from 'lucide-react';
import {
  adminWarmupApi,
  type WarmupStatus,
  type WarmupScheduleResponse,
} from '../../api/admin-warmup';

function pct(part: number, whole: number): number {
  if (whole <= 0) return 0;
  return Math.min(100, Math.round((part / whole) * 100));
}

export function WarmupManager() {
  const queryClient = useQueryClient();
  const [ip, setIp] = useState('');
  const [error, setError] = useState<string | null>(null);

  const status = useQuery<WarmupStatus[]>({
    queryKey: ['admin-warmup-status'],
    queryFn: () => adminWarmupApi.status(),
  });
  const schedule = useQuery<WarmupScheduleResponse>({
    queryKey: ['admin-warmup-schedule'],
    queryFn: () => adminWarmupApi.schedule(),
  });

  const startMut = useMutation({
    mutationFn: (addr: string) => adminWarmupApi.start(addr),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-warmup-status'] });
      setIp('');
      setError(null);
    },
    onError: (err: Error) => setError(err.message || 'Could not start tracking.'),
  });

  return (
    <div>
      <h1 style={{ display: 'flex', alignItems: 'center', gap: 8, margin: '0 0 16px' }}>
        <Activity size={22} /> IP warm-up
      </h1>
      <p style={{ color: 'var(--color-text-secondary, #64748b)', marginTop: 0 }}>
        Track new outbound IPs through the standard 8-week progression so
        receiving servers learn to trust the sender. Day 1 starts at a
        low daily limit and ramps until week 8 (unlimited).
      </p>

      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 15, marginBottom: 8 }}>Tracked IPs</h2>
        {status.isLoading && <p>Loading status…</p>}
        {status.data && status.data.length === 0 && (
          <p style={{ color: 'var(--color-text-secondary, #64748b)' }}>No IPs are being warmed up yet.</p>
        )}
        {status.data && status.data.length > 0 && (
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ background: 'var(--color-bg-elevated, #f8fafc)' }}>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>IP</th>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>Day</th>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>Today</th>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>Total sent</th>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>Started</th>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>State</th>
              </tr>
            </thead>
            <tbody>
              {status.data.map((row) => (
                <tr key={row.ip_address} style={{ borderTop: '1px solid var(--color-border, #e5e7eb)' }}>
                  <td style={{ padding: '8px 12px', fontFamily: 'monospace' }}>{row.ip_address}</td>
                  <td style={{ padding: '8px 12px' }}>Day {row.current_day} (week {row.current_week})</td>
                  <td style={{ padding: '8px 12px', minWidth: 200 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                      <span>{row.emails_sent_today} / {row.daily_limit || '∞'}</span>
                      <div style={{ flex: 1, height: 6, background: 'var(--color-border, #e5e7eb)', borderRadius: 3, overflow: 'hidden' }}>
                        <div style={{ width: `${pct(row.emails_sent_today, row.daily_limit || 1)}%`, height: '100%', background: 'var(--color-primary, #2563eb)' }} />
                      </div>
                    </div>
                  </td>
                  <td style={{ padding: '8px 12px' }}>{row.total_emails_sent.toLocaleString()}</td>
                  <td style={{ padding: '8px 12px', color: 'var(--color-text-secondary, #64748b)' }}>
                    {row.started_at ? new Date(row.started_at).toLocaleDateString() : '—'}
                  </td>
                  <td style={{ padding: '8px 12px' }}>
                    {row.completed ? <span style={{ color: '#22c55e' }}>Completed</span>
                      : row.paused ? <span style={{ color: '#f59e0b' }}>Paused</span>
                      : 'Active'}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 15, marginBottom: 8 }}>Start tracking a new IP</h2>
        <form
          onSubmit={(e) => { e.preventDefault(); if (ip.trim()) startMut.mutate(ip.trim()); }}
          style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}
        >
          <input
            type="text"
            value={ip}
            onChange={(e) => setIp(e.target.value)}
            placeholder="203.0.113.10 or 2001:db8::1"
            style={{ padding: '6px 10px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)', minWidth: 280 }}
          />
          <button type="submit" className="btn btn--primary" disabled={startMut.isPending || !ip.trim()}>
            <Play size={14} /> {startMut.isPending ? 'Starting…' : 'Start warm-up'}
          </button>
        </form>
        {error && <div role="alert" style={{ color: 'var(--color-danger, #dc2626)', fontSize: 13, marginTop: 8 }}>{error}</div>}
      </section>

      <section>
        <h2 style={{ fontSize: 15, marginBottom: 8 }}>Schedule</h2>
        {schedule.data && (
          <>
            <p style={{ color: 'var(--color-text-secondary, #64748b)', fontSize: 13 }}>{schedule.data.description}</p>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
              <thead>
                <tr style={{ background: 'var(--color-bg-elevated, #f8fafc)' }}>
                  <th style={{ textAlign: 'left', padding: '8px 12px' }}>Week</th>
                  <th style={{ textAlign: 'left', padding: '8px 12px' }}>Daily limit</th>
                  <th style={{ textAlign: 'left', padding: '8px 12px' }}>Notes</th>
                </tr>
              </thead>
              <tbody>
                {schedule.data.schedule.weeks.map((w) => (
                  <tr key={w.week} style={{ borderTop: '1px solid var(--color-border, #e5e7eb)' }}>
                    <td style={{ padding: '8px 12px' }}>Week {w.week}</td>
                    <td style={{ padding: '8px 12px', fontFamily: 'monospace' }}>{w.daily_limit === 0 ? '∞ (unlimited)' : w.daily_limit.toLocaleString()}</td>
                    <td style={{ padding: '8px 12px', color: 'var(--color-text-secondary, #64748b)' }}>{w.description}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </section>
    </div>
  );
}
