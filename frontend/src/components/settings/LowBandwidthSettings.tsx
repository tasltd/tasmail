import { Gauge } from 'lucide-react';
import { useLowBandwidthStore, isSlowConnection } from '../../hooks/useLowBandwidth';

export function LowBandwidthSettings() {
  const { enabled, autoDetect, textOnly, setEnabled, setAutoDetect, setTextOnly } = useLowBandwidthStore();
  const currentlySlow = isSlowConnection();

  return (
    <div className="settings-panel">
      <div className="settings-panel__header">
        <h2><Gauge size={20} /> Low-Bandwidth Mode</h2>
      </div>

      <p style={{ fontSize: '13px', color: 'var(--color-text-secondary)', marginBottom: '16px' }}>
        Optimize TASMail for slow or metered connections. Reduces data usage by disabling images,
        compressing content, and using text-only email views.
      </p>

      {currentlySlow && (
        <div style={{
          background: 'var(--color-primary)', color: 'white', padding: '8px 12px',
          borderRadius: '6px', marginBottom: '12px', fontSize: '13px',
        }}>
          Slow connection detected — low-bandwidth mode recommended.
        </div>
      )}

      <div className="settings-form">
        <div className="form-group" style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <input
            type="checkbox"
            id="lb-auto"
            checked={autoDetect}
            onChange={(e) => setAutoDetect(e.target.checked)}
          />
          <div>
            <label htmlFor="lb-auto"><strong>Auto-detect slow connections</strong></label>
            <p style={{ fontSize: '12px', color: 'var(--color-text-secondary)', margin: 0 }}>
              Automatically enable when on 2G/slow-3G or when browser Save Data is on
            </p>
          </div>
        </div>

        <div className="form-group" style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <input
            type="checkbox"
            id="lb-enable"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
          />
          <div>
            <label htmlFor="lb-enable"><strong>Always enable low-bandwidth mode</strong></label>
            <p style={{ fontSize: '12px', color: 'var(--color-text-secondary)', margin: 0 }}>
              Force low-bandwidth optimizations regardless of connection speed
            </p>
          </div>
        </div>

        <div className="form-group" style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <input
            type="checkbox"
            id="lb-text"
            checked={textOnly}
            onChange={(e) => setTextOnly(e.target.checked)}
          />
          <div>
            <label htmlFor="lb-text"><strong>Text-only emails</strong></label>
            <p style={{ fontSize: '12px', color: 'var(--color-text-secondary)', margin: 0 }}>
              Show plain text version of emails instead of HTML (saves bandwidth)
            </p>
          </div>
        </div>
      </div>

      <div style={{ marginTop: '16px', padding: '12px', background: 'var(--color-bg)', borderRadius: '6px', fontSize: '13px' }}>
        <strong>When low-bandwidth mode is active:</strong>
        <ul style={{ paddingLeft: '20px', marginTop: '4px' }}>
          <li>Inline images are not loaded automatically</li>
          <li>Attachment previews are disabled</li>
          <li>Emails show plain text instead of HTML (if text-only enabled)</li>
          <li>Page size reduced to 20 messages per page</li>
          <li>Offline cache TTL is extended for fewer network requests</li>
        </ul>
      </div>
    </div>
  );
}
