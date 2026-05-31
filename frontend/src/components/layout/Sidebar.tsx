// Changed (TMAIL-398): registry-driven sidebar replaces the 41-button flat
// block. The top-level surface now stays ≤ 8 entries for a non-admin user
// (Compose + FolderTree + 4 apps + Settings) and gains a single Admin entry
// for admins. Every secondary manager moves behind the SettingsHub
// (/app/settings) and the AdminShell (/admin) in stories TMAIL-399 / 400.
import { PenSquare } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { FolderTree } from '../mail/FolderTree';
import { QuotaBar } from './QuotaBar';
import { useMailStore } from '../../stores/mailStore';
import { useResponsive } from '../../hooks/useResponsive';
import { useUiStore } from '../../stores/uiStore';
import { useAuth } from '../../hooks/useAuth';
import { visibleNavGroups, type NavItem } from './nav-registry';

export function Sidebar() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const viewMode = useMailStore((s) => s.viewMode);
  const { isMobile } = useResponsive();
  const setSidebarOpen = useUiStore((s) => s.setSidebarOpen);
  const { isAdmin } = useAuth();
  const navigate = useNavigate();

  // Closing the sidebar on mobile after navigation is a hard requirement —
  // TMAIL-33 wired this up so the user isn't left staring at the menu
  // after picking an entry.
  const closeOnMobile = () => {
    if (isMobile) setSidebarOpen(false);
  };

  const handleCompose = () => {
    setViewMode('compose');
    closeOnMobile();
  };

  const handleNavItem = (item: NavItem) => {
    if (item.href) {
      navigate(item.href);
    } else {
      // viewMode-driven items: cast is safe — items without href are typed
      // to NavViewMode by construction in nav-registry.ts.
      setViewMode(item.key as Parameters<typeof setViewMode>[0]);
    }
    closeOnMobile();
  };

  const groups = visibleNavGroups(isAdmin);

  return (
    <aside className="sidebar">
      <button className="btn btn--primary btn--compose" onClick={handleCompose}>
        <PenSquare size={18} />
        Compose
      </button>
      {/* FolderTree stays the visually dominant block — the Inbox row inside
          carries the folder-item--primary treatment (see FolderTree.tsx). */}
      <div className="sidebar__folders sidebar__folders--primary">
        <FolderTree />
      </div>
      {groups.map(({ group, items }) => (
        <div
          key={group}
          className={`sidebar__group sidebar__group--${group}`}
          data-testid={`sidebar-group-${group}`}
          style={{
            borderTop: '1px solid var(--color-border)',
            marginTop: '12px',
            paddingTop: '8px',
          }}
        >
          {items.map((item) => {
            const Icon = item.icon;
            const isActive = !item.href && viewMode === item.key;
            return (
              <button
                key={item.key}
                className={`folder-item ${isActive ? 'folder-item--active' : ''}`}
                data-nav-key={item.key}
                onClick={() => handleNavItem(item)}
              >
                <Icon size={18} />
                <span className="folder-item__name">{item.label}</span>
              </button>
            );
          })}
        </div>
      ))}
      <QuotaBar />
    </aside>
  );
}
