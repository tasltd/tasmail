// TMAIL-166: admin dashboard for runtime feature flags.
// Lists every flag returned by GET /api/admin/feature-flags, lets the operator
// toggle enabled state, surfaces public/private status, and shows when the flag
// was last changed and by whom. Optimistically updates so the UI feels instant;
// reverts on PATCH failure.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Globe, Lock, RefreshCw } from 'lucide-react';
import { featureFlagsApi, type FeatureFlag } from '../../api/featureFlags';
import './FeatureFlagsManager.css';

export function FeatureFlagsManager() {
  const queryClient = useQueryClient();

  const { data: flags = [], isLoading, error, refetch } = useQuery<FeatureFlag[]>({
    queryKey: ['admin', 'feature-flags'],
    queryFn: featureFlagsApi.listAll,
    staleTime: 60_000,
  });

  const toggle = useMutation({
    mutationFn: ({ key, enabled }: { key: string; enabled: boolean }) =>
      featureFlagsApi.update(key, { enabled }),
    onMutate: async ({ key, enabled }) => {
      // Optimistic — snap the UI before the network call returns.
      await queryClient.cancelQueries({ queryKey: ['admin', 'feature-flags'] });
      const previous = queryClient.getQueryData<FeatureFlag[]>(['admin', 'feature-flags']);
      queryClient.setQueryData<FeatureFlag[]>(['admin', 'feature-flags'], (old) =>
        (old ?? []).map((f) => (f.key === key ? { ...f, enabled } : f))
      );
      return { previous };
    },
    onError: (_err, _vars, ctx) => {
      // Roll back on failure so the toggle reflects reality.
      if (ctx?.previous) queryClient.setQueryData(['admin', 'feature-flags'], ctx.previous);
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['admin', 'feature-flags'] }),
  });

  if (isLoading) {
    return <div className="ff-manager"><div className="ff-manager__loading">Loading feature flags…</div></div>;
  }

  if (error) {
    return (
      <div className="ff-manager">
        <div className="ff-manager__error">
          Could not load feature flags. {(error as Error).message}
          <button onClick={() => refetch()} className="ff-btn">Retry</button>
        </div>
      </div>
    );
  }

  return (
    <div className="ff-manager">
      <header className="ff-manager__header">
        <div>
          <h1>Feature flags</h1>
          <p>Runtime toggles for onboarding, signup, billing, and other operator-controlled product surfaces. Changes take effect within 60 seconds (cache TTL) for unauthenticated callers and immediately for authenticated ones.</p>
        </div>
        <button className="ff-btn ff-btn--ghost" onClick={() => refetch()} title="Refresh">
          <RefreshCw size={16} />
        </button>
      </header>

      <ul className="ff-list">
        {flags.map((flag) => (
          <li key={flag.key} className="ff-row">
            <div className="ff-row__main">
              <div className="ff-row__title">
                <span className="ff-row__name">{flag.name}</span>
                <code className="ff-row__key">{flag.key}</code>
                {flag.is_public ? (
                  <span className="ff-badge ff-badge--public" title="Visible to unauthenticated callers">
                    <Globe size={12} /> public
                  </span>
                ) : (
                  <span className="ff-badge ff-badge--private" title="Admin-only">
                    <Lock size={12} /> private
                  </span>
                )}
              </div>
              <p className="ff-row__desc">{flag.description}</p>
              {flag.updated_at && (
                <p className="ff-row__meta">
                  last changed {new Date(flag.updated_at).toLocaleString()}
                  {flag.updated_by ? ` by ${flag.updated_by.slice(0, 8)}…` : ''}
                </p>
              )}
            </div>

            <label className="ff-switch" aria-label={`Toggle ${flag.name}`}>
              <input
                type="checkbox"
                checked={flag.enabled}
                disabled={toggle.isPending}
                onChange={(e) => toggle.mutate({ key: flag.key, enabled: e.target.checked })}
              />
              <span className="ff-switch__slider" />
            </label>
          </li>
        ))}
      </ul>
    </div>
  );
}
