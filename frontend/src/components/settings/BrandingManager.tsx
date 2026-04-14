// Added: Branding manager component for white-label customization (TMAIL-111)
import { useState, useEffect } from 'react';
import type { FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Palette, Save, RotateCcw, ArrowLeft } from 'lucide-react';
import { getBranding, updateBranding, resetBranding } from '../../api/branding';
import type { Branding, UpdateBrandingRequest } from '../../api/branding';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

/**
 * PURPOSE: Admin UI for customizing instance branding (logo, colors, app name, etc.)
 * CONSTRAINTS: Only admins should access this — route protection handled by backend
 * EXTERNAL: Uses /api/branding (public GET) and /api/admin/branding (protected PUT/POST)
 */
export function BrandingManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [error, setError] = useState('');
  const [saved, setSaved] = useState(false);

  // Added: Form state for all branding fields
  const [appName, setAppName] = useState('TASMail');
  const [logoUrl, setLogoUrl] = useState('');
  const [faviconUrl, setFaviconUrl] = useState('');
  const [primaryColor, setPrimaryColor] = useState('#2563eb');
  const [secondaryColor, setSecondaryColor] = useState('#1e40af');
  const [accentColor, setAccentColor] = useState('#3b82f6');
  const [loginBackgroundUrl, setLoginBackgroundUrl] = useState('');
  const [customCss, setCustomCss] = useState('');
  const [footerText, setFooterText] = useState('');
  const [supportEmail, setSupportEmail] = useState('');
  const [supportUrl, setSupportUrl] = useState('');

  const { data: branding, isLoading } = useQuery<Branding>({
    queryKey: ['branding'],
    queryFn: getBranding,
  });

  // Added: Populate form when branding data loads
  useEffect(() => {
    if (branding) {
      setAppName(branding.app_name);
      setLogoUrl(branding.logo_url ?? '');
      setFaviconUrl(branding.favicon_url ?? '');
      setPrimaryColor(branding.primary_color);
      setSecondaryColor(branding.secondary_color);
      setAccentColor(branding.accent_color);
      setLoginBackgroundUrl(branding.login_background_url ?? '');
      setCustomCss(branding.custom_css ?? '');
      setFooterText(branding.footer_text ?? '');
      setSupportEmail(branding.support_email ?? '');
      setSupportUrl(branding.support_url ?? '');
    }
  }, [branding]);

  const updateMutation = useMutation({
    mutationFn: (data: UpdateBrandingRequest) => updateBranding(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['branding'] });
      setSaved(true);
      setError('');
      setTimeout(() => setSaved(false), 3000);
    },
    onError: (err: Error) => {
      setError(err.message);
      setSaved(false);
    },
  });

  const resetMutation = useMutation({
    mutationFn: () => resetBranding(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['branding'] });
      setSaved(true);
      setError('');
      setTimeout(() => setSaved(false), 3000);
    },
    onError: (err: Error) => {
      setError(err.message);
    },
  });

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    updateMutation.mutate({
      app_name: appName,
      logo_url: logoUrl || undefined,
      favicon_url: faviconUrl || undefined,
      primary_color: primaryColor,
      secondary_color: secondaryColor,
      accent_color: accentColor,
      login_background_url: loginBackgroundUrl || undefined,
      custom_css: customCss || undefined,
      footer_text: footerText || undefined,
      support_email: supportEmail || undefined,
      support_url: supportUrl || undefined,
    });
  };

  const handleReset = () => {
    if (window.confirm('Reset all branding to defaults? This cannot be undone.')) {
      resetMutation.mutate();
    }
  };

  if (isLoading) return <LoadingSkeleton />;

  return (
    <div className="settings-panel" style={{ padding: '24px', maxWidth: '800px' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '20px' }}>
        <button className="btn btn--icon" title="Back" onClick={() => setViewMode('list')}>
          <ArrowLeft size={18} />
        </button>
        <Palette size={24} />
        <h2 style={{ margin: 0 }}>Branding</h2>
      </div>

      {error && (
        <div className="alert alert--error" style={{ marginBottom: '16px' }}>
          {error}
        </div>
      )}
      {saved && (
        <div className="alert alert--success" style={{ marginBottom: '16px' }}>
          Branding saved successfully.
        </div>
      )}

      <form onSubmit={handleSubmit}>
        {/* Added: App name input */}
        <div className="form-group" style={{ marginBottom: '16px' }}>
          <label htmlFor="branding-app-name">App Name</label>
          <input
            id="branding-app-name"
            type="text"
            className="input"
            value={appName}
            onChange={(e) => setAppName(e.target.value)}
            placeholder="Application name"
          />
        </div>

        {/* Added: Logo URL with preview */}
        <div className="form-group" style={{ marginBottom: '16px' }}>
          <label htmlFor="branding-logo-url">Logo URL</label>
          <input
            id="branding-logo-url"
            type="url"
            className="input"
            value={logoUrl}
            onChange={(e) => setLogoUrl(e.target.value)}
            placeholder="https://example.com/logo.png"
          />
          {logoUrl && (
            <div style={{ marginTop: '8px' }}>
              <img
                src={logoUrl}
                alt="Logo preview"
                style={{ maxHeight: '48px', maxWidth: '200px', objectFit: 'contain' }}
              />
            </div>
          )}
        </div>

        {/* Added: Favicon URL */}
        <div className="form-group" style={{ marginBottom: '16px' }}>
          <label htmlFor="branding-favicon-url">Favicon URL</label>
          <input
            id="branding-favicon-url"
            type="url"
            className="input"
            value={faviconUrl}
            onChange={(e) => setFaviconUrl(e.target.value)}
            placeholder="https://example.com/favicon.ico"
          />
        </div>

        {/* Added: Color pickers — primary, secondary, accent */}
        <div style={{ display: 'flex', gap: '16px', marginBottom: '16px', flexWrap: 'wrap' }}>
          <div className="form-group">
            <label htmlFor="branding-primary-color">Primary Color</label>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <input
                id="branding-primary-color"
                type="color"
                value={primaryColor}
                onChange={(e) => setPrimaryColor(e.target.value)}
              />
              <span>{primaryColor}</span>
            </div>
          </div>
          <div className="form-group">
            <label htmlFor="branding-secondary-color">Secondary Color</label>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <input
                id="branding-secondary-color"
                type="color"
                value={secondaryColor}
                onChange={(e) => setSecondaryColor(e.target.value)}
              />
              <span>{secondaryColor}</span>
            </div>
          </div>
          <div className="form-group">
            <label htmlFor="branding-accent-color">Accent Color</label>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <input
                id="branding-accent-color"
                type="color"
                value={accentColor}
                onChange={(e) => setAccentColor(e.target.value)}
              />
              <span>{accentColor}</span>
            </div>
          </div>
        </div>

        {/* Added: Login background URL */}
        <div className="form-group" style={{ marginBottom: '16px' }}>
          <label htmlFor="branding-login-bg">Login Background URL</label>
          <input
            id="branding-login-bg"
            type="url"
            className="input"
            value={loginBackgroundUrl}
            onChange={(e) => setLoginBackgroundUrl(e.target.value)}
            placeholder="https://example.com/background.jpg"
          />
        </div>

        {/* Added: Custom CSS textarea */}
        <div className="form-group" style={{ marginBottom: '16px' }}>
          <label htmlFor="branding-custom-css">Custom CSS</label>
          <textarea
            id="branding-custom-css"
            className="input"
            value={customCss}
            onChange={(e) => setCustomCss(e.target.value)}
            placeholder="/* Custom CSS overrides */"
            rows={5}
            style={{ fontFamily: 'monospace', resize: 'vertical' }}
          />
        </div>

        {/* Added: Footer text */}
        <div className="form-group" style={{ marginBottom: '16px' }}>
          <label htmlFor="branding-footer-text">Footer Text</label>
          <input
            id="branding-footer-text"
            type="text"
            className="input"
            value={footerText}
            onChange={(e) => setFooterText(e.target.value)}
            placeholder="Powered by TASMail"
          />
        </div>

        {/* Added: Support email and URL */}
        <div style={{ display: 'flex', gap: '16px', marginBottom: '16px', flexWrap: 'wrap' }}>
          <div className="form-group" style={{ flex: 1, minWidth: '200px' }}>
            <label htmlFor="branding-support-email">Support Email</label>
            <input
              id="branding-support-email"
              type="email"
              className="input"
              value={supportEmail}
              onChange={(e) => setSupportEmail(e.target.value)}
              placeholder="support@example.com"
            />
          </div>
          <div className="form-group" style={{ flex: 1, minWidth: '200px' }}>
            <label htmlFor="branding-support-url">Support URL</label>
            <input
              id="branding-support-url"
              type="url"
              className="input"
              value={supportUrl}
              onChange={(e) => setSupportUrl(e.target.value)}
              placeholder="https://support.example.com"
            />
          </div>
        </div>

        {/* Added: Live preview section */}
        <div style={{ border: '1px solid var(--color-border)', borderRadius: '8px', padding: '16px', marginBottom: '20px' }}>
          <h3 style={{ marginTop: 0 }}>Live Preview</h3>
          <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '12px' }}>
            {logoUrl && (
              <img src={logoUrl} alt="Preview logo" style={{ maxHeight: '32px', objectFit: 'contain' }} />
            )}
            <span style={{ fontSize: '18px', fontWeight: 600, color: primaryColor }}>
              {appName}
            </span>
          </div>
          <div style={{ display: 'flex', gap: '8px', marginBottom: '8px' }}>
            <div
              style={{
                width: '60px',
                height: '32px',
                borderRadius: '4px',
                backgroundColor: primaryColor,
              }}
              title="Primary"
            />
            <div
              style={{
                width: '60px',
                height: '32px',
                borderRadius: '4px',
                backgroundColor: secondaryColor,
              }}
              title="Secondary"
            />
            <div
              style={{
                width: '60px',
                height: '32px',
                borderRadius: '4px',
                backgroundColor: accentColor,
              }}
              title="Accent"
            />
          </div>
          {footerText && (
            <div style={{ fontSize: '12px', color: '#666', marginTop: '8px' }}>
              {footerText}
            </div>
          )}
        </div>

        {/* Added: Action buttons — Save and Reset */}
        <div style={{ display: 'flex', gap: '12px' }}>
          <button
            type="submit"
            className="btn btn--primary"
            disabled={updateMutation.isPending}
          >
            <Save size={16} />
            {updateMutation.isPending ? 'Saving...' : 'Save Branding'}
          </button>
          <button
            type="button"
            className="btn btn--secondary"
            onClick={handleReset}
            disabled={resetMutation.isPending}
          >
            <RotateCcw size={16} />
            {resetMutation.isPending ? 'Resetting...' : 'Reset to Defaults'}
          </button>
        </div>
      </form>
    </div>
  );
}
