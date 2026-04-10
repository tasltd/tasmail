import { useQuery } from '@tanstack/react-query';
import { fetchFolders } from '../api/folders';
import { fetchMessages, fetchMessage, searchMessages } from '../api/messages';
import { useMailStore } from '../stores/mailStore';
import { offlineCache } from '../utils/offline-cache';
import type { Folder, MessageListResponse, FullMessage } from '../types/mail';

// Added: Network-first with IndexedDB fallback for offline support
async function fetchFoldersWithCache(): Promise<Folder[]> {
  try {
    const data = await fetchFolders();
    offlineCache.cacheFolders(data);
    return data;
  } catch (err) {
    const cached = await offlineCache.getFolders();
    if (cached) return cached as Folder[];
    throw err;
  }
}

async function fetchMessagesWithCache(
  folder: string,
  page: number,
  pageSize: number,
): Promise<MessageListResponse> {
  try {
    const data = await fetchMessages(folder, page, pageSize);
    offlineCache.cacheMessages(folder, page, data);
    return data;
  } catch (err) {
    const cached = await offlineCache.getMessages(folder, page);
    if (cached) return cached as MessageListResponse;
    throw err;
  }
}

async function fetchMessageWithCache(
  folder: string,
  uid: number,
): Promise<FullMessage> {
  try {
    const data = await fetchMessage(folder, uid);
    offlineCache.cacheFullMessage(folder, uid, data);
    return data;
  } catch (err) {
    const cached = await offlineCache.getFullMessage(folder, uid);
    if (cached) return cached as FullMessage;
    throw err;
  }
}

export function useFolders() {
  return useQuery({
    queryKey: ['folders'],
    queryFn: fetchFoldersWithCache,
    staleTime: 30_000,
  });
}

export function useMessages(folder: string, page = 0, pageSize = 50) {
  return useQuery({
    queryKey: ['messages', folder, page, pageSize],
    queryFn: () => fetchMessagesWithCache(folder, page, pageSize),
    staleTime: 15_000,
    enabled: !!folder,
  });
}

export function useMessage(folder: string, uid: number | null) {
  return useQuery({
    queryKey: ['message', folder, uid],
    queryFn: () => fetchMessageWithCache(folder, uid!),
    enabled: !!folder && uid != null,
  });
}

export function useCurrentMessages() {
  const folder = useMailStore((s) => s.selectedFolder);
  return useMessages(folder);
}

export function useCurrentMessage() {
  const folder = useMailStore((s) => s.selectedFolder);
  const uid = useMailStore((s) => s.selectedUid);
  return useMessage(folder, uid);
}

// Added: Search hook with debounced query
export function useSearch(query: string, folder?: string) {
  return useQuery({
    queryKey: ['search', query, folder],
    queryFn: () => searchMessages(query, folder),
    enabled: query.length >= 2,
    staleTime: 30_000,
  });
}
