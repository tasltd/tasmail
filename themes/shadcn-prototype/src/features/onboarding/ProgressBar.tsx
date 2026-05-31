// Added (TMAIL-346): Step indicator for the Modern UI onboarding wizard.
import { Check } from 'lucide-react';
import { cn } from '@/components/ui/utils';
import type { Step } from './types';

const ORDER: Step[] = ['provider', 'imap', 'smtp', 'done'];
const LABELS: Record<Step, string> = {
  provider: 'Provider',
  imap: 'IMAP (incoming)',
  smtp: 'SMTP (outgoing)',
  done: 'Done',
};

export function ProgressBar({ step }: { step: Step }) {
  const idx = ORDER.indexOf(step);
  // The 'done' step is the terminal state; we still want it to render so the
  // user sees full progress when finished.
  return (
    <ol
      className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
      aria-label="Onboarding progress"
    >
      {ORDER.slice(0, 3).map((s, i) => {
        const done = i < idx;
        const active = i === idx;
        return (
          <li key={s} className="flex items-center gap-2">
            <span
              className={cn(
                'flex size-6 items-center justify-center rounded-full border text-xs font-medium',
                done && 'border-primary bg-primary text-primary-foreground',
                active && !done && 'border-primary text-primary',
                !active && !done && 'border-muted text-muted-foreground',
              )}
              aria-current={active ? 'step' : undefined}
            >
              {done ? <Check className="size-3.5" aria-hidden="true" /> : i + 1}
            </span>
            <span className={cn(active && 'text-foreground font-medium')}>{LABELS[s]}</span>
            {i < 2 && <span className="text-muted-foreground/40">›</span>}
          </li>
        );
      })}
    </ol>
  );
}
