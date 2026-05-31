// Added (TMAIL-346): IMAP/SMTP form step. Shared shape for incoming + outgoing.
import { ArrowLeft, Check, Loader2, X } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { cn } from '@/components/ui/utils';
import type { Encryption } from '@/api/byok';
import type { ServerForm } from './types';

interface Props {
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
}

export function ServerStep({
  kind,
  value,
  onChange,
  chosenHint,
  busy,
  testResult,
  onTest,
  onBack,
  onContinue,
  continueLabel,
}: Props) {
  const canContinue = Boolean(
    value.host && value.port && value.username && value.password,
  );
  const idPrefix = kind.toLowerCase();

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold">
          {kind} server {kind === 'IMAP' ? '(incoming mail)' : '(outgoing mail)'}
        </h2>
        {chosenHint && (
          <p className="mt-1 rounded-md bg-muted/60 px-3 py-2 text-xs text-muted-foreground">
            💡 {chosenHint}
          </p>
        )}
      </div>

      <div className="space-y-4">
        <div className="space-y-2">
          <Label htmlFor={`${idPrefix}-host`}>Server host</Label>
          <Input
            id={`${idPrefix}-host`}
            value={value.host}
            onChange={(e) => onChange({ ...value, host: e.target.value })}
            placeholder={kind === 'IMAP' ? 'imap.example.com' : 'smtp.example.com'}
            autoComplete="off"
          />
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-2">
            <Label htmlFor={`${idPrefix}-port`}>Port</Label>
            <Input
              id={`${idPrefix}-port`}
              type="number"
              value={value.port || ''}
              onChange={(e) =>
                onChange({ ...value, port: parseInt(e.target.value, 10) || 0 })
              }
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor={`${idPrefix}-encryption`}>Encryption</Label>
            <Select
              value={value.encryption}
              onValueChange={(v) => onChange({ ...value, encryption: v as Encryption })}
            >
              <SelectTrigger id={`${idPrefix}-encryption`}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="ssl">SSL/TLS</SelectItem>
                <SelectItem value="starttls">STARTTLS</SelectItem>
                <SelectItem value="none">None (insecure)</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div className="space-y-2">
          <Label htmlFor={`${idPrefix}-username`}>Username</Label>
          <Input
            id={`${idPrefix}-username`}
            value={value.username}
            onChange={(e) => onChange({ ...value, username: e.target.value })}
            placeholder="usually your full email address"
            autoComplete="off"
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor={`${idPrefix}-password`}>Password / App Password</Label>
          <Input
            id={`${idPrefix}-password`}
            type="password"
            value={value.password}
            onChange={(e) => onChange({ ...value, password: e.target.value })}
            autoComplete="off"
          />
        </div>
      </div>

      {testResult && (
        <Alert
          variant={testResult.ok ? 'default' : 'destructive'}
          data-testid={`${idPrefix}-test-result`}
        >
          <AlertDescription className="flex items-center gap-2">
            <span
              className={cn(
                'flex size-5 items-center justify-center rounded-full',
                testResult.ok ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700',
              )}
              aria-hidden="true"
            >
              {testResult.ok ? <Check className="size-3" /> : <X className="size-3" />}
            </span>
            <span>{testResult.message}</span>
          </AlertDescription>
        </Alert>
      )}

      <div className="flex items-center justify-between gap-2 pt-2">
        <Button type="button" variant="ghost" onClick={onBack} disabled={busy}>
          <ArrowLeft className="size-4" aria-hidden="true" />
          Back
        </Button>
        <div className="flex items-center gap-2">
          {onTest && (
            <Button
              type="button"
              variant="outline"
              onClick={onTest}
              disabled={busy || !canContinue}
              data-testid={`${idPrefix}-test-button`}
            >
              {busy && <Loader2 className="size-4 animate-spin" aria-hidden="true" />}
              {busy ? 'Testing…' : 'Test connection'}
            </Button>
          )}
          <Button
            type="button"
            onClick={onContinue}
            disabled={busy || !canContinue}
            data-testid={`${idPrefix}-continue-button`}
          >
            {busy && <Loader2 className="size-4 animate-spin" aria-hidden="true" />}
            {busy ? 'Saving…' : continueLabel ?? 'Save & continue'}
          </Button>
        </div>
      </div>
    </div>
  );
}
