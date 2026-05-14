// TMAIL-199: admin client for cache status / stats / flush.
import { apiClient } from './client';

export interface CacheStatus {
  connected: boolean;
  redis_url: string;
  branding_ttl_secs: number;
  quota_ttl_secs: number;
  session_ttl_secs: number;
  rate_limit_window_secs: number;
  rate_limit_max_requests: number;
}

export interface CacheStatsResponse {
  connected: boolean;
  info: string | null;
}

export interface CacheFlushResponse {
  flushed: boolean;
  message: string;
}

export const cacheApi = {
  status: () => apiClient.get<CacheStatus>('/admin/cache/status'),
  stats: () => apiClient.get<CacheStatsResponse>('/admin/cache/stats'),
  flush: () => apiClient.post<CacheFlushResponse>('/admin/cache/flush', {}),
};
