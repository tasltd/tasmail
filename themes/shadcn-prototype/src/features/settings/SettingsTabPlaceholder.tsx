// TMAIL-323: placeholder pane rendered by every settings tab until its real
// implementation lands in P1. Reads label + description from the tab
// registry so future content swaps are one-edit changes — replace the route
// element with the real pane component without touching this file.
import type { SettingsTab } from '@/features/settings/tabs';

interface SettingsTabPlaceholderProps {
  tab: SettingsTab;
}

export function SettingsTabPlaceholder({ tab }: SettingsTabPlaceholderProps) {
  const Icon = tab.icon;
  return (
    <div
      data-testid={`${tab.testId}-pane`}
      className="h-full w-full p-6 sm:p-8 overflow-y-auto"
    >
      <header className="flex items-center gap-3 mb-4">
        <Icon
          className="size-6 text-blue-600 dark:text-blue-400"
          aria-hidden="true"
        />
        <h2 className="text-xl sm:text-2xl font-semibold">{tab.label}</h2>
      </header>
      <p className="text-sm text-zinc-600 dark:text-zinc-400 max-w-2xl mb-6">
        {tab.description}
      </p>
      <div
        data-testid={`${tab.testId}-coming-soon`}
        className="rounded-lg border border-dashed border-zinc-300 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-900/40 p-6 sm:p-8 text-sm text-zinc-500"
      >
        <p className="font-medium text-zinc-700 dark:text-zinc-300 mb-1">
          Coming soon
        </p>
        <p>
          This pane lands in a follow-up task. The classic UI already exposes
          the underlying controls — use the &ldquo;← Classic&rdquo; link in
          the top bar to manage these settings today.
        </p>
      </div>
    </div>
  );
}
