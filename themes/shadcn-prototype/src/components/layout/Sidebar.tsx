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
import { mockFolders } from '@/data/mockData';
import type { Folder } from '@/data/mockData';

interface SidebarProps {
  activeFolder: string;
  onFolderChange: (folderId: string) => void;
  onCompose: () => void;
  // TMAIL-217: when supplied, drives the folder list (real /api/folders).
  // When omitted (standalone dev mode), falls back to mockFolders.
  folders?: Folder[];
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

export function Sidebar({ activeFolder, onFolderChange, onCompose, folders: foldersProp }: SidebarProps) {
  const [collapsed, setCollapsed] = useState(false);
  const location = useLocation();
  const isCalendar = location.pathname === '/calendar';
  // TMAIL-217 / TMAIL-228: when the parent supplies real folders, render
  // directly from that prop (derived state). Local state was racing the
  // /api/folders query — the Sidebar mounted with an empty array on the
  // first render, captured that into useState, and never re-rendered when
  // the prop later resolved. Falling back to the mock list only when the
  // prop is absent (standalone dev run).
  const liveFolders: Folder[] = foldersProp ?? mockFolders;
  // Local state is kept ONLY for the inline new-folder addition flow which
  // exists on top of the live list (real backend additions need a separate
  // endpoint — out of scope here).
  const [extraLocalFolders, setExtraLocalFolders] = useState<Folder[]>([]);
  const folders: Folder[] = [...liveFolders, ...extraLocalFolders];
  const [isAddingFolder, setIsAddingFolder] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');

  const handleAddFolder = () => {
    if (newFolderName.trim()) {
      const newFolder: Folder = {
        id: newFolderName.toLowerCase().replace(/\s+/g, '-'),
        name: newFolderName,
        icon: 'Briefcase',
        count: 0,
        isCustom: true,
      };
      // TMAIL-228: only the local-only "extra" list mutates here. Real
      // folders come from /api/folders via the parent prop and are
      // immutable from the sidebar — adding/deleting a real folder needs
      // a separate backend call (out of scope here).
      setExtraLocalFolders((prev) => [...prev, newFolder]);
      setNewFolderName('');
      setIsAddingFolder(false);
    }
  };

  const handleDeleteFolder = (folderId: string) => {
    setExtraLocalFolders((prev) => prev.filter((f) => f.id !== folderId));
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
              className="flex-1 px-2 py-1 text-sm bg-zinc-50 dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded"
              autoFocus
            />
          </div>
        ) : (
          <Button
            variant="ghost"
            className="w-full justify-start mb-1 text-zinc-500"
            onClick={() => setIsAddingFolder(true)}
          >
            <Plus className="size-4 mr-3" />
            <span>New folder</span>
          </Button>
        )}
      </div>

      <div className="flex-1" />
      <div className="p-4 border-t border-zinc-200 dark:border-zinc-800 sticky bottom-0 bg-white dark:bg-zinc-950">
        <div className="flex items-center gap-3">
          <Avatar>
            <AvatarFallback className="bg-blue-600 text-white">ME</AvatarFallback>
          </Avatar>
          <div className="flex-1 min-w-0">
            <div className="text-sm font-medium truncate">me@mydomain.com</div>
            <div className="text-xs text-zinc-500">Personal Account</div>
          </div>
        </div>
      </div>
    </div>
  );
}
