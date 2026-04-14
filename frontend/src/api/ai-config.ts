// Added: AI configuration API client for BYOK AI integration (TMAIL-105)

import { apiClient } from './client';

// Added: AI provider types matching backend enum
export type AiProvider = 'openai' | 'anthropic' | 'google' | 'ollama' | 'custom';

export interface AiConfigurationResponse {
  id: string;
  user_id: string;
  provider: AiProvider;
  api_key_masked: string;
  model_name: string;
  base_url: string | null;
  max_tokens: number;
  temperature: number;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateAiConfigRequest {
  provider: AiProvider;
  api_key: string;
  model_name: string;
  base_url?: string;
  max_tokens?: number;
  temperature?: number;
}

export interface UpdateAiConfigRequest {
  api_key?: string;
  model_name?: string;
  base_url?: string;
  max_tokens?: number;
  temperature?: number;
  active?: boolean;
}

export interface TestResult {
  success: boolean;
  message: string;
  response?: string;
}

export interface SummarizeResult {
  summary: string;
  provider: AiProvider;
  model: string;
}

// PURPOSE: List all AI configs for the current user (keys masked)
export async function listAiConfigs(): Promise<AiConfigurationResponse[]> {
  return apiClient.get<AiConfigurationResponse[]>('/ai/config');
}

// PURPOSE: Create a new AI provider configuration
export async function createAiConfig(data: CreateAiConfigRequest): Promise<AiConfigurationResponse> {
  return apiClient.post<AiConfigurationResponse>('/ai/config', data);
}

// PURPOSE: Update an existing AI configuration
export async function updateAiConfig(id: string, data: UpdateAiConfigRequest): Promise<AiConfigurationResponse> {
  return apiClient.put<AiConfigurationResponse>(`/ai/config/${id}`, data);
}

// PURPOSE: Delete an AI configuration
export async function deleteAiConfig(id: string): Promise<void> {
  await apiClient.delete(`/ai/config/${id}`);
}

// PURPOSE: Test an AI configuration by sending a simple completion request
export async function testAiConfig(id: string): Promise<TestResult> {
  return apiClient.post<TestResult>(`/ai/config/${id}/test`, {});
}

// PURPOSE: Summarize a single email using the user's active AI configuration
export async function summarizeEmail(_folder: string, _uid: number, emailText: string): Promise<SummarizeResult> {
  return apiClient.post<SummarizeResult>('/ai/summarize', { email_text: emailText });
}

// Added: Thread/conversation summary response for TMAIL-103
export interface ThreadSummaryResult {
  summary: string;
  message_count: number;
  provider: AiProvider;
  model: string;
}

// Added: Summarize an email thread/conversation using multiple message UIDs (TMAIL-103)
// PURPOSE: Fetches multiple emails via IMAP on the backend and produces a combined thread summary
export async function summarizeThread(folder: string, uids: number[]): Promise<ThreadSummaryResult> {
  return apiClient.post<ThreadSummaryResult>('/ai/thread-summary', { folder, uids });
}

// Added: Smart reply tone type for TMAIL-104
export type SmartReplyTone = 'brief' | 'detailed' | 'decline';

// Added: Smart reply response matching backend SmartReplyResponse struct
export interface SmartReplyResult {
  reply: string;
  tone: SmartReplyTone;
  provider: AiProvider;
  model: string;
}

// Added: Generate an AI-powered reply suggestion for an email (TMAIL-104)
// PURPOSE: Fetches the email via IMAP on the backend and generates a reply based on the selected tone
export async function getSmartReply(folder: string, uid: number, tone: SmartReplyTone): Promise<SmartReplyResult> {
  return apiClient.post<SmartReplyResult>('/ai/smart-reply', { folder, uid, tone });
}
