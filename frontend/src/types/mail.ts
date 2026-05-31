export interface Folder {
  name: string;
  delimiter: string;
  messages: number | null;
  unseen: number | null;
}

export interface MessageEnvelope {
  uid: number;
  subject: string | null;
  from: string | null;
  date: string | null;
  flags: string[];
  size: number | null;
  // Added (TMAIL-350): threading headers carried on every envelope so the
  // alt-UI EmailList can group rows into conversations without making a
  // per-row /messages/{uid} fetch. All three are optional in the wire shape
  // — `message_id` is null on legacy senders that omit Message-ID,
  // `in_reply_to` is null on thread roots, `references` is empty on first
  // messages of a conversation. Classic SPA does not use these yet (TMAIL-350
  // is alt-UI only) but exposing the fields on the shared type keeps both
  // surfaces decoding the same JSON shape.
  message_id?: string | null;
  in_reply_to?: string | null;
  references?: string[];
}

export interface FullMessage {
  uid: number;
  subject: string | null;
  from: string | null;
  to: string[];
  cc: string[];
  date: string | null;
  flags: string[];
  text_body: string | null;
  html_body: string | null;
  attachments: Attachment[];
  message_id: string | null;
  in_reply_to: string | null;
  references: string[];
}

export interface Attachment {
  filename: string;
  content_type: string;
  size: number;
  part_id: string;
}

export interface MessageListResponse {
  messages: MessageEnvelope[];
  total: number;
  page: number;
  page_size: number;
}

export interface SearchResponse {
  messages: MessageEnvelope[];
  total: number;
  query: string;
  folder: string;
}

export interface SendEmailRequest {
  to: string[];
  cc?: string[];
  bcc?: string[];
  subject: string;
  text_body?: string;
  html_body?: string;
}
