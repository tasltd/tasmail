import { Inbox, Send, FileText, Trash2, AlertCircle, Folder as FolderIcon } from 'lucide-react';
import { useFolders } from '../../hooks/useMailbox';
import { useMailStore } from '../../stores/mailStore';
import type { Folder } from '../../types/mail';

const FOLDER_ICONS: Record<string, typeof Inbox> = {
  INBOX: Inbox,
  Sent: Send,
  Drafts: FileText,
  Trash: Trash2,
  Junk: AlertCircle,
};

function FolderItem({ folder }: { folder: Folder }) {
  const selectedFolder = useMailStore((s) => s.selectedFolder);
  const setSelectedFolder = useMailStore((s) => s.setSelectedFolder);

  const Icon = FOLDER_ICONS[folder.name] || FolderIcon;
  const isActive = selectedFolder === folder.name;

  return (
    <button
      className={`folder-item ${isActive ? 'folder-item--active' : ''}`}
      onClick={() => setSelectedFolder(folder.name)}
    >
      <Icon size={18} />
      <span className="folder-item__name">{folder.name}</span>
      {folder.unseen != null && folder.unseen > 0 && (
        <span className="folder-item__badge">{folder.unseen}</span>
      )}
    </button>
  );
}

export function FolderTree() {
  const { data: folders, isLoading, error } = useFolders();

  if (isLoading) {
    return <div className="folder-tree folder-tree--loading">Loading folders...</div>;
  }

  if (error) {
    return <div className="folder-tree folder-tree--error">Failed to load folders</div>;
  }

  return (
    <nav className="folder-tree">
      {folders?.map((folder) => (
        <FolderItem key={folder.name} folder={folder} />
      ))}
    </nav>
  );
}
