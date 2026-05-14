import type { Folder } from '../types/mail';
import { apiClient } from './client';

export async function fetchFolders(): Promise<Folder[]> {
  return apiClient.get<Folder[]>('/folders');
}
