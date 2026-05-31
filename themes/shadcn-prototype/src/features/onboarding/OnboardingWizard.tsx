// Added (TMAIL-346): Native Modern UI onboarding wizard.
//
// Mirrors the classic SPA's frontend/src/components/onboarding/OnboardingWizard.tsx
// but uses shadcn/ui primitives so the Modern UI can run standalone — signup
// no longer has to bounce back to /onboarding in the classic SPA.
//
// Flow:
//   provider → imap → smtp → done
//
// Each step writes to /api/imap-configs or /api/smtp-configs via the modern
// `apiClient` and stores credentials AES-256-GCM-encrypted at rest in the
// backend (see backend/src/services/encryption.rs).
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router';
import { CheckCircle2, Mail } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import {
  byokApi,
  type CreateImapConfig,
  type CreateSmtpConfig,
  type ProviderPreset,
} from '@/api/byok';
import { ProgressBar } from './ProgressBar';
import { ProviderStep } from './ProviderStep';
import { ServerStep } from './ServerStep';
import { BLANK_IMAP, BLANK_SMTP, type ServerForm, type Step } from './types';

function readUsernameClaim(): string {
  const token =
    localStorage.getItem('access_token') ||
    sessionStorage.getItem('access_token');
  if (!token) return '';
  try {
    const claims = JSON.parse(atob(token.split('.')[1]));
    return (claims.username as string | undefined) ?? '';
  } catch {
    return '';
  }
}

export function OnboardingWizard() {
  const navigate = useNavigate();
  const [step, setStep] = useState<Step>('provider');
  const [presets, setPresets] = useState<ProviderPreset[]>([]);
  const [emailHint, setEmailHint] = useState('');
  const [chosen, setChosen] = useState<ProviderPreset | null>(null);
  const [imap, setImap] = useState<ServerForm>(BLANK_IMAP);
  const [smtp, setSmtp] = useState<ServerForm>(BLANK_SMTP);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(
    null,
  );

  useEffect(() => {
    setEmailHint(readUsernameClaim());
    byokApi
      .presets()
      .then(setPresets)
      .catch(() => setError('Could not load provider presets'));
  }, []);

  const suggestion = useMemo(() => {
    if (!emailHint || presets.length === 0) return null;
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
        username: emailHint,
        password: '',
        encryption: p.imap.encryption,
      });
      setSmtp({
        host: p.smtp.host,
        port: p.smtp.port,
        username: emailHint,
        password: '',
        encryption: p.smtp.encryption,
      });
    } else {
      setImap({ ...BLANK_IMAP, username: emailHint });
      setSmtp({ ...BLANK_SMTP, username: emailHint });
    }
    setStep('imap');
  }

  async function testImap() {
    setError('');
    setTestResult(null);
    setBusy(true);
    try {
      const r = await byokApi.testImap({
        host: imap.host,
        port: imap.port,
        username: imap.username,
        password: imap.password,
        encryption: imap.encryption,
      });
      setTestResult(r);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Connection test failed');
    } finally {
      setBusy(false);
    }
  }

  async function saveImapAndContinue() {
    setError('');
    setBusy(true);
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
    } finally {
      setBusy(false);
    }
  }

  async function saveSmtpAndFinish() {
    setError('');
    setBusy(true);
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
      setStep('done');
      // Brief pause so the user sees the success state, then drop them in
      // the inbox.
      setTimeout(() => navigate('/', { replace: true }), 1200);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not save SMTP config');
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-zinc-50 to-zinc-100 dark:from-zinc-950 dark:to-zinc-900">
      <div className="mx-auto flex max-w-2xl flex-col px-4 py-10">
        <div className="mb-6 flex flex-col items-center gap-2">
          <div
            className="flex size-14 items-center justify-center rounded-xl bg-primary text-primary-foreground"
            aria-hidden="true"
          >
            <Mail className="size-7" />
          </div>
          <h1 className="text-2xl font-semibold tracking-tight">Connect your mailbox</h1>
          <p className="max-w-md text-center text-sm text-muted-foreground">
            TASMail is a webmail UI for any IMAP/SMTP server — we never store your mail,
            only the encrypted credentials needed to fetch it.
          </p>
        </div>

        <div className="rounded-xl border bg-card p-6 text-card-foreground shadow-sm">
          <div className="mb-6">
            <ProgressBar step={step} />
          </div>

          {error && (
            <Alert variant="destructive" className="mb-4" role="alert">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}

          {step === 'provider' && (
            <ProviderStep presets={presets} suggestion={suggestion} onPick={pickProvider} />
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
              onBack={() => {
                setStep('provider');
                setTestResult(null);
              }}
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
            <div
              className="flex flex-col items-center gap-3 py-10 text-center"
              data-testid="onboarding-done"
            >
              <CheckCircle2 className="size-12 text-green-600" aria-hidden="true" />
              <h2 className="text-lg font-semibold">You&apos;re all set.</h2>
              <p className="text-sm text-muted-foreground">Taking you to your inbox…</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
