import { useState } from 'react';
import { Link, useLocation } from 'react-router';
import {
  Inbox,
  Send,
  FileText,
  AlertOctagon,
  Trash2,
  Briefcase,
  User,
  ChevronLeft,
  ChevronRight,
  Plus,
  X,
  CalendarDays,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import type { Folder } from '@/types/ui';
import { decodeAccessClaims } from '@/lib/jwt';

interface SidebarProps {
  activeFolder: string;
  onFolderChange: (folderId: string) => void;
  onCompose: () => void;
  // TMAIL-217: when supplied, drives the folder list (real /api/folders).
  // When omitted (no data yet), shows an empty list — never any seed data.
  folders?: Folder[];
  // TMAIL-324: callbacks now go to the parent which runs the real
  // POST/DELETE /api/folders mutations and invalidates the ['folders'] query.
  // Local state is gone — the sidebar is a pure view over the live list.
  onAddFolder?: (name: string) => void;
  onDeleteFolder?: (folderId: string) => void;
  isAddingFolderPending?: boolean;
}

const iconMap: Record<string, any> = {
  Inbox,
  Send,
  FileText,
  AlertOctagon,
  Trash2,
  Briefcase,
  User
};

export function Sidebar({
  activeFolder,
  onFolderChange,
  onCompose,
  folders: foldersProp,
  onAddFolder,
  onDeleteFolder,
  isAddingFolderPending,
}: SidebarProps) {
  const [collapsed, setCollapsed] = useState(false);
  const location = useLocation();
  const isCalendar = location.pathname === '/calendar';
  // TMAIL-217 / TMAIL-228 / TMAIL-239 / TMAIL-324: render directly from the
  // prop. The list is the source of truth — no parallel client-side state.
  const folders: Folder[] = foldersProp ?? [];
  const [isAddingFolder, setIsAddingFolder] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');

  // TMAIL-324: submit the inline new-folder input. Defers the real network
  // call to the parent's onAddFolder mutation; the parent invalidates the
  // ['folders'] query so the list refreshes from the backend. The form is
  // cleared immediately so the input doesn't feel stuck while the request
  // is in flight.
  const handleAddFolder = () => {
    const trimmed = newFolderName.trim();
    if (trimmed && onAddFolder) {
      onAddFolder(trimmed);
    }
    setNewFolderName('');
    setIsAddingFolder(false);
  };

  // TMAIL-324: delete is now a parent-driven mutation that invalidates the
  // ['folders'] query on settle. If the currently-active folder is deleted,
  // bounce navigation back to the inbox so the message list isn't pointing
  // at a folder that no longer exists.
  const handleDeleteFolder = (folderId: string) => {
    if (onDeleteFolder) {
      onDeleteFolder(folderId);
    }
    if (activeFolder === folderId) {
      onFolderChange('inbox');
    }
  };

  if (collapsed) {
    return (
      <div className="w-16 border-r border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 flex flex-col items-center py-4 gap-2">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setCollapsed(false)}
          className="mb-2"
        >
          <ChevronRight className="size-4" />
        </Button>
        {folders.slice(0, 5).map((folder) => {
          const Icon = iconMap[folder.icon] ?? Briefcase;
          return (
            <Button
              key={folder.id}
              variant={activeFolder === folder.id ? 'default' : 'ghost'}
              size="icon"
              onClick={() => onFolderChange(folder.id)}
              className="relative"
            >
              <Icon className="size-4" />
              {folder.count > 0 && (
                <span className="absolute -top-1 -right-1 size-4 bg-blue-600 text-white text-xs rounded-full flex items-center justify-center">
                  {folder.count}
                </span>
              )}
            </Button>
          );
        })}
        <Link to="/calendar">
          <Button
            variant={isCalendar ? 'default' : 'ghost'}
            size="icon"
            title="Calendar"
          >
            <CalendarDays className="size-4" />
          </Button>
        </Link>
      </div>
    );
  }

  return (
    <div className="w-64 border-r border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 flex flex-col h-full overflow-y-auto">
      <div className="p-4 space-y-2">
        <div className="flex items-center justify-between mb-2">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setCollapsed(true)}
          >
            <ChevronLeft className="size-4" />
          </Button>
        </div>

        <Button onClick={onCompose} className="w-full">
          <Plus className="size-4 mr-2" />
          Compose
        </Button>
      </div>

      <div className="px-2">
        {folders.map((folder) => {
          // TMAIL-228: defensive — fall back to Briefcase if the folder.icon
          // string isn't in iconMap. Prevents the whole list from crashing
          // (React would error-boundary the parent fragment) when a real
          // backend folder name doesn't map cleanly.
          const Icon = iconMap[folder.icon] ?? Briefcase;
          return (
            <div key={folder.id} className="group relative">
              <Button
                variant={activeFolder === folder.id ? 'secondary' : 'ghost'}
                className="w-full justify-start mb-1"
                onClick={() => onFolderChange(folder.id)}
              >
                <Icon className="size-4 mr-3" />
                <span className="flex-1 text-left">{folder.name}</span>
                {folder.count > 0 && (
                  <span className="text-xs bg-blue-600 text-white px-2 py-0.5 rounded-full">
                    {folder.count}
                  </span>
                )}
              </Button>
              {folder.isCustom && (
                <Button
                  variant="ghost"
                  size="icon"
                  className="absolute right-1 top-1 size-6 opacity-0 group-hover:opacity-100"
                  data-testid={`delete-folder-${folder.id}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    handleDeleteFolder(folder.id);
                  }}
                >
                  <X className="size-3" />
                </Button>
              )}
            </div>
          );
        })}

        {/* Calendar Link */}
        <Link to="/calendar">
          <Button
            variant={isCalendar ? 'secondary' : 'ghost'}
            className="w-full justify-start mb-1"
          >
            <CalendarDays className="size-4 mr-3" />
            <span className="flex-1 text-left">Calendar</span>
          </Button>
        </Link>

        {isAddingFolder ? (
          <div className="flex items-center gap-1 px-2 py-1">
            <input
              type="text"
              value={newFolderName}
              onChange={(e) => setNewFolderName(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleAddFolder()}
              onBlur={handleAddFolder}
              placeholder="Folder name"
              data-testid="new-folder-input"
              disabled={isAddingFolderPending}
              className="flex-1 px-2 py-1 text-sm bg-zinc-50 dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded disabled:opacity-50"
              autoFocus
            />
          </div>
        ) : (
          <Button
            variant="ghost"
            className="w-full justify-start mb-1 text-zinc-500"
            data-testid="new-folder-button"
            onClick={() => setIsAddingFolder(true)}
          >
            <Plus className="size-4 mr-3" />
            <span>New folder</span>
          </Button>
        )}
      </div>

      <div className="flex-1" />
      <SidebarFooter />
    </div>
  );
}

// TMAIL-239: footer shows the signed-in user from the JWT instead of the
// hardcoded "me@mydomain.com" mock identity.
function SidebarFooter() {
  const claims = decodeAccessClaims();
  const username = claims?.username ?? claims?.sub ?? 'unknown';
  const initials = username
    .split(/[@._-]/)
    .filter(Boolean)
    .slice(0, 2)
    .map((s) => s[0]?.toUpperCase() ?? '')
    .join('') || 'U';
  return (
    <div className="p-4 border-t border-zinc-200 dark:border-zinc-800 sticky bottom-0 bg-white dark:bg-zinc-950">
      <div className="flex items-center gap-3">
        <Avatar>
          <AvatarFallback className="bg-blue-600 text-white">{initials}</AvatarFallback>
        </Avatar>
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium truncate">{username}</div>
          <div className="text-xs text-zinc-500">{claims?.is_admin ? 'Admin · TASMail' : 'TASMail'}</div>
        </div>
      </div>
    </div>
  );
}
