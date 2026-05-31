// TMAIL-353: Modern UI Admin → Branding sub-tab. Live-loads the singleton
// branding row from /api/branding, edits the white-label fields, and
// persists with PUT /api/admin/branding. The "Reset to defaults" button
// hits POST /api/admin/branding/reset.
//
// NOTE: `logo_url` is stored as a URL — uploading the binary itself is a
// separate concern (the backend has /api/admin/branding/upload-logo for
// media — out of scope for TMAIL-353). The admin pastes a hosted URL.
import { useEffect, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Save, RotateCcw, AlertCircle, CheckCircle, Palette } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { adminBrandingApi, type Branding, type UpdateBrandingRequest } from '@/api/admin-branding';

type FormState = {
  app_name: string;
  logo_url: string;
  favicon_url: string;
  primary_color: string;
  secondary_color: string;
  accent_color: string;
  login_background_url: string;
  custom_css: string;
  footer_text: string;
  support_email: string;
  support_url: string;
};

function toForm(b: Branding): FormState {
  return {
    app_name: b.app_name ?? '',
    logo_url: b.logo_url ?? '',
    favicon_url: b.favicon_url ?? '',
    primary_color: b.primary_color ?? '#2563eb',
    secondary_color: b.secondary_color ?? '#1e40af',
    accent_color: b.accent_color ?? '#3b82f6',
    login_background_url: b.login_background_url ?? '',
    custom_css: b.custom_css ?? '',
    footer_text: b.footer_text ?? '',
    support_email: b.support_email ?? '',
    support_url: b.support_url ?? '',
  };
}

// Build the partial update payload. Empty strings collapse to `null` so the
// backend can clear an existing override (NOTE: the COALESCE in the SQL
// keeps existing values for `undefined`, while a literal `null` overrides
// to NULL — the wire null is what wipes a previously-set URL).
function toUpdateBody(form: FormState): UpdateBrandingRequest {
  const emptyToNull = (s: string) => (s.trim() === '' ? null : s.trim());
  return {
    app_name: form.app_name.trim() || undefined,
    logo_url: emptyToNull(form.logo_url),
    favicon_url: emptyToNull(form.favicon_url),
    primary_color: form.primary_color || undefined,
    secondary_color: form.secondary_color || undefined,
    accent_color: form.accent_color || undefined,
    login_background_url: emptyToNull(form.login_background_url),
    custom_css: emptyToNull(form.custom_css),
    footer_text: emptyToNull(form.footer_text),
    support_email: emptyToNull(form.support_email),
    support_url: emptyToNull(form.support_url),
  };
}

export function BrandingTab() {
  const qc = useQueryClient();
  const brandingQ = useQuery<Branding>({
    queryKey: ['admin', 'branding'],
    queryFn: () => adminBrandingApi.get(),
  });

  const [form, setForm] = useState<FormState | null>(null);
  const [savedNote, setSavedNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Hydrate the form from the server payload once on load (and again on
  // reset). Avoid clobbering user edits on every refetch.
  useEffect(() => {
    if (brandingQ.data && form === null) {
      setForm(toForm(brandingQ.data));
    }
  }, [brandingQ.data, form]);

  const saveMut = useMutation({
    mutationFn: (body: UpdateBrandingRequest) => adminBrandingApi.update(body),
    onSuccess: (b) => {
      qc.setQueryData(['admin', 'branding'], b);
      setForm(toForm(b));
      setSavedNote('Branding saved');
      setError(null);
      window.setTimeout(() => setSavedNote(null), 2500);
    },
    onError: (e: Error) => setError(e.message),
  });

  const resetMut = useMutation({
    mutationFn: () => adminBrandingApi.reset(),
    onSuccess: (b) => {
      qc.setQueryData(['admin', 'branding'], b);
      setForm(toForm(b));
      setSavedNote('Branding reset to defaults');
      setError(null);
      window.setTimeout(() => setSavedNote(null), 2500);
    },
    onError: (e: Error) => setError(e.message),
  });

  if (brandingQ.isLoading || form === null) {
    return <div className="p-6 text-zinc-500 text-sm" data-testid="branding-loading">Loading branding…</div>;
  }
  if (brandingQ.isError) {
    return (
      <Card className="p-4 border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950">
        <div className="text-sm text-red-700 dark:text-red-300">
          Couldn't load branding. {String(brandingQ.error)}
        </div>
      </Card>
    );
  }

  const set = <K extends keyof FormState>(k: K, v: FormState[K]) =>
    setForm((p) => (p ? { ...p, [k]: v } : p));

  return (
    <div className="space-y-6" data-testid="branding-tab">
      {error && (
        <Card className="p-3 border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950">
          <div className="text-sm text-red-700 dark:text-red-300 flex items-center gap-2">
            <AlertCircle className="size-4" /> {error}
          </div>
        </Card>
      )}
      {savedNote && (
        <Card className="p-3 border-green-300 dark:border-green-800 bg-green-50 dark:bg-green-950">
          <div className="text-sm text-green-700 dark:text-green-300 flex items-center gap-2">
            <CheckCircle className="size-4" /> {savedNote}
          </div>
        </Card>
      )}

      <Card className="p-6 space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-semibold flex items-center gap-2">
            <Palette className="size-5" /> Branding
          </h2>
          <div className="flex gap-2">
            <Button
              variant="outline"
              onClick={() => {
                if (window.confirm('Reset all branding to factory defaults?')) {
                  resetMut.mutate();
                }
              }}
              disabled={resetMut.isPending}
              data-testid="branding-reset-button"
            >
              <RotateCcw className="size-4 mr-2" />
              {resetMut.isPending ? 'Resetting…' : 'Reset to defaults'}
            </Button>
            <Button
              onClick={() => saveMut.mutate(toUpdateBody(form))}
              disabled={saveMut.isPending}
              data-testid="branding-save-button"
            >
              <Save className="size-4 mr-2" />
              {saveMut.isPending ? 'Saving…' : 'Save'}
            </Button>
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="space-y-2">
            <Label htmlFor="branding-app-name">App name</Label>
            <Input
              id="branding-app-name"
              value={form.app_name}
              onChange={(e) => set('app_name', e.target.value)}
              placeholder="TASMail"
              data-testid="branding-app-name"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="branding-footer-text">Footer text</Label>
            <Input
              id="branding-footer-text"
              value={form.footer_text}
              onChange={(e) => set('footer_text', e.target.value)}
              placeholder="© 2026 Acme Corp"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="branding-logo-url">Logo URL</Label>
            <Input
              id="branding-logo-url"
              type="url"
              value={form.logo_url}
              onChange={(e) => set('logo_url', e.target.value)}
              placeholder="https://your-cdn/logo.png"
              data-testid="branding-logo-url"
            />
            {form.logo_url && (
              <img
                src={form.logo_url}
                alt="Logo preview"
                className="h-10 mt-1 object-contain"
                onError={(e) => ((e.target as HTMLImageElement).style.display = 'none')}
              />
            )}
          </div>
          <div className="space-y-2">
            <Label htmlFor="branding-favicon-url">Favicon URL</Label>
            <Input
              id="branding-favicon-url"
              type="url"
              value={form.favicon_url}
              onChange={(e) => set('favicon_url', e.target.value)}
              placeholder="https://your-cdn/favicon.ico"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="branding-login-bg">Login background URL</Label>
            <Input
              id="branding-login-bg"
              type="url"
              value={form.login_background_url}
              onChange={(e) => set('login_background_url', e.target.value)}
              placeholder="https://your-cdn/login-bg.jpg"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="branding-support-email">Support email</Label>
            <Input
              id="branding-support-email"
              type="email"
              value={form.support_email}
              onChange={(e) => set('support_email', e.target.value)}
              placeholder="help@example.com"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="branding-support-url">Support URL</Label>
            <Input
              id="branding-support-url"
              type="url"
              value={form.support_url}
              onChange={(e) => set('support_url', e.target.value)}
              placeholder="https://support.example.com"
            />
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 pt-2">
          <ColorField
            id="branding-primary-color"
            label="Primary colour"
            value={form.primary_color}
            onChange={(v) => set('primary_color', v)}
            testid="branding-primary-color"
          />
          <ColorField
            id="branding-secondary-color"
            label="Secondary colour"
            value={form.secondary_color}
            onChange={(v) => set('secondary_color', v)}
            testid="branding-secondary-color"
          />
          <ColorField
            id="branding-accent-color"
            label="Accent colour"
            value={form.accent_color}
            onChange={(v) => set('accent_color', v)}
            testid="branding-accent-color"
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="branding-custom-css">Custom CSS</Label>
          <textarea
            id="branding-custom-css"
            value={form.custom_css}
            onChange={(e) => set('custom_css', e.target.value)}
            placeholder=".sidebar { background: #000; }"
            rows={5}
            className="w-full rounded-md border border-zinc-200 dark:border-zinc-800 bg-transparent px-3 py-2 text-sm font-mono"
          />
        </div>
      </Card>
    </div>
  );
}

interface ColorFieldProps {
  id: string;
  label: string;
  value: string;
  onChange: (v: string) => void;
  testid?: string;
}

function ColorField({ id, label, value, onChange, testid }: ColorFieldProps) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      <div className="flex gap-2 items-center">
        <input
          id={id}
          type="color"
          value={value || '#000000'}
          onChange={(e) => onChange(e.target.value)}
          className="h-9 w-12 rounded border border-zinc-200 dark:border-zinc-800 cursor-pointer"
          data-testid={testid}
        />
        <Input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="#2563eb"
          className="flex-1 font-mono"
        />
      </div>
    </div>
  );
}
