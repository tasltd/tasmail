// Added (TMAIL-346): Provider preset picker. Lists results from
// /api/imap-configs/presets and surfaces an auto-detect suggestion when the
// signed-in user's email domain matches one of the presets.
import { Button } from '@/components/ui/button';
import { Sparkles } from 'lucide-react';
import type { ProviderPreset } from '@/api/byok';

interface Props {
  presets: ProviderPreset[];
  suggestion: ProviderPreset | null;
  onPick: (preset: ProviderPreset | null) => void;
}

export function ProviderStep({ presets, suggestion, onPick }: Props) {
  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold">Who hosts your email?</h2>
        <p className="text-sm text-muted-foreground">
          Pick your provider so we can pre-fill the server settings.
        </p>
      </div>

      {suggestion && (
        <button
          type="button"
          onClick={() => onPick(suggestion)}
          className="group flex w-full items-center gap-3 rounded-lg border border-primary/30 bg-primary/5 p-4 text-left transition hover:border-primary hover:bg-primary/10"
          data-testid="provider-suggestion"
        >
          <span className="flex size-9 shrink-0 items-center justify-center rounded-full bg-primary/10 text-primary">
            <Sparkles className="size-4" aria-hidden="true" />
          </span>
          <span className="flex-1">
            <span className="block text-sm font-semibold">Use {suggestion.name}</span>
            <span className="block text-xs text-muted-foreground">
              auto-detected from your address
            </span>
          </span>
          <span className="text-xs text-primary opacity-0 transition group-hover:opacity-100">
            Continue →
          </span>
        </button>
      )}

      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        {presets.map((p) => (
          <button
            key={p.name}
            type="button"
            onClick={() => onPick(p)}
            data-testid={`provider-${p.domain}`}
            className="flex w-full flex-col items-start rounded-lg border bg-card p-3 text-left text-card-foreground shadow-sm transition hover:border-primary hover:shadow"
          >
            <span className="text-sm font-medium">{p.name}</span>
            <span className="text-xs text-muted-foreground">{p.domain}</span>
          </button>
        ))}
      </div>

      <Button
        type="button"
        variant="outline"
        className="w-full"
        onClick={() => onPick(null)}
        data-testid="provider-custom"
      >
        Other / Custom — enter server settings manually
      </Button>
    </div>
  );
}
