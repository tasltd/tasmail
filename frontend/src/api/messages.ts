import type { FullMessage, MessageListResponse, SearchResponse, SendEmailRequest } from '../types/mail';
import { apiClient } from './client';

// Added: Advanced search filter parameters for TMAIL-32
export interface AdvancedSearchParams {
  query: string;
  folder?: string;
  from?: string;
  to?: string;
  subject?: string;
  dateFrom?: string;  // ISO date string (YYYY-MM-DD)
  dateTo?: string;    // ISO date string (YYYY-MM-DD)
  hasAttachment?: boolean;
  isUnread?: boolean;
  isStarred?: boolean;
}

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

// Changed: Accept AdvancedSearchParams or simple query+folder for backward compatibility
export async function searchMessages(
  queryOrParams: string | AdvancedSearchParams,
  folder?: string,
): Promise<SearchResponse> {
  const params = new URLSearchParams();

  if (typeof queryOrParams === 'string') {
    // NOTE: Legacy call signature — simple query string
    params.set('q', queryOrParams);
    if (folder) params.set('folder', folder);
  } else {
    // Added: Serialize all non-empty advanced search params
    const advancedSearchParams = queryOrParams;
    if (advancedSearchParams.query) params.set('q', advancedSearchParams.query);
    if (advancedSearchParams.folder) params.set('folder', advancedSearchParams.folder);
    if (advancedSearchParams.from) params.set('from', advancedSearchParams.from);
    if (advancedSearchParams.to) params.set('to', advancedSearchParams.to);
    if (advancedSearchParams.subject) params.set('subject', advancedSearchParams.subject);
    if (advancedSearchParams.dateFrom) params.set('date_from', advancedSearchParams.dateFrom);
    if (advancedSearchParams.dateTo) params.set('date_to', advancedSearchParams.dateTo);
    if (advancedSearchParams.hasAttachment) params.set('has_attachment', 'true');
    if (advancedSearchParams.isUnread) params.set('is_unread', 'true');
    if (advancedSearchParams.isStarred) params.set('is_starred', 'true');
  }

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
