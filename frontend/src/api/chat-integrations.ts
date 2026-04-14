// Added: Chat integration API client for team chat webhook management (TMAIL-129)

import { apiClient } from './client';

// Added: Chat platform types matching backend enum
export type ChatPlatform = 'slack' | 'teams' | 'google_chat' | 'discord' | 'custom';

export interface ChatIntegration {
  id: string;
  user_id: string;
  platform: ChatPlatform;
  webhook_url: string;
  channel_name: string | null;
  notify_on_receive: boolean;
  notify_on_send: boolean;
  notify_on_mention: boolean;
  filter_from: string | null;
  filter_subject: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateChatIntegrationRequest {
  platform: ChatPlatform;
  webhook_url: string;
  channel_name?: string;
  notify_on_receive?: boolean;
  notify_on_send?: boolean;
  notify_on_mention?: boolean;
  filter_from?: string;
  filter_subject?: string;
}

export interface UpdateChatIntegrationRequest {
  webhook_url?: string;
  channel_name?: string;
  notify_on_receive?: boolean;
  notify_on_send?: boolean;
  notify_on_mention?: boolean;
  filter_from?: string;
  filter_subject?: string;
  active?: boolean;
}

export interface TestResult {
  success: boolean;
  message: string;
}

// PURPOSE: List all chat integrations for the current user
export async function listChatIntegrations(): Promise<ChatIntegration[]> {
  return apiClient.get<ChatIntegration[]>('/chat-integrations');
}

// PURPOSE: Create a new chat integration
export async function createChatIntegration(data: CreateChatIntegrationRequest): Promise<ChatIntegration> {
  return apiClient.post<ChatIntegration>('/chat-integrations', data);
}

// PURPOSE: Get a single chat integration by ID
export async function getChatIntegration(id: string): Promise<ChatIntegration> {
  return apiClient.get<ChatIntegration>(`/chat-integrations/${id}`);
}

// PURPOSE: Update an existing chat integration
export async function updateChatIntegration(id: string, data: UpdateChatIntegrationRequest): Promise<ChatIntegration> {
  return apiClient.put<ChatIntegration>(`/chat-integrations/${id}`, data);
}

// PURPOSE: Delete a chat integration
export async function deleteChatIntegration(id: string): Promise<void> {
  await apiClient.delete(`/chat-integrations/${id}`);
}

// PURPOSE: Send a test notification to the chat integration's webhook
export async function testChatIntegration(id: string): Promise<TestResult> {
  return apiClient.post<TestResult>(`/chat-integrations/${id}/test`, {});
}
