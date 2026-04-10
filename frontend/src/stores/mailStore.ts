import { create } from 'zustand';

type ViewMode = 'list' | 'reader' | 'compose' | 'search' | 'signatures' | 'contacts' | 'security' | 'vacation';

interface MailState {
  selectedFolder: string;
  selectedUid: number | null;
  viewMode: ViewMode;
  searchQuery: string;
  setSelectedFolder: (folder: string) => void;
  setSelectedUid: (uid: number | null) => void;
  setViewMode: (mode: ViewMode) => void;
  setSearchQuery: (query: string) => void;
}

export const useMailStore = create<MailState>((set) => ({
  selectedFolder: 'INBOX',
  selectedUid: null,
  viewMode: 'list',
  searchQuery: '',
  setSelectedFolder: (folder) => set({ selectedFolder: folder, selectedUid: null, viewMode: 'list' }),
  setSelectedUid: (uid) => set({ selectedUid: uid, viewMode: uid ? 'reader' : 'list' }),
  setViewMode: (mode) => set({ viewMode: mode }),
  setSearchQuery: (query) => set({ searchQuery: query, viewMode: query ? 'search' : 'list' }),
}));
