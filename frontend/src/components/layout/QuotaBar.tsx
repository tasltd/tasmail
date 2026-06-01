import { useQuery } from '@tanstack/react-query';
import { quotaApi } from '../../api/quota';
import type { QuotaStatus } from '../../api/quota';

// Fix (TMAIL-417): hardened against missing / NaN inputs so an unexpected
// /api/quota payload shape can't render "NaN undefined" in the sidebar.
function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.min(
    units.length - 1,
    Math.max(0, Math.floor(Math.log(bytes) / Math.log(1024))),
  );
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

  // Fix (TMAIL-417): coerce nullable/missing numeric fields so the sidebar
  // footer never renders "NaN undefined" when the backend (or an E2E mock)
  // returns a partial payload. If quota_bytes is unknown/zero, hide the bar
  // entirely rather than showing a meaningless 0-byte limit.
  const usedBytes = Number.isFinite(quota.used_bytes) ? quota.used_bytes : 0;
  const quotaBytes = Number.isFinite(quota.quota_bytes) ? quota.quota_bytes : 0;
  if (quotaBytes <= 0) return null;

  const usagePercent = Number.isFinite(quota.usage_percent) ? quota.usage_percent : 0;

  const barColor = quota.is_over_quota
    ? 'var(--color-error, #dc3545)'
    : quota.is_warning
      ? 'var(--color-warning, #ffc107)'
      : 'var(--color-primary, #4a90d9)';

  const percent = Math.min(Math.max(usagePercent, 0), 100);

  return (
    <div className="quota-bar" style={{ padding: '8px 12px', fontSize: '12px' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px', color: 'var(--color-text-secondary)' }}>
        <span>{formatBytes(usedBytes)} used</span>
        <span>{formatBytes(quotaBytes)}</span>
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
          {usagePercent.toFixed(0)}% of storage used
        </div>
      )}
    </div>
  );
}
