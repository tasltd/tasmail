// TMAIL-200: admin client for the domains table.
import { apiClient } from './client';

export interface Domain {
  id: string;
  name: string;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export const adminDomainsApi = {
  list: () => apiClient.get<Domain[]>('/admin/domains'),
  create: (name: string) => apiClient.post<Domain>('/admin/domains', { name }),
  delete: (id: string) => apiClient.delete<void>(`/admin/domains/${id}`),
};
