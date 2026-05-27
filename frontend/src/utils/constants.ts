export const API_BASE_URL = import.meta.env.VITE_API_URL || '/api';
export const WS_URL = import.meta.env.VITE_WS_URL || `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}/ws`;

export const DEFAULT_PAGE_SIZE = 50;

// Standard IMAP folder names
export const FOLDER_INBOX = 'INBOX';
export const FOLDER_SENT = 'Sent';
export const FOLDER_DRAFTS = 'Drafts';
export const FOLDER_TRASH = 'Trash';
export const FOLDER_SPAM = 'Junk';

// Added: Large file sharing thresholds (TMAIL-138). When a file picked in the
// Composer exceeds LARGE_FILE_THRESHOLD_BYTES, it is auto-uploaded via the
// shared-files API and replaced with a download link in the message body.
export const LARGE_FILE_THRESHOLD_BYTES = 25 * 1024 * 1024; // 25 MB
export const MAX_SHARED_FILE_BYTES = 500 * 1024 * 1024;     // 500 MB hard cap

// Added: Expiry presets for shared download links. `null` value = no expiry.
export interface SharedLinkExpiryOption {
  label: string;
  days: number | null;
}
export const SHARED_LINK_EXPIRY_OPTIONS: SharedLinkExpiryOption[] = [
  { label: '7 days', days: 7 },
  { label: '30 days', days: 30 },
  { label: 'Never', days: null },
];
export const DEFAULT_SHARED_LINK_EXPIRY_DAYS = 30;
