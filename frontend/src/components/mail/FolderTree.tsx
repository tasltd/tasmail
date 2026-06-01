import { useCallback } from 'react';
import { Inbox, Send, FileText, Trash2, AlertCircle, Folder as FolderIcon } from 'lucide-react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useFolders } from '../../hooks/useMailbox';
import { useMailStore } from '../../stores/mailStore';
// Added: Import drop hook and move API for drag-and-drop (TMAIL-122)
import { useFolderDrop, type DragData } from '../../hooks/useDragAndDrop';
import { moveMessage } from '../../api/messages';
// Fix (TMAIL-414): close the mobile sidebar after folder selection so users
// aren't left staring at the overlay (matches Sidebar.tsx's closeOnMobile
// behaviour for Compose / nav items, originally wired in TMAIL-33).
import { useResponsive } from '../../hooks/useResponsive';
import { useUiStore } from '../../stores/uiStore';
import type { Folder } from '../../types/mail';

const FOLDER_ICONS: Record<string, typeof Inbox> = {
  INBOX: Inbox,
  Sent: Send,
  Drafts: FileText,
  Trash: Trash2,
  Junk: AlertCircle,
};

function FolderItem({ folder, onAfterSelect }: { folder: Folder; onAfterSelect?: () => void }) {
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
  // Added (TMAIL-398): Inbox row is the visually dominant entry per the
  // grouped-sidebar redesign — bold/larger row marks it as the primary
  // mail destination so first-time users find it immediately.
  const isPrimary = folder.name === 'INBOX';

  return (
    <button
      // Changed: Added drop handlers and drop-target class for visual feedback (TMAIL-122)
      // Added (TMAIL-401): data-tour="inbox" anchors the FirstLoginTour step 2 to the Inbox row.
      className={`folder-item ${isActive ? 'folder-item--active' : ''} ${isPrimary ? 'folder-item--primary' : ''} ${isOver ? 'folder-item--drop-target' : ''}`}
      onClick={() => {
        setSelectedFolder(folder.name);
        // Fix (TMAIL-414): dismiss the mobile sidebar overlay after picking a folder.
        onAfterSelect?.();
      }}
      data-tour={isPrimary ? 'inbox' : undefined}
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
  // Fix (TMAIL-414): subscribe once at the tree level so each FolderItem
  // shares the same close handler — one matchMedia listener, one store hook.
  const { isMobile } = useResponsive();
  const setSidebarOpen = useUiStore((s) => s.setSidebarOpen);
  const closeOnMobile = useCallback(() => {
    if (isMobile) setSidebarOpen(false);
  }, [isMobile, setSidebarOpen]);

  if (isLoading) {
    return <div className="folder-tree folder-tree--loading">Loading folders...</div>;
  }

  if (error) {
    return <div className="folder-tree folder-tree--error">Failed to load folders</div>;
  }

  return (
    <nav className="folder-tree">
      {folders?.map((folder) => (
        <FolderItem key={folder.name} folder={folder} onAfterSelect={closeOnMobile} />
      ))}
    </nav>
  );
}
