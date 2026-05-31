// Added (TMAIL-399): unit tests for SettingsHub — verify the registry is
// rendered, deep links resolve to the right pane, and category/section
// switching pushes the expected URL.
//
// Every section component in the registry is mocked at the module path used
// inside settings-hub-registry.ts so the hub never tries to mount a real
// manager (those have their own tests + heavy API dependencies).
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter, Routes, Route, useLocation } from 'react-router-dom';
import { SettingsHub } from './SettingsHub';
import { SETTINGS_CATEGORIES, findCategory, findSection } from './settings-hub-registry';

// Lightweight stand-ins for every manager the hub references. The mock
// modules expose the same named exports the real ones do so the registry's
// lazy imports resolve to a trivial component during tests.
vi.mock('./TwoFactorManager', () => ({
  TwoFactorManager: () => <div data-testid="manager-security">Security pane</div>,
}));
vi.mock('./PushDevicesManager', () => ({
  PushDevicesManager: () => <div data-testid="manager-push-devices">Push devices pane</div>,
}));
vi.mock('./SignatureManager', () => ({
  SignatureManager: () => <div data-testid="manager-signatures">Signatures pane</div>,
}));
vi.mock('./VacationResponder', () => ({
  VacationResponder: () => <div data-testid="manager-vacation">Vacation pane</div>,
}));
vi.mock('./FilterManager', () => ({
  FilterManager: () => <div data-testid="manager-filters">Filters pane</div>,
}));
vi.mock('./TemplateManager', () => ({
  TemplateManager: () => <div data-testid="manager-templates">Templates pane</div>,
}));
vi.mock('./SpamFilterManager', () => ({
  SpamFilterManager: () => <div data-testid="manager-spam">Spam pane</div>,
}));
vi.mock('./SmtpConfigManager', () => ({
  SmtpConfigManager: () => <div data-testid="manager-smtp">SMTP pane</div>,
}));
vi.mock('./Pop3ConfigManager', () => ({
  Pop3ConfigManager: () => <div data-testid="manager-pop3">POP3 pane</div>,
}));
vi.mock('./DavConfigManager', () => ({
  DavConfigManager: () => <div data-testid="manager-dav">DAV pane</div>,
}));
vi.mock('./MigrationManager', () => ({
  MigrationManager: () => <div data-testid="manager-migration">Migration pane</div>,
}));
vi.mock('./SharedFileManager', () => ({
  SharedFileManager: () => <div data-testid="manager-shared-files">Shared files pane</div>,
}));
vi.mock('./GroupManager', () => ({
  GroupManager: () => <div data-testid="manager-groups">Groups pane</div>,
}));
vi.mock('./AiConfigManager', () => ({
  AiConfigManager: () => <div data-testid="manager-ai-config">AI config pane</div>,
}));
vi.mock('./OllamaManager', () => ({
  OllamaManager: () => <div data-testid="manager-ollama">Ollama pane</div>,
}));
vi.mock('./LowBandwidthSettings', () => ({
  LowBandwidthSettings: () => <div data-testid="manager-bandwidth">Bandwidth pane</div>,
}));

// Probe component that surfaces the current location so we can assert
// navigation side-effects without depending on the AppShell chrome.
function LocationProbe() {
  const location = useLocation();
  return <div data-testid="location">{location.pathname}</div>;
}

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/app/settings/*" element={<SettingsHub />} />
      </Routes>
      <LocationProbe />
    </MemoryRouter>,
  );
}

describe('settings-hub-registry', () => {
  it('exposes the four required categories in order', () => {
    expect(SETTINGS_CATEGORIES.map((c) => c.id)).toEqual([
      'account',
      'mail',
      'connections',
      'productivity',
    ]);
  });

  it('lists every section the TMAIL-399 spec requires', () => {
    const byId: Record<string, string[]> = Object.fromEntries(
      SETTINGS_CATEGORIES.map((c) => [c.id, c.sections.map((s) => s.id)]),
    );
    expect(byId.account).toEqual(expect.arrayContaining(['security', 'push-devices']));
    expect(byId.mail).toEqual(
      expect.arrayContaining(['signatures', 'vacation', 'filters', 'templates', 'spam']),
    );
    expect(byId.connections).toEqual(
      expect.arrayContaining([
        'smtp',
        'pop3',
        'dav',
        'migration',
        'shared-files',
        'groups',
      ]),
    );
    expect(byId.productivity).toEqual(
      expect.arrayContaining(['ai-config', 'ollama', 'bandwidth']),
    );
  });

  it('findCategory falls back to the default category when the id is unknown', () => {
    expect(findCategory(undefined).id).toBe('account');
    expect(findCategory('nope').id).toBe('account');
    expect(findCategory('mail').id).toBe('mail');
  });

  it('findSection falls back to the category first section when the id is unknown', () => {
    const mail = findCategory('mail');
    expect(findSection(mail, undefined).id).toBe('signatures');
    expect(findSection(mail, 'nope').id).toBe('signatures');
    expect(findSection(mail, 'filters').id).toBe('filters');
  });
});

describe('SettingsHub', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('defaults to Account & Security tab when no category is in the URL', async () => {
    renderAt('/app/settings');
    await waitFor(() => {
      expect(screen.getByTestId('manager-security')).toBeInTheDocument();
    });
    expect(
      screen.getByTestId('settings-category-account').getAttribute('data-active'),
    ).toBe('true');
  });

  it('opens the Mail category when /app/settings/mail is loaded', async () => {
    renderAt('/app/settings/mail');
    await waitFor(() => {
      expect(screen.getByTestId('manager-signatures')).toBeInTheDocument();
    });
    expect(
      screen.getByTestId('settings-category-mail').getAttribute('data-active'),
    ).toBe('true');
  });

  it('opens the FilterManager pane when /app/settings/mail/filters is loaded', async () => {
    renderAt('/app/settings/mail/filters');
    await waitFor(() => {
      expect(screen.getByTestId('manager-filters')).toBeInTheDocument();
    });
    expect(
      screen.getByTestId('settings-section-filters').getAttribute('data-active'),
    ).toBe('true');
  });

  it('falls back to the first section when the section id is unknown', async () => {
    renderAt('/app/settings/mail/no-such-section');
    await waitFor(() => {
      expect(screen.getByTestId('manager-signatures')).toBeInTheDocument();
    });
  });

  it('navigates to a category first section when its rail tab is clicked', async () => {
    renderAt('/app/settings');
    await waitFor(() => {
      expect(screen.getByTestId('manager-security')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('settings-category-connections'));
    await waitFor(() => {
      expect(screen.getByTestId('location').textContent).toBe(
        '/app/settings/connections/smtp',
      );
    });
    await waitFor(() => {
      expect(screen.getByTestId('manager-smtp')).toBeInTheDocument();
    });
  });

  it('navigates within a category when a section button is clicked', async () => {
    renderAt('/app/settings/mail');
    await waitFor(() => {
      expect(screen.getByTestId('manager-signatures')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('settings-section-templates'));
    await waitFor(() => {
      expect(screen.getByTestId('location').textContent).toBe(
        '/app/settings/mail/templates',
      );
    });
    await waitFor(() => {
      expect(screen.getByTestId('manager-templates')).toBeInTheDocument();
    });
  });

  it('renders every category tab so all four are reachable from the rail', () => {
    renderAt('/app/settings');
    SETTINGS_CATEGORIES.forEach((cat) => {
      expect(screen.getByTestId(`settings-category-${cat.id}`)).toBeInTheDocument();
    });
  });
});
