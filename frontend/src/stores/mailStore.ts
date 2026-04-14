import { create } from 'zustand';
import type { AdvancedSearchParams } from '../api/messages';

type ViewMode = 'list' | 'reader' | 'compose' | 'search' | 'signatures' | 'contacts' | 'security' | 'vacation' | 'groups' | 'migration' | 'bandwidth' | 'filters';

interface MailState {
  selectedFolder: string;
  selectedUid: number | null;
  viewMode: ViewMode;
  searchQuery: string;
  // Added: Advanced search filter state for TMAIL-32
  advancedSearch: AdvancedSearchParams | null;
  setSelectedFolder: (folder: string) => void;
  setSelectedUid: (uid: number | null) => void;
  setViewMode: (mode: ViewMode) => void;
  setSearchQuery: (query: string) => void;
  setAdvancedSearch: (params: AdvancedSearchParams | null) => void;
}

export const useMailStore = create<MailState>((set) => ({
  selectedFolder: 'INBOX',
  selectedUid: null,
  viewMode: 'list',
  searchQuery: '',
  advancedSearch: null,
  setSelectedFolder: (folder) => set({ selectedFolder: folder, selectedUid: null, viewMode: 'list' }),
  setSelectedUid: (uid) => set({ selectedUid: uid, viewMode: uid ? 'reader' : 'list' }),
  setViewMode: (mode) => set({ viewMode: mode }),
  setSearchQuery: (query) => set({ searchQuery: query, viewMode: query ? 'search' : 'list' }),
  // Added: Set advanced search params and switch to search view
  setAdvancedSearch: (params) => set({
    advancedSearch: params,
    viewMode: params ? 'search' : 'list',
  }),
}));
