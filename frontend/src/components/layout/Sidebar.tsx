// Added: Mailbox icon for shared mailboxes sidebar entry (TMAIL-96)
import { PenSquare, FileSignature, Users, Shield, Plane, UsersRound, Upload, Gauge, Filter, Mailbox } from 'lucide-react';
import { FolderTree } from '../mail/FolderTree';
import { QuotaBar } from './QuotaBar';
import { useMailStore } from '../../stores/mailStore';

export function Sidebar() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const viewMode = useMailStore((s) => s.viewMode);

  return (
    <aside className="sidebar">
      <button className="btn btn--primary btn--compose" onClick={() => setViewMode('compose')}>
        <PenSquare size={18} />
        Compose
      </button>
      <FolderTree />
      <div style={{ borderTop: '1px solid var(--color-border)', marginTop: '12px', paddingTop: '8px' }}>
        <button
          className={`folder-item ${viewMode === 'signatures' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('signatures')}
        >
          <FileSignature size={18} />
          <span className="folder-item__name">Signatures</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'contacts' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('contacts')}
        >
          <Users size={18} />
          <span className="folder-item__name">Contacts</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'security' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('security')}
        >
          <Shield size={18} />
          <span className="folder-item__name">Security</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'vacation' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('vacation')}
        >
          <Plane size={18} />
          <span className="folder-item__name">Vacation</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'groups' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('groups')}
        >
          <UsersRound size={18} />
          <span className="folder-item__name">Groups</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'migration' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('migration')}
        >
          <Upload size={18} />
          <span className="folder-item__name">Migration</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'filters' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('filters')}
        >
          <Filter size={18} />
          <span className="folder-item__name">Filters</span>
        </button>
        {/* Added: Shared mailboxes navigation entry (TMAIL-96) */}
        <button
          className={`folder-item ${viewMode === 'shared' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('shared')}
        >
          <Mailbox size={18} />
          <span className="folder-item__name">Shared Mailboxes</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'bandwidth' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('bandwidth')}
        >
          <Gauge size={18} />
          <span className="folder-item__name">Bandwidth</span>
        </button>
      </div>
      <QuotaBar />
    </aside>
  );
}
