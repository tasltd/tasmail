import type { FullMessage, MessageListResponse, SendEmailRequest } from '../types/mail';
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
