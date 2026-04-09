import { useQuery } from '@tanstack/react-query';
import { fetchFolders } from '../api/folders';
import { fetchMessages, fetchMessage } from '../api/messages';
import { useMailStore } from '../stores/mailStore';

export function useFolders() {
  return useQuery({
    queryKey: ['folders'],
    queryFn: fetchFolders,
    staleTime: 30_000,
  });
}

export function useMessages(folder: string, page = 0, pageSize = 50) {
  return useQuery({
    queryKey: ['messages', folder, page, pageSize],
    queryFn: () => fetchMessages(folder, page, pageSize),
    staleTime: 15_000,
    enabled: !!folder,
  });
}

export function useMessage(folder: string, uid: number | null) {
  return useQuery({
    queryKey: ['message', folder, uid],
    queryFn: () => fetchMessage(folder, uid!),
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
