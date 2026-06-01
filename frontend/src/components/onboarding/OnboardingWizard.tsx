// Onboarding wizard. Runs after signup (or any time the user has no IMAP/SMTP config yet).
//
// TMAIL-168 added a `path` step in front of the existing BYOK flow:
//   path → (BYOK)  → provider → imap → smtp → done
//   path → (DNS-MX)→ managed-mailbox-form → done       (only if dns_mx_onboarding_enabled)
//
// When only one onboarding path is enabled in feature flags, the path step is auto-skipped.
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import { byokApi, type ProviderPreset, type CreateImapConfig, type CreateSmtpConfig } from '../../api/byok';
import { featureFlagsApi, type FeatureFlag } from '../../api/featureFlags';
// TMAIL-404: prefetch folders during wizard so AppShell renders the real
// folder list immediately instead of getting stuck on "Loading folders…"
// while the IMAP login + LIST round-trips against the user's BYOK server.
import { fetchFolders } from '../../api/folders';
import './OnboardingWizard.css';

type Step = 'path' | 'provider' | 'imap' | 'smtp' | 'managed' | 'done';
type Path = 'byok' | 'dns_mx';
type Encryption = 'ssl' | 'starttls' | 'none';

interface ServerForm {
  host: string;
  port: number;
  username: string;
  password: string;
  encryption: Encryption;
}

const blank: ServerForm = { host: '', port: 0, username: '', password: '', encryption: 'ssl' };

export function OnboardingWizard() {
  const navigate = useNavigate();
  // TMAIL-404: shared QueryClient so we can invalidate any stale `folders`
  // / `messages` / `quota` entries left over from an earlier visit (e.g. the
  // user landed on /app with no IMAP config, got a 503-cached error, then
  // came back through the wizard) AND prefetch /api/folders before navigating
  // so the request is warm by the time AppShell mounts FolderTree.
  const queryClient = useQueryClient();
  const [step, setStep] = useState<Step>('path');
  const [presets, setPresets] = useState<ProviderPreset[]>([]);
  const [flags, setFlags] = useState<FeatureFlag[]>([]);
  const [emailHint, setEmailHint] = useState('');
  const [chosen, setChosen] = useState<ProviderPreset | null>(null);
  const [imap, setImap] = useState<ServerForm>({ ...blank, port: 993 });
  const [smtp, setSmtp] = useState<ServerForm>({ ...blank, port: 587, encryption: 'starttls' });
  const [managedLocal, setManagedLocal] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);

  // Pull saved email from local storage so we can suggest the right preset
  useEffect(() => {
    const token = localStorage.getItem('access_token');
    if (token) {
      try {
        const claims = JSON.parse(atob(token.split('.')[1]));
        if (claims.username) setEmailHint(claims.username);
      } catch { /* ignore */ }
    }
    byokApi.presets().then(setPresets).catch(() => setError('Could not load provider presets'));
    // TMAIL-168: pull public feature flags to decide which onboarding paths to show.
    featureFlagsApi.listPublic().then(setFlags).catch(() => { /* default to BYOK only */ });
  }, []);

  const byokEnabled = flags.find((f) => f.key === 'byok_onboarding_enabled')?.enabled ?? true;
  const dnsMxEnabled = flags.find((f) => f.key === 'dns_mx_onboarding_enabled')?.enabled ?? false;

  // If only one path is enabled, jump straight into it instead of showing the picker.
  useEffect(() => {
    if (step !== 'path') return;
    if (flags.length === 0) return; // wait for flags to load
    if (byokEnabled && !dnsMxEnabled) setStep('provider');
    else if (!byokEnabled && dnsMxEnabled) setStep('managed');
  }, [step, flags, byokEnabled, dnsMxEnabled]);

  const suggestion = useMemo(() => {
    if (!emailHint || !presets.length) return null;
    const domain = emailHint.split('@')[1]?.toLowerCase();
    return presets.find((p) => p.domain === domain) ?? null;
  }, [emailHint, presets]);

  function pickProvider(p: ProviderPreset | null) {
    setChosen(p);
    setError('');
    if (p) {
      setImap({
        host: p.imap.host,
        port: p.imap.port,
        username: emailHint || '',
        password: '',
        encryption: p.imap.encryption,
      });
      setSmtp({
        host: p.smtp.host,
        port: p.smtp.port,
        username: emailHint || '',
        password: '',
        encryption: p.smtp.encryption,
      });
    } else {
      // "Custom" / "None of these"
      setImap({ ...blank, port: 993, username: emailHint || '' });
      setSmtp({ ...blank, port: 587, username: emailHint || '', encryption: 'starttls' });
    }
    setStep('imap');
  }

  async function testImap() {
    setError(''); setTestResult(null); setBusy(true);
    try {
      const r = await byokApi.testImap({
        host: imap.host, port: imap.port, username: imap.username,
        password: imap.password, encryption: imap.encryption,
      });
      setTestResult(r);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Connection test failed');
    } finally { setBusy(false); }
  }

  async function saveImapAndContinue() {
    setError(''); setBusy(true);
    try {
      const req: CreateImapConfig = {
        name: chosen?.name ?? 'My IMAP server',
        host: imap.host,
        port: imap.port,
        username: imap.username,
        password: imap.password,
        encryption: imap.encryption,
        is_default: true,
      };
      await byokApi.createImap(req);
      setTestResult(null);
      setStep('smtp');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not save IMAP config');
    } finally { setBusy(false); }
  }

  async function saveSmtpAndFinish() {
    setError(''); setBusy(true);
    try {
      const req: CreateSmtpConfig = {
        name: chosen?.name ?? 'My SMTP server',
        host: smtp.host,
        port: smtp.port,
        username: smtp.username,
        password: smtp.password,
        encryption: smtp.encryption,
        from_address: emailHint || smtp.username,
        is_default: true,
      };
      await byokApi.createSmtp(req);

      // TMAIL-404: warm /api/folders BEFORE navigating to /app. Without this
      // the FolderTree mounts cold and shows "Loading folders…" for the full
      // duration of the first IMAP login + LIST round-trip against the BYOK
      // server (5-15 s for a fresh swmail/Gmail/Outlook session). Starting the
      // request here lets it run in parallel with the "done" overlay and the
      // route transition, so by the time AppShell mounts FolderTree the
      // request is either complete or far enough along that the user sees the
      // real folder list within seconds. `fetchQuery` is fire-and-forget — we
      // don't await it so the wizard's success state shows immediately.
      // The invalidate is paired so any stale error state from a previous
      // visit (e.g. 503 "No IMAP server configured") is dropped before the
      // prefetch attaches.
      queryClient.invalidateQueries({ queryKey: ['folders'] });
      queryClient.invalidateQueries({ queryKey: ['messages'] });
      queryClient.invalidateQueries({ queryKey: ['quota'] });
      void queryClient.prefetchQuery({
        queryKey: ['folders'],
        queryFn: fetchFolders,
        staleTime: 30_000,
      });

      setStep('done');
      // Changed (TMAIL-404): trim the success-screen delay from 1.2 s to 600 ms
      // — the prefetch above means the user no longer pays for an idle pause
      // before the real mailbox renders.
      setTimeout(() => navigate('/app', { replace: true }), 600);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not save SMTP config');
    } finally { setBusy(false); }
  }

  return (
    <div className="onboarding">
      <div className="onboarding-card">
        <header className="onboarding-card__header">
          <h1>Connect your mailbox</h1>
          <p>TASMail is a webmail UI for any IMAP/SMTP server &mdash; we never store your mail, only the encrypted credentials needed to fetch it.</p>
          <ProgressBar step={step} />
        </header>

        {error && <div className="onboarding-error" role="alert">{error}</div>}

        {step === 'path' && (
          <PathStep
            byokEnabled={byokEnabled}
            dnsMxEnabled={dnsMxEnabled}
            onPick={(p: Path) => setStep(p === 'byok' ? 'provider' : 'managed')}
          />
        )}

        {step === 'managed' && (
          <ManagedMailboxStep
            localPart={managedLocal}
            onChange={setManagedLocal}
            busy={busy}
            onBack={() => setStep('path')}
            onContinue={async () => {
              // The provisioning endpoint lives in TMAIL-167. Until that lands, surface a helpful message.
              setError(
                'DNS-MX provisioning is enabled but the backend endpoint is not yet implemented (see TMAIL-167). ' +
                'You can switch to BYOK from the previous step in the meantime.'
              );
            }}
          />
        )}

        {step === 'provider' && (
          <ProviderStep
            presets={presets}
            suggestion={suggestion}
            onPick={pickProvider}
          />
        )}

        {step === 'imap' && (
          <ServerStep
            kind="IMAP"
            value={imap}
            onChange={setImap}
            chosenHint={chosen?.hint}
            busy={busy}
            testResult={testResult}
            onTest={testImap}
            onBack={() => { setStep('provider'); setTestResult(null); }}
            onContinue={saveImapAndContinue}
          />
        )}

        {step === 'smtp' && (
          <ServerStep
            kind="SMTP"
            value={smtp}
            onChange={setSmtp}
            chosenHint={chosen?.hint}
            busy={busy}
            testResult={null}
            onBack={() => setStep('imap')}
            onContinue={saveSmtpAndFinish}
            continueLabel="Finish setup"
          />
        )}

        {step === 'done' && (
          <div className="onboarding-done">
            <div className="onboarding-done__check">✓</div>
            <h2>You&apos;re all set.</h2>
            <p>Taking you to your inbox…</p>
          </div>
        )}
      </div>
    </div>
  );
}

function ProgressBar({ step }: { step: Step }) {
  // The path step + managed-mailbox flow have different progress sequences than BYOK.
  // Show whichever lane the user is currently on.
  if (step === 'managed') {
    return (
      <div className="onboarding-progress">
        <div className="onboarding-progress__step is-active">
          <span className="onboarding-progress__dot">1</span>
          <span className="onboarding-progress__label">Pick a path</span>
        </div>
        <div className="onboarding-progress__step is-active">
          <span className="onboarding-progress__dot">2</span>
          <span className="onboarding-progress__label">Choose mailbox name</span>
        </div>
      </div>
    );
  }

  const byokOrder: Step[] = ['path', 'provider', 'imap', 'smtp', 'done'];
  const labels: Record<Step, string> = {
    path: 'Pick a path',
    provider: 'Provider',
    imap: 'IMAP (incoming)',
    smtp: 'SMTP (outgoing)',
    managed: 'Managed mailbox',
    done: 'Done',
  };
  const idx = byokOrder.indexOf(step);
  return (
    <div className="onboarding-progress">
      {byokOrder.slice(0, 4).map((s, i) => (
        <div key={s} className={`onboarding-progress__step ${i <= idx ? 'is-active' : ''}`}>
          <span className="onboarding-progress__dot">{i + 1}</span>
          <span className="onboarding-progress__label">{labels[s]}</span>
        </div>
      ))}
    </div>
  );
}

function PathStep({
  byokEnabled,
  dnsMxEnabled,
  onPick,
}: {
  byokEnabled: boolean;
  dnsMxEnabled: boolean;
  onPick: (p: Path) => void;
}) {
  return (
    <div className="onboarding-step">
      <h2>How do you want to use TASMail?</h2>
      <p className="onboarding-step__sub">Pick the path that matches your situation. You can change this later.</p>

      <div className="onboarding-paths">
        {byokEnabled && (
          <button className="onboarding-path onboarding-path--primary" onClick={() => onPick('byok')}>
            <span className="onboarding-path__name">Connect an existing account</span>
            <span className="onboarding-path__sub">
              You already have email at Gmail, Outlook, Zoho, FastMail, your company server… —
              TASMail becomes a webmail UI for that mailbox.
            </span>
            <span className="onboarding-path__cta">Recommended</span>
          </button>
        )}

        {dnsMxEnabled && (
          <button className="onboarding-path" onClick={() => onPick('dns_mx')}>
            <span className="onboarding-path__name">Get a new mailbox on this server</span>
            <span className="onboarding-path__sub">
              Provision a brand-new <code>@yourname.com</code> address on the operator&apos;s
              managed mail server.
            </span>
          </button>
        )}

        {!byokEnabled && !dnsMxEnabled && (
          <div className="onboarding-error">
            Onboarding is currently disabled. Contact the operator.
          </div>
        )}
      </div>
    </div>
  );
}

function ManagedMailboxStep({
  localPart,
  onChange,
  busy,
  onBack,
  onContinue,
}: {
  localPart: string;
  onChange: (v: string) => void;
  busy: boolean;
  onBack: () => void;
  onContinue: () => void;
}) {
  return (
    <div className="onboarding-step">
      <h2>Pick your new mailbox name</h2>
      <p className="onboarding-step__sub">Choose the local part of your new email address. The full address becomes <code>{localPart || 'yourname'}@&lt;managed-domain&gt;</code>.</p>

      <div className="onboarding-form">
        <div className="form-group">
          <label>Local part</label>
          <input
            value={localPart}
            onChange={(e) => onChange(e.target.value.toLowerCase().replace(/[^a-z0-9._-]/g, ''))}
            placeholder="yourname"
            maxLength={64}
            autoFocus
          />
        </div>
      </div>

      <div className="onboarding-actions">
        <button className="btn btn--ghost" onClick={onBack} disabled={busy}>Back</button>
        <button className="btn btn--primary" onClick={onContinue} disabled={busy || !localPart}>
          {busy ? 'Provisioning…' : 'Provision mailbox'}
        </button>
      </div>
    </div>
  );
}

function ProviderStep({ presets, suggestion, onPick }: {
  presets: ProviderPreset[];
  suggestion: ProviderPreset | null;
  onPick: (p: ProviderPreset | null) => void;
}) {
  return (
    <div className="onboarding-step">
      <h2>Who hosts your email?</h2>
      <p className="onboarding-step__sub">Pick your provider so we can pre-fill the server settings.</p>

      {suggestion && (
        <button className="onboarding-suggest" onClick={() => onPick(suggestion)}>
          <strong>Use {suggestion.name}</strong>
          <span>auto-detected from your address</span>
        </button>
      )}

      <div className="onboarding-providers">
        {presets.map((p) => (
          <button key={p.name} className="onboarding-provider" onClick={() => onPick(p)}>
            <span className="onboarding-provider__name">{p.name}</span>
            <span className="onboarding-provider__domain">{p.domain}</span>
          </button>
        ))}
        <button className="onboarding-provider onboarding-provider--custom" onClick={() => onPick(null)}>
          <span className="onboarding-provider__name">Other / Custom</span>
          <span className="onboarding-provider__domain">Enter server settings manually</span>
        </button>
      </div>
    </div>
  );
}

function ServerStep({
  kind, value, onChange, chosenHint, busy, testResult, onTest, onBack, onContinue, continueLabel,
}: {
  kind: 'IMAP' | 'SMTP';
  value: ServerForm;
  onChange: (v: ServerForm) => void;
  chosenHint?: string;
  busy: boolean;
  testResult: { ok: boolean; message: string } | null;
  onTest?: () => void;
  onBack: () => void;
  onContinue: () => void;
  continueLabel?: string;
}) {
  const canContinue = value.host && value.port && value.username && value.password;

  return (
    <div className="onboarding-step">
      <h2>{kind} server {kind === 'IMAP' ? '(incoming mail)' : '(outgoing mail)'}</h2>
      {chosenHint && <p className="onboarding-step__hint">💡 {chosenHint}</p>}

      <div className="onboarding-form">
        <div className="form-group">
          <label>Server host</label>
          <input value={value.host} onChange={(e) => onChange({ ...value, host: e.target.value })} placeholder={kind === 'IMAP' ? 'imap.example.com' : 'smtp.example.com'} />
        </div>
        <div className="onboarding-form__row">
          <div className="form-group">
            <label>Port</label>
            <input type="number" value={value.port || ''} onChange={(e) => onChange({ ...value, port: parseInt(e.target.value, 10) || 0 })} />
          </div>
          <div className="form-group">
            <label>Encryption</label>
            <select value={value.encryption} onChange={(e) => onChange({ ...value, encryption: e.target.value as Encryption })}>
              <option value="ssl">SSL/TLS</option>
              <option value="starttls">STARTTLS</option>
              <option value="none">None (insecure)</option>
            </select>
          </div>
        </div>
        <div className="form-group">
          <label>Username</label>
          <input value={value.username} onChange={(e) => onChange({ ...value, username: e.target.value })} placeholder="usually your full email address" />
        </div>
        <div className="form-group">
          <label>Password / App Password</label>
          <input type="password" value={value.password} onChange={(e) => onChange({ ...value, password: e.target.value })} autoComplete="off" />
        </div>
      </div>

      {testResult && (
        <div className={`onboarding-test-result ${testResult.ok ? 'is-ok' : 'is-fail'}`}>
          {testResult.ok ? '✓' : '✕'} {testResult.message}
        </div>
      )}

      <div className="onboarding-actions">
        <button className="btn btn--ghost" onClick={onBack} disabled={busy}>Back</button>
        <div className="onboarding-actions__right">
          {onTest && (
            <button className="btn btn--ghost" onClick={onTest} disabled={busy || !canContinue}>
              {busy ? 'Testing…' : 'Test connection'}
            </button>
          )}
          <button className="btn btn--primary" onClick={onContinue} disabled={busy || !canContinue}>
            {busy ? 'Saving…' : (continueLabel ?? 'Save & continue')}
          </button>
        </div>
      </div>
    </div>
  );
}
