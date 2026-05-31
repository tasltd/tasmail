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
  // Added (TMAIL-350): RFC 5322 §3.6.4 threading headers passed through from
  // MessageEnvelope so EmailList can route into the threaded view via
  // features/email/threadGrouping.ts. All optional — a message without
  // message_id (legacy sender) or without in_reply_to/references (thread
  // root) becomes its own thread-of-one bucket.
  messageId?: string | null;
  inReplyTo?: string | null;
  references?: string[];
}

export interface Folder {
  id: string;
  name: string;
  icon: string;
  count: number;
  isCustom?: boolean;
}
