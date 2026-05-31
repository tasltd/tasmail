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
  // Added (TMAIL-329): ~200 char plaintext snippet of the message body so
  // EmailList rows render a preview line under the subject. Backend emits
  // null when nothing could be extracted (truncated MIME, attachment-only
  // messages, unparseable bodies); callers must `?? ''` defensively.
  preview: string | null;
  // Added (TMAIL-350): RFC 5322 §3.6.4 threading headers extracted server-side
  // from the same 8 KiB partial body fetch the preview comes from. EmailList
  // groups rows into conversations using groupByThread() in
  // features/email/threadGrouping.ts. Fields are optional in the JSON shape
  // — message_id is null on legacy senders that omit Message-ID, in_reply_to
  // is null on thread roots, references is empty/absent on first messages.
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
