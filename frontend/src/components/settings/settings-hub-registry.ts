// Added (TMAIL-399): data-driven registry for the SettingsHub page.
//
// Per the "Modularize" rule, category/section membership is config, not code:
// adding a new settings panel means appending one entry here, no SettingsHub
// edit required. The hub component iterates this list to render both the
// left-rail category tabs and the per-category section list.
//
// Each section's `component` is a React.lazy import so a manager's bundle is
// only fetched when its section is actually opened — matches the lazy-load
// posture AppShell already uses for managers (TMAIL-259).
import { lazy } from 'react';
import type { ComponentType, LazyExoticComponent } from 'react';

export interface SettingsSection {
  id: string;
  label: string;
  component: LazyExoticComponent<ComponentType>;
}

export interface SettingsCategory {
  id: string;
  label: string;
  sections: SettingsSection[];
}

// Order is render order — both for the left-rail tabs and the section list
// inside each category.
export const SETTINGS_CATEGORIES: SettingsCategory[] = [
  {
    id: 'account',
    label: 'Account & Security',
    sections: [
      {
        id: 'security',
        label: 'Two-Factor Auth',
        component: lazy(() =>
          import('./TwoFactorManager').then((m) => ({ default: m.TwoFactorManager })),
        ),
      },
      {
        id: 'push-devices',
        label: 'Push Notifications',
        component: lazy(() =>
          import('./PushDevicesManager').then((m) => ({ default: m.PushDevicesManager })),
        ),
      },
    ],
  },
  {
    id: 'mail',
    label: 'Mail',
    sections: [
      {
        id: 'signatures',
        label: 'Signatures',
        component: lazy(() =>
          import('./SignatureManager').then((m) => ({ default: m.SignatureManager })),
        ),
      },
      {
        id: 'vacation',
        label: 'Vacation Responder',
        component: lazy(() =>
          import('./VacationResponder').then((m) => ({ default: m.VacationResponder })),
        ),
      },
      {
        id: 'filters',
        label: 'Filters',
        component: lazy(() =>
          import('./FilterManager').then((m) => ({ default: m.FilterManager })),
        ),
      },
      {
        id: 'templates',
        label: 'Templates',
        component: lazy(() =>
          import('./TemplateManager').then((m) => ({ default: m.TemplateManager })),
        ),
      },
      {
        id: 'spam',
        label: 'Spam Filter',
        component: lazy(() =>
          import('./SpamFilterManager').then((m) => ({ default: m.SpamFilterManager })),
        ),
      },
    ],
  },
  {
    id: 'connections',
    label: 'Connections',
    sections: [
      {
        id: 'smtp',
        label: 'SMTP',
        component: lazy(() =>
          import('./SmtpConfigManager').then((m) => ({ default: m.SmtpConfigManager })),
        ),
      },
      {
        id: 'pop3',
        label: 'POP3',
        component: lazy(() =>
          import('./Pop3ConfigManager').then((m) => ({ default: m.Pop3ConfigManager })),
        ),
      },
      {
        id: 'dav',
        label: 'CalDAV / CardDAV',
        component: lazy(() =>
          import('./DavConfigManager').then((m) => ({ default: m.DavConfigManager })),
        ),
      },
      {
        id: 'migration',
        label: 'Migration',
        component: lazy(() =>
          import('./MigrationManager').then((m) => ({ default: m.MigrationManager })),
        ),
      },
      {
        id: 'shared-files',
        label: 'Shared Files',
        component: lazy(() =>
          import('./SharedFileManager').then((m) => ({ default: m.SharedFileManager })),
        ),
      },
      {
        id: 'groups',
        label: 'Groups',
        component: lazy(() =>
          import('./GroupManager').then((m) => ({ default: m.GroupManager })),
        ),
      },
    ],
  },
  {
    id: 'productivity',
    label: 'Productivity',
    sections: [
      {
        id: 'ai-config',
        label: 'AI Provider',
        component: lazy(() =>
          import('./AiConfigManager').then((m) => ({ default: m.AiConfigManager })),
        ),
      },
      {
        id: 'ollama',
        label: 'Ollama (Local LLM)',
        component: lazy(() =>
          import('./OllamaManager').then((m) => ({ default: m.OllamaManager })),
        ),
      },
      {
        id: 'bandwidth',
        label: 'Low Bandwidth',
        component: lazy(() =>
          import('./LowBandwidthSettings').then((m) => ({ default: m.LowBandwidthSettings })),
        ),
      },
    ],
  },
];

// The hub defaults the left-rail selection to this category when the URL
// has no :category param.
export const DEFAULT_CATEGORY_ID = SETTINGS_CATEGORIES[0].id;

// Look up a category by id, falling back to the default category so a typo'd
// or stale deep link still lands on something useful.
export function findCategory(id?: string): SettingsCategory {
  if (!id) return SETTINGS_CATEGORIES[0];
  return SETTINGS_CATEGORIES.find((c) => c.id === id) ?? SETTINGS_CATEGORIES[0];
}

// Look up a section by id within a category, falling back to the category's
// first section so an unknown :section param still renders something.
export function findSection(category: SettingsCategory, id?: string): SettingsSection {
  if (!id) return category.sections[0];
  return category.sections.find((s) => s.id === id) ?? category.sections[0];
}
