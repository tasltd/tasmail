// Added: NLP search API client for TMAIL-135
// PURPOSE: Provides functions for AI-powered natural language email search and history management

import { apiClient } from './client';

/// PURPOSE: Structured search parameters parsed from a natural language query by the AI
export interface ParsedSearchParams {
  from?: string;
  to?: string;
  subject?: string;
  keywords: string[];
  date_from?: string;
  date_to?: string;
  folder?: string;
  has_attachment?: boolean;
}

/// PURPOSE: A single message result from NLP search
export interface NlpSearchResultItem {
  folder: string;
  uid: number;
  subject: string | null;
  from: string | null;
  date: string | null;
}

/// PURPOSE: Full NLP search response including parsed params and results
export interface NlpSearchResult {
  query: string;
  parsed_params: ParsedSearchParams;
  result_count: number;
  results: NlpSearchResultItem[];
}

/// PURPOSE: A single entry from the NLP search history
export interface NlpSearchHistoryEntry {
  id: string;
  user_id: string;
  query_text: string;
  parsed_params: ParsedSearchParams;
  result_count: number;
  created_at: string;
}

// PURPOSE: Execute a natural language search query via AI parsing
// CONSTRAINTS: Requires an active AI config on the backend
export async function nlpSearch(query: string): Promise<NlpSearchResult> {
  return apiClient.post<NlpSearchResult>('/search/nlp', { query });
}

// PURPOSE: List the user's NLP search history (most recent first, max 50)
export async function listNlpHistory(): Promise<NlpSearchHistoryEntry[]> {
  return apiClient.get<NlpSearchHistoryEntry[]>('/search/nlp/history');
}

// PURPOSE: Clear all NLP search history for the current user
export async function clearNlpHistory(): Promise<{ deleted: number; message: string }> {
  return apiClient.delete<{ deleted: number; message: string }>('/search/nlp/history');
}
