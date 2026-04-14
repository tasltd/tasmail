// Added: Semantic search API client for TMAIL-106
// PURPOSE: Provides functions for vector similarity search, email indexing, and index statistics

import { apiClient } from './client';

/// PURPOSE: A single semantic search result with similarity score
export interface SemanticSearchResult {
  folder: string;
  uid: number;
  subject: string | null;
  similarity_score: number;
}

/// PURPOSE: Index statistics response with total and per-folder breakdown
export interface IndexStatsResponse {
  total_indexed: number;
  per_folder: FolderIndexCount[];
}

export interface FolderIndexCount {
  folder: string;
  count: number;
}

/// PURPOSE: Response from indexing an email
export interface IndexEmailResponse {
  id: string;
  folder: string;
  uid: number;
  model_used: string;
  indexed: boolean;
}

// PURPOSE: Search emails by meaning using vector similarity
// CONSTRAINTS: Requires at least one active AI config on the backend
export async function semanticSearch(
  query: string,
  limit?: number,
): Promise<SemanticSearchResult[]> {
  return apiClient.post<SemanticSearchResult[]>('/search/semantic', {
    query,
    ...(limit !== undefined && { limit }),
  });
}

// PURPOSE: Index a specific email by generating and storing its embedding
export async function indexEmail(
  folder: string,
  uid: number,
  text: string,
  subject?: string,
): Promise<IndexEmailResponse> {
  return apiClient.post<IndexEmailResponse>('/search/index', {
    folder,
    uid,
    text,
    ...(subject !== undefined && { subject }),
  });
}

// PURPOSE: Get indexing statistics — total indexed count and per-folder breakdown
export async function getIndexStats(): Promise<IndexStatsResponse> {
  return apiClient.get<IndexStatsResponse>('/search/index/stats');
}
