// Added: Ollama local LLM management API client for TMAIL-102
// PURPOSE: Admin endpoints for Ollama config, status, model management

import { apiClient } from './client';

// Added: Ollama configuration matching backend OllamaConfig struct
export interface OllamaConfig {
  id: string;
  base_url: string;
  enabled: boolean;
  default_model: string | null;
  max_context_length: number | null;
  gpu_layers: number | null;
  updated_at: string | null;
}

// Added: Update request matching backend UpdateOllamaConfigRequest
export interface UpdateOllamaConfigRequest {
  base_url?: string;
  enabled?: boolean;
  default_model?: string;
  max_context_length?: number;
  gpu_layers?: number;
}

// Added: Model info from Ollama /api/tags
export interface OllamaModelInfo {
  name: string;
  size: number | null;
  parameter_size: string | null;
  quantization_level: string | null;
  modified_at: string | null;
}

// Added: Server status with health, version, and models
export interface OllamaStatus {
  running: boolean;
  version: string | null;
  models: OllamaModelInfo[];
}

// Added: Cached model from the database
export interface OllamaModelCache {
  id: string;
  model_name: string;
  size_bytes: number | null;
  parameter_count: string | null;
  quantization: string | null;
  last_pulled_at: string | null;
  created_at: string | null;
}

// Added: Pull result response
export interface PullResult {
  success: boolean;
  message: string;
}

// PURPOSE: Get the current Ollama configuration
export async function getOllamaConfig(): Promise<OllamaConfig> {
  return apiClient.get<OllamaConfig>('/admin/ollama/config');
}

// PURPOSE: Update Ollama configuration
export async function updateOllamaConfig(data: UpdateOllamaConfigRequest): Promise<OllamaConfig> {
  return apiClient.put<OllamaConfig>('/admin/ollama/config', data);
}

// PURPOSE: Get Ollama server health status and available models
export async function getOllamaStatus(): Promise<OllamaStatus> {
  return apiClient.get<OllamaStatus>('/admin/ollama/status');
}

// PURPOSE: Pull (download) a model on the Ollama server
export async function pullOllamaModel(model: string): Promise<PullResult> {
  return apiClient.post<PullResult>('/admin/ollama/models/pull', { model });
}

// PURPOSE: Delete a model from the Ollama server
export async function deleteOllamaModel(name: string): Promise<void> {
  await apiClient.delete(`/admin/ollama/models/${encodeURIComponent(name)}`);
}

// PURPOSE: List cached models from the database
export async function listCachedModels(): Promise<OllamaModelCache[]> {
  return apiClient.get<OllamaModelCache[]>('/admin/ollama/models');
}

// PURPOSE: Format byte size into human-readable string
export function formatModelSize(bytes: number | null): string {
  if (bytes === null || bytes === 0) return '—';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let unitIndex = 0;
  let size = bytes;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }
  return `${size.toFixed(1)} ${units[unitIndex]}`;
}
