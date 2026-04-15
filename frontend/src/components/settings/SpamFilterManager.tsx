// Added: Rspamd spam filter management component for TMAIL-15
// PURPOSE: Tabbed interface for spam settings, quarantine management, and statistics
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, RefreshCw, Trash2, CheckCircle, ShieldBan } from 'lucide-react';
import {
  fetchSpamSettings,
  updateSpamSettings,
  fetchQuarantine,
  releaseQuarantine,
  deleteQuarantine,
  fetchSpamStats,
} from '../../api/spam';
import type { SpamQuarantineItem, SpamStats } from '../../api/spam';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Tab type for the three management sections
type SpamTab = 'settings' | 'quarantine' | 'statistics';

/**
 * PURPOSE: Badge showing spam action with appropriate color
 */
function ActionBadge({ action }: { action: SpamQuarantineItem['action'] }) {
  const actionColors: Record<string, string> = {
    reject: '#f44336',
    greylist: '#ff9800',
    add_header: '#2196f3',
    no_action: '#4caf50',
  };

  const displayLabel = action.replace('_', ' ');

  return (
    <span
      style={{
        fontSize: '11px',
        background: actionColors[action] || '#888',
        color: 'white',
        padding: '2px 8px',
        borderRadius: '10px',
        whiteSpace: 'nowrap',
        textTransform: 'capitalize',
      }}
    >
      {displayLabel}
    </span>
  );
}

/**
 * PURPOSE: Display aggregated spam statistics
 */
function StatsPanel({ stats }: { stats: SpamStats }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(150px, 1fr))',
          gap: '12px',
        }}
      >
        {/* Added: Stat cards for each metric */}
        <StatCard label="Total Scanned" value={stats.total_scanned} color="#2196f3" />
        <StatCard label="Blocked" value={stats.total_blocked} color="#f44336" />
        <StatCard label="Passed" value={stats.total_passed} color="#4caf50" />
        <StatCard label="Quarantined" value={stats.quarantined} color="#ff9800" />
        <StatCard label="Released" value={stats.released} color="#9c27b0" />
      </div>
    </div>
  );
}

function StatCard({ label, value, color }: { label: string; value: number; color: string }) {
  return (
    <div
      style={{
        padding: '16px',
        borderRadius: '8px',
        background: 'var(--color-bg-secondary, #f5f5f5)',
        textAlign: 'center',
      }}
    >
      <div style={{ fontSize: '24px', fontWeight: 'bold', color }}>{value.toLocaleString()}</div>
      <div style={{ fontSize: '12px', color: 'var(--color-text-secondary, #666)', marginTop: '4px' }}>{label}</div>
    </div>
  );
}

export function SpamFilterManager() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState<SpamTab>('settings');

  // Added: Fetch spam settings
  const settingsQuery = useQuery({
    queryKey: ['spam-settings'],
    queryFn: fetchSpamSettings,
  });

  // Added: Fetch quarantined messages
  const quarantineQuery = useQuery({
    queryKey: ['spam-quarantine'],
    queryFn: fetchQuarantine,
  });

  // Added: Fetch spam statistics
  const statsQuery = useQuery({
    queryKey: ['spam-stats'],
    queryFn: fetchSpamStats,
  });

  // Added: Settings update mutation
  const updateMutation = useMutation({
    mutationFn: updateSpamSettings,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['spam-settings'] }),
  });

  // Added: Release quarantined message mutation
  const releaseMutation = useMutation({
    mutationFn: releaseQuarantine,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['spam-quarantine'] });
      queryClient.invalidateQueries({ queryKey: ['spam-stats'] });
    },
  });

  // Added: Delete quarantined message mutation
  const deleteMutation = useMutation({
    mutationFn: deleteQuarantine,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['spam-quarantine'] });
      queryClient.invalidateQueries({ queryKey: ['spam-stats'] });
    },
  });

  // Added: Local state for settings form
  const [thresholdReject, setThresholdReject] = useState<number>(15);
  const [thresholdGreylist, setThresholdGreylist] = useState<number>(4);
  const [thresholdAddHeader, setThresholdAddHeader] = useState<number>(6);
  const [dkimEnabled, setDkimEnabled] = useState(true);
  const [arcEnabled, setArcEnabled] = useState(false);
  const [autolearnEnabled, setAutolearnEnabled] = useState(true);

  // Added: Sync local state when settings load
  const settings = settingsQuery.data;
  if (settings && thresholdReject === 15 && !updateMutation.isPending) {
    // NOTE: Only set once when data first arrives; avoids overwriting user edits
    if (settings.threshold_reject !== thresholdReject) {
      setThresholdReject(settings.threshold_reject);
      setThresholdGreylist(settings.threshold_greylist);
      setThresholdAddHeader(settings.threshold_add_header);
      setDkimEnabled(settings.dkim_signing_enabled);
      setArcEnabled(settings.arc_signing_enabled);
      setAutolearnEnabled(settings.autolearn_enabled);
    }
  }

  const isLoading = settingsQuery.isLoading || quarantineQuery.isLoading || statsQuery.isLoading;
  if (isLoading) return <LoadingSkeleton />;

  const handleSaveSettings = () => {
    updateMutation.mutate({
      threshold_reject: thresholdReject,
      threshold_greylist: thresholdGreylist,
      threshold_add_header: thresholdAddHeader,
      dkim_signing_enabled: dkimEnabled,
      arc_signing_enabled: arcEnabled,
      autolearn_enabled: autolearnEnabled,
    });
  };

  const handleRefresh = () => {
    queryClient.invalidateQueries({ queryKey: ['spam-settings'] });
    queryClient.invalidateQueries({ queryKey: ['spam-quarantine'] });
    queryClient.invalidateQueries({ queryKey: ['spam-stats'] });
  };

  return (
    <div style={{ padding: '24px', maxWidth: '900px' }}>
      {/* Added: Header with back button and refresh */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '20px' }}>
        <button title="Back" className="btn btn--icon" onClick={() => setViewMode('list')}>
          <ArrowLeft size={18} />
        </button>
        <ShieldBan size={24} />
        <h2 style={{ margin: 0, flex: 1 }}>Spam Filter</h2>
        <button className="btn btn--secondary" onClick={handleRefresh}>
          <RefreshCw size={14} />
          Refresh
        </button>
      </div>

      {/* Added: Tab navigation */}
      <div style={{ display: 'flex', gap: '8px', marginBottom: '20px', borderBottom: '1px solid var(--color-border)' }}>
        {(['settings', 'quarantine', 'statistics'] as SpamTab[]).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            style={{
              padding: '8px 16px',
              border: 'none',
              background: 'none',
              cursor: 'pointer',
              borderBottom: activeTab === tab ? '2px solid var(--color-primary, #1976d2)' : '2px solid transparent',
              fontWeight: activeTab === tab ? 600 : 400,
              textTransform: 'capitalize',
            }}
          >
            {tab}
          </button>
        ))}
      </div>

      {/* Added: Settings tab with threshold sliders and toggles */}
      {activeTab === 'settings' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
          <div>
            <label style={{ display: 'block', fontSize: '13px', marginBottom: '4px' }}>
              Reject Threshold: {thresholdReject}
            </label>
            <input
              type="range"
              min={1}
              max={30}
              step={0.5}
              value={thresholdReject}
              onChange={(e) => setThresholdReject(parseFloat(e.target.value))}
              style={{ width: '100%' }}
            />
          </div>
          <div>
            <label style={{ display: 'block', fontSize: '13px', marginBottom: '4px' }}>
              Greylist Threshold: {thresholdGreylist}
            </label>
            <input
              type="range"
              min={1}
              max={20}
              step={0.5}
              value={thresholdGreylist}
              onChange={(e) => setThresholdGreylist(parseFloat(e.target.value))}
              style={{ width: '100%' }}
            />
          </div>
          <div>
            <label style={{ display: 'block', fontSize: '13px', marginBottom: '4px' }}>
              Add Header Threshold: {thresholdAddHeader}
            </label>
            <input
              type="range"
              min={1}
              max={20}
              step={0.5}
              value={thresholdAddHeader}
              onChange={(e) => setThresholdAddHeader(parseFloat(e.target.value))}
              style={{ width: '100%' }}
            />
          </div>
          <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
            <input type="checkbox" checked={dkimEnabled} onChange={(e) => setDkimEnabled(e.target.checked)} />
            DKIM Signing
          </label>
          <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
            <input type="checkbox" checked={arcEnabled} onChange={(e) => setArcEnabled(e.target.checked)} />
            ARC Signing
          </label>
          <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
            <input type="checkbox" checked={autolearnEnabled} onChange={(e) => setAutolearnEnabled(e.target.checked)} />
            Autolearn
          </label>
          <button
            className="btn btn--primary"
            onClick={handleSaveSettings}
            disabled={updateMutation.isPending}
          >
            {updateMutation.isPending ? 'Saving...' : 'Save Settings'}
          </button>
          {updateMutation.isSuccess && (
            <p style={{ color: '#4caf50', fontSize: '13px' }}>Settings saved successfully.</p>
          )}
        </div>
      )}

      {/* Added: Quarantine tab with release/delete actions */}
      {activeTab === 'quarantine' && (
        <div>
          {(!quarantineQuery.data || quarantineQuery.data.length === 0) ? (
            <p style={{ color: 'var(--color-text-secondary, #666)' }}>No quarantined messages.</p>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              {quarantineQuery.data.map((item: SpamQuarantineItem) => (
                <div
                  key={item.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '12px',
                    padding: '12px',
                    background: 'var(--color-bg-secondary, #f5f5f5)',
                    borderRadius: '8px',
                  }}
                >
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontWeight: 500, fontSize: '14px' }}>
                      {item.subject || '(no subject)'}
                    </div>
                    <div style={{ fontSize: '12px', color: 'var(--color-text-secondary, #666)' }}>
                      From: {item.sender || 'unknown'} &middot; Score: {item.score}
                    </div>
                  </div>
                  <ActionBadge action={item.action} />
                  {!item.released && (
                    <>
                      <button
                        title="Release"
                        className="btn btn--icon"
                        onClick={() => releaseMutation.mutate(item.id)}
                        disabled={releaseMutation.isPending}
                      >
                        <CheckCircle size={16} />
                      </button>
                      <button
                        title="Delete"
                        className="btn btn--icon"
                        onClick={() => deleteMutation.mutate(item.id)}
                        disabled={deleteMutation.isPending}
                      >
                        <Trash2 size={16} />
                      </button>
                    </>
                  )}
                  {item.released && (
                    <span style={{ fontSize: '11px', color: '#4caf50' }}>Released</span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Added: Statistics tab */}
      {activeTab === 'statistics' && (
        <div>
          {statsQuery.data ? (
            <StatsPanel stats={statsQuery.data} />
          ) : (
            <p style={{ color: 'var(--color-text-secondary, #666)' }}>No statistics available.</p>
          )}
        </div>
      )}
    </div>
  );
}
