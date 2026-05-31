// Added (TMAIL-399): Gmail-style two-pane Settings page mounted at
// /app/settings/:category?/:section?. The Sidebar's Settings entry (TMAIL-398
// nav-registry) navigates here; deep links like /app/settings/mail/filters
// open straight to the relevant pane.
//
// The hub itself is dumb — it iterates settings-hub-registry.ts to render
// the left-rail category tabs and the section list inside the selected
// category, then lazy-loads the matching manager component into the right
// pane. Every managed setting moves through this registry; the hub never
// imports a manager directly.
import { Suspense, useMemo } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import {
  SETTINGS_CATEGORIES,
  findCategory,
  findSection,
} from './settings-hub-registry';

// Parse the trailing :category/:section out of the pathname. Using a splat
// route in App.tsx and parsing here keeps App.tsx free of nested-route
// boilerplate per category.
function parseSettingsPath(pathname: string): { categoryId?: string; sectionId?: string } {
  const trimmed = pathname.replace(/^\/app\/settings\/?/, '');
  const segments = trimmed.split('/').filter(Boolean);
  return { categoryId: segments[0], sectionId: segments[1] };
}

function HubLoading() {
  return <div className="settings-hub__loading">Loading…</div>;
}

export function SettingsHub() {
  const location = useLocation();
  const navigate = useNavigate();

  const { categoryId, sectionId } = parseSettingsPath(location.pathname);
  const category = useMemo(() => findCategory(categoryId), [categoryId]);
  const section = useMemo(() => findSection(category, sectionId), [category, sectionId]);
  const SectionComponent = section.component;

  const selectCategory = (id: string) => {
    // When the user switches category we land on its first section so the
    // right pane is never empty after a tab click.
    const next = findCategory(id);
    navigate(`/app/settings/${next.id}/${next.sections[0].id}`);
  };

  const selectSection = (id: string) => {
    navigate(`/app/settings/${category.id}/${id}`);
  };

  return (
    <div
      className="settings-hub"
      data-testid="settings-hub"
      style={{
        display: 'flex',
        height: '100%',
        minHeight: 0,
        background: 'var(--color-bg)',
      }}
    >
      <aside
        className="settings-hub__rail"
        data-testid="settings-hub-rail"
        style={{
          width: 240,
          flexShrink: 0,
          borderRight: '1px solid var(--color-border)',
          overflowY: 'auto',
          padding: '16px 8px',
        }}
      >
        <h2
          style={{
            fontSize: 18,
            margin: '0 8px 12px',
            color: 'var(--color-text)',
          }}
        >
          Settings
        </h2>
        {SETTINGS_CATEGORIES.map((cat) => {
          const isActive = cat.id === category.id;
          return (
            <div key={cat.id} style={{ marginBottom: 12 }}>
              <button
                type="button"
                data-testid={`settings-category-${cat.id}`}
                data-active={isActive}
                onClick={() => selectCategory(cat.id)}
                style={{
                  width: '100%',
                  textAlign: 'left',
                  padding: '8px 12px',
                  background: isActive ? 'var(--color-bg-hover)' : 'transparent',
                  border: 'none',
                  borderRadius: 6,
                  color: 'var(--color-text)',
                  fontWeight: isActive ? 600 : 500,
                  cursor: 'pointer',
                }}
              >
                {cat.label}
              </button>
              {isActive && (
                <ul
                  style={{
                    listStyle: 'none',
                    margin: '4px 0 0',
                    padding: '0 0 0 8px',
                  }}
                >
                  {cat.sections.map((sec) => {
                    const isSecActive = sec.id === section.id;
                    return (
                      <li key={sec.id}>
                        <button
                          type="button"
                          data-testid={`settings-section-${sec.id}`}
                          data-active={isSecActive}
                          onClick={() => selectSection(sec.id)}
                          style={{
                            width: '100%',
                            textAlign: 'left',
                            padding: '6px 12px',
                            background: isSecActive
                              ? 'var(--color-accent-bg, var(--color-bg-hover))'
                              : 'transparent',
                            border: 'none',
                            borderRadius: 6,
                            color: 'var(--color-text)',
                            fontSize: 14,
                            cursor: 'pointer',
                          }}
                        >
                          {sec.label}
                        </button>
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>
          );
        })}
      </aside>
      <section
        className="settings-hub__pane"
        data-testid="settings-hub-pane"
        data-section={section.id}
        style={{ flex: 1, minWidth: 0, overflowY: 'auto', padding: 16 }}
      >
        <Suspense fallback={<HubLoading />}>
          <SectionComponent />
        </Suspense>
      </section>
    </div>
  );
}
