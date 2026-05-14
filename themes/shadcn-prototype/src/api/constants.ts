export const API_BASE_URL = import.meta.env.VITE_API_URL || '/api';
export const WS_URL = import.meta.env.VITE_WS_URL || `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}/ws`;

export const DEFAULT_PAGE_SIZE = 50;

// Standard IMAP folder names
export const FOLDER_INBOX = 'INBOX';
export const FOLDER_SENT = 'Sent';
export const FOLDER_DRAFTS = 'Drafts';
export const FOLDER_TRASH = 'Trash';
export const FOLDER_SPAM = 'Junk';
