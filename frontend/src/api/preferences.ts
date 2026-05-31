// Added (TMAIL-401): per-user preference flags. Currently only the
// first-login-tour-seen flag; further flags should land here so all
// preference reads/writes share one module.

import { apiClient } from './client';

export interface TourSeenResponse {
  seen: boolean;
}

const PATH = '/me/preferences/first-login-tour-seen';

export async function fetchFirstLoginTourSeen(): Promise<TourSeenResponse> {
  return apiClient.get<TourSeenResponse>(PATH);
}

export async function markFirstLoginTourSeen(): Promise<TourSeenResponse> {
  return apiClient.patch<TourSeenResponse>(PATH, {});
}
