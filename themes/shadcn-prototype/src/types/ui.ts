// TMAIL-239: alt-UI shared view-model types. Moved out of data/mockData.ts so
// the typed surface is independent of any mock seed data. EmailClient,
// EmailList, EmailReader, and Sidebar all import from here.
export interface Email {
  id: string;
  from: string;
  fromEmail: string;
  to: string;
  subject: string;
  preview: string;
  body: string;
  timestamp: Date;
  read: boolean;
  starred: boolean;
  folder: string;
  attachments?: Array<{ name: string; size: string }>;
}

export interface Folder {
  id: string;
  name: string;
  icon: string;
  count: number;
  isCustom?: boolean;
}
