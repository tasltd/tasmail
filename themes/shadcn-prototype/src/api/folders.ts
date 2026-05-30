import type { Folder } from '../types/mail';
import { apiClient } from './client';

export async function fetchFolders(): Promise<Folder[]> {
  return apiClient.get<Folder[]>('/folders');
}

// TMAIL-324: real CRUD against POST/DELETE /api/folders. Replaces the prior
// local-only `extraLocalFolders` state in Sidebar.tsx so folder add/delete
// persists to the user's IMAP server and survives reloads.
export async function createFolder(name: string): Promise<Folder> {
  return apiClient.post<Folder>('/folders', { name });
}

export async function deleteFolder(name: string): Promise<void> {
  return apiClient.delete<void>(`/folders/${encodeURIComponent(name)}`);
}
