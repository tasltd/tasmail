import { create } from 'zustand';
import type { AdvancedSearchParams } from '../api/messages';

// Added: 'shared' view mode for shared mailbox management (TMAIL-96)
// Added: 'queue' view mode for email queue management (TMAIL-58)
// Added: 'tasks' view mode for task/to-do management (TMAIL-126)
// Added: 'webhooks' view mode for outbound webhook management (TMAIL-131)
// Added: 'branding' view mode for white-label customization (TMAIL-111)
// Added: 'retention' view mode for retention policies and legal holds (TMAIL-109)
// Added: 'hostnames' view mode for custom hostname management (TMAIL-112)
// Added: 'shared-files' view mode for large file sharing management (TMAIL-138)
// Added: 'bulk-import' view mode for bulk user provisioning (TMAIL-136)
// Added: 'chat' view mode for team chat integration management (TMAIL-129)
// Added: 'calendar' view mode for meeting scheduling (TMAIL-127)
// Added: 'ldap' view mode for LDAP/AD directory sync management (TMAIL-100)
// Added: 'ai-config' view mode for BYOK AI provider management (TMAIL-105)
// Added: 'saml' view mode for SAML 2.0 SSO configuration management (TMAIL-101)
// Added: 'oidc' view mode for OIDC identity provider management (TMAIL-99)
// Added: 'ediscovery' view mode for eDiscovery compliance search (TMAIL-137)
// Added: 'dlp' view mode for Data Loss Prevention rule management (TMAIL-108)
// Added: 'dane' view mode for DANE/TLSA policy and verification management (TMAIL-125)
// Added: 'smtp-config' view mode for BYO-SMTP configuration management (TMAIL-48)
// Added: 'plugins' view mode for plugin/extension management (TMAIL-132)
// Added: 'contacts-app' view mode for full contacts management app (TMAIL-119)
// Added: 'pop3' view mode for POP3 configuration management (TMAIL-133)
// Added: 'archive' view mode for email archive management with Piler (TMAIL-107)
// Added: 'activesync' view mode for ActiveSync device management (TMAIL-130)
// Added: 'ollama' view mode for Ollama local LLM management (TMAIL-102)
// Added: 'dav-config' view mode for CalDAV/CardDAV configuration management (TMAIL-117)
// Added: 'spam' view mode for Rspamd spam filter management (TMAIL-15)
// Added: 'billing' view mode for Paystack/MoMo billing management (TMAIL-46)
// Added: 'deliverability' view mode for email deliverability testing (TMAIL-39)
type ViewMode = 'list' | 'reader' | 'compose' | 'search' | 'signatures' | 'contacts' | 'security' | 'vacation' | 'groups' | 'migration' | 'bandwidth' | 'filters' | 'shared' | 'queue' | 'tasks' | 'webhooks' | 'branding' | 'retention' | 'hostnames' | 'shared-files' | 'bulk-import' | 'chat' | 'calendar' | 'ldap' | 'ai-config' | 'saml' | 'oidc' | 'ediscovery' | 'dlp' | 'dane' | 'smtp-config' | 'plugins' | 'contacts-app' | 'pop3' | 'archive' | 'activesync' | 'ollama' | 'dav-config' | 'spam' | 'billing' | 'deliverability';

interface MailState {
  selectedFolder: string;
  selectedUid: number | null;
  viewMode: ViewMode;
  searchQuery: string;
  // Added: Advanced search filter state for TMAIL-32
  advancedSearch: AdvancedSearchParams | null;
  setSelectedFolder: (folder: string) => void;
  setSelectedUid: (uid: number | null) => void;
  setViewMode: (mode: ViewMode) => void;
  setSearchQuery: (query: string) => void;
  setAdvancedSearch: (params: AdvancedSearchParams | null) => void;
}

export const useMailStore = create<MailState>((set) => ({
  selectedFolder: 'INBOX',
  selectedUid: null,
  viewMode: 'list',
  searchQuery: '',
  advancedSearch: null,
  setSelectedFolder: (folder) => set({ selectedFolder: folder, selectedUid: null, viewMode: 'list' }),
  setSelectedUid: (uid) => set({ selectedUid: uid, viewMode: uid ? 'reader' : 'list' }),
  setViewMode: (mode) => set({ viewMode: mode }),
  setSearchQuery: (query) => set({ searchQuery: query, viewMode: query ? 'search' : 'list' }),
  // Added: Set advanced search params and switch to search view
  setAdvancedSearch: (params) => set({
    advancedSearch: params,
    viewMode: params ? 'search' : 'list',
  }),
}));
