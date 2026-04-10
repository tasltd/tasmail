import { useQuery } from '@tanstack/react-query';
import { quotaApi } from '../../api/quota';
import type { QuotaStatus } from '../../api/quota';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

export function QuotaBar() {
  const { data: quota } = useQuery<QuotaStatus>({
    queryKey: ['quota'],
    queryFn: quotaApi.getQuota,
    refetchInterval: 5 * 60 * 1000, // Refresh every 5 minutes
    staleTime: 2 * 60 * 1000,
  });

  if (!quota) return null;

  const barColor = quota.is_over_quota
    ? 'var(--color-error, #dc3545)'
    : quota.is_warning
      ? 'var(--color-warning, #ffc107)'
      : 'var(--color-primary, #4a90d9)';

  const percent = Math.min(quota.usage_percent, 100);

  return (
    <div className="quota-bar" style={{ padding: '8px 12px', fontSize: '12px' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px', color: 'var(--color-text-secondary)' }}>
        <span>{formatBytes(quota.used_bytes)} used</span>
        <span>{formatBytes(quota.quota_bytes)}</span>
      </div>
      <div
        style={{
          height: '4px',
          background: 'var(--color-border, #e0e0e0)',
          borderRadius: '2px',
          overflow: 'hidden',
        }}
      >
        <div
          style={{
            width: `${percent}%`,
            height: '100%',
            background: barColor,
            borderRadius: '2px',
            transition: 'width 0.3s ease',
          }}
        />
      </div>
      {quota.is_over_quota && (
        <div style={{ color: 'var(--color-error, #dc3545)', marginTop: '4px', fontWeight: 600 }}>
          Mailbox full — delete messages to free space
        </div>
      )}
      {quota.is_warning && !quota.is_over_quota && (
        <div style={{ color: 'var(--color-warning, #ffc107)', marginTop: '4px' }}>
          {quota.usage_percent.toFixed(0)}% of storage used
        </div>
      )}
    </div>
  );
}
