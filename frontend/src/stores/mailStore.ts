import { create } from 'zustand';

interface MailState {
  selectedFolder: string;
  selectedUid: number | null;
  viewMode: 'list' | 'reader' | 'compose';
  setSelectedFolder: (folder: string) => void;
  setSelectedUid: (uid: number | null) => void;
  setViewMode: (mode: 'list' | 'reader' | 'compose') => void;
}

export const useMailStore = create<MailState>((set) => ({
  selectedFolder: 'INBOX',
  selectedUid: null,
  viewMode: 'list',
  setSelectedFolder: (folder) => set({ selectedFolder: folder, selectedUid: null, viewMode: 'list' }),
  setSelectedUid: (uid) => set({ selectedUid: uid, viewMode: uid ? 'reader' : 'list' }),
  setViewMode: (mode) => set({ viewMode: mode }),
}));
