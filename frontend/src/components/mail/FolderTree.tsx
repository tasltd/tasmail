import { useCallback } from 'react';
import { Inbox, Send, FileText, Trash2, AlertCircle, Folder as FolderIcon } from 'lucide-react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useFolders } from '../../hooks/useMailbox';
import { useMailStore } from '../../stores/mailStore';
// Added: Import drop hook and move API for drag-and-drop (TMAIL-122)
import { useFolderDrop, type DragData } from '../../hooks/useDragAndDrop';
import { moveMessage } from '../../api/messages';
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
  const queryClient = useQueryClient();

  // Added: Mutation for moving messages via drag-and-drop (TMAIL-122)
  const moveMutation = useMutation({
    mutationFn: (dragData: DragData) =>
      moveMessage(dragData.folder, dragData.uid, folder.name),
    onSuccess: (_result, dragData) => {
      // NOTE: Invalidate both source and target folder message queries
      queryClient.invalidateQueries({ queryKey: ['messages', dragData.folder] });
      queryClient.invalidateQueries({ queryKey: ['messages', folder.name] });
      queryClient.invalidateQueries({ queryKey: ['folders'] });
    },
  });

  // Added: Drop handler that triggers the move mutation
  const handleDrop = useCallback(
    (dragData: DragData) => {
      moveMutation.mutate(dragData);
    },
    [moveMutation],
  );

  // Added: Folder drop zone handlers (TMAIL-122)
  const { isOver, ...dropHandlers } = useFolderDrop(folder.name, handleDrop);

  const Icon = FOLDER_ICONS[folder.name] || FolderIcon;
  const isActive = selectedFolder === folder.name;

  return (
    <button
      // Changed: Added drop handlers and drop-target class for visual feedback (TMAIL-122)
      className={`folder-item ${isActive ? 'folder-item--active' : ''} ${isOver ? 'folder-item--drop-target' : ''}`}
      onClick={() => setSelectedFolder(folder.name)}
      {...dropHandlers}
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
