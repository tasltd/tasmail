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

// PURPOSE: Summarize an email using the user's active AI configuration
export async function summarizeEmail(emailText: string): Promise<SummarizeResult> {
  return apiClient.post<SummarizeResult>('/ai/summarize', { email_text: emailText });
}
