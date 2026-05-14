// TMAIL-203: admin client for the IP warm-up endpoints.
import { apiClient } from './client';

export interface WarmupStatus {
  ip_address: string;
  current_day: number;
  current_week: number;
  daily_limit: number;
  emails_sent_today: number;
  total_emails_sent: number;
  remaining_today: number;
  started_at: string | null;
  paused: boolean;
  completed: boolean;
}

export interface WarmupWeek {
  week: number;
  daily_limit: number;
  description: string;
}

export interface WarmupSchedule {
  weeks: WarmupWeek[];
  total_days: number;
}

export interface WarmupScheduleResponse {
  schedule: WarmupSchedule;
  description: string;
}

export const adminWarmupApi = {
  status: () => apiClient.get<WarmupStatus[]>('/admin/warmup/status'),
  schedule: () => apiClient.get<WarmupScheduleResponse>('/admin/warmup/schedule'),
  start: (ip_address: string) =>
    apiClient.post<WarmupStatus>('/admin/warmup/start', { ip_address }),
};
