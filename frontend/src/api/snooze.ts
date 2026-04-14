import { apiClient } from './client';

export interface SnoozedEmail {
  id: string;
  mailbox_id: string;
  folder: string;
  message_uid: number;
  snooze_until: string;
  original_folder: string;
  created_at: string;
}

export interface CreateSnoozeRequest {
  folder: string;
  message_uid: number;
  snooze_until: string;
}

export async function snoozeMessage(data: CreateSnoozeRequest): Promise<SnoozedEmail> {
  return apiClient.post('/api/messages/snooze', data);
}

export async function listSnoozed(): Promise<SnoozedEmail[]> {
  return apiClient.get('/api/messages/snoozed');
}

export async function cancelSnooze(id: string): Promise<void> {
  return apiClient.delete(`/api/messages/snooze/${id}`);
}

// Added: common snooze presets
export function getSnoozePresets(): { label: string; getTime: () => Date }[] {
  return [
    {
      label: 'Later today',
      getTime: () => {
        const d = new Date();
        d.setHours(d.getHours() + 3);
        return d;
      },
    },
    {
      label: 'Tomorrow morning',
      getTime: () => {
        const d = new Date();
        d.setDate(d.getDate() + 1);
        d.setHours(8, 0, 0, 0);
        return d;
      },
    },
    {
      label: 'Next week',
      getTime: () => {
        const d = new Date();
        d.setDate(d.getDate() + 7);
        d.setHours(8, 0, 0, 0);
        return d;
      },
    },
  ];
}
