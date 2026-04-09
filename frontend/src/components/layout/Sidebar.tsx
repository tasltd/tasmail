import { PenSquare } from 'lucide-react';
import { FolderTree } from '../mail/FolderTree';
import { useMailStore } from '../../stores/mailStore';

export function Sidebar() {
  const setViewMode = useMailStore((s) => s.setViewMode);

  return (
    <aside className="sidebar">
      <button className="btn btn--primary btn--compose" onClick={() => setViewMode('compose')}>
        <PenSquare size={18} />
        Compose
      </button>
      <FolderTree />
    </aside>
  );
}
