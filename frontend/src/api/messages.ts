import type { FullMessage, MessageListResponse, SearchResponse, SendEmailRequest } from '../types/mail';
import { apiClient } from './client';

export async function fetchMessages(
  folder: string,
  page = 0,
  pageSize = 50,
): Promise<MessageListResponse> {
  return apiClient.get<MessageListResponse>(
    `/folders/${encodeURIComponent(folder)}/messages?page=${page}&page_size=${pageSize}`,
  );
}

export async function fetchMessage(
  folder: string,
  uid: number,
): Promise<FullMessage> {
  return apiClient.get<FullMessage>(
    `/folders/${encodeURIComponent(folder)}/messages/${uid}`,
  );
}

export async function sendMessage(request: SendEmailRequest): Promise<void> {
  await apiClient.post('/messages/send', request);
}

export async function searchMessages(
  query: string,
  folder?: string,
): Promise<SearchResponse> {
  const params = new URLSearchParams({ q: query });
  if (folder) params.set('folder', folder);
  return apiClient.get<SearchResponse>(`/search?${params.toString()}`);
}

export async function deleteMessage(folder: string, uid: number): Promise<void> {
  await apiClient.delete(`/folders/${encodeURIComponent(folder)}/messages/${uid}`);
}

export async function moveMessage(
  folder: string,
  uid: number,
  toFolder: string,
): Promise<void> {
  await apiClient.post(`/folders/${encodeURIComponent(folder)}/messages/${uid}/move`, {
    to_folder: toFolder,
  });
}

export async function flagMessage(
  folder: string,
  uid: number,
  flag: string,
  add: boolean,
): Promise<void> {
  await apiClient.post(`/folders/${encodeURIComponent(folder)}/messages/${uid}/flag`, {
    flag,
    add,
  });
}

export interface SaveDraftRequest {
  to: string[];
  cc?: string[];
  subject: string;
  html_body?: string;
  text_body?: string;
}

export async function saveDraft(request: SaveDraftRequest): Promise<void> {
  await apiClient.post('/drafts', request);
}
