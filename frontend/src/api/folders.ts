import type { Folder } from '../types/mail';
import { apiClient } from './client';

export async function fetchFolders(): Promise<Folder[]> {
  return apiClient.get<Folder[]>('/folders');
}

// TMAIL-324: create / delete IMAP mailboxes via the user's BYOK server.
// The alt-UI (themes/shadcn-prototype) consumes these via its own copy of
// folders.ts; the classic SPA exposes them here so the routes are not orphaned
// from the static traceability scan and so future classic-SPA folder CRUD has
// a single place to call.
export async function createFolder(name: string): Promise<Folder> {
  return apiClient.post<Folder>('/folders', { name });
}

export async function deleteFolder(name: string): Promise<void> {
  return apiClient.delete<void>(`/folders/${encodeURIComponent(name)}`);
}
