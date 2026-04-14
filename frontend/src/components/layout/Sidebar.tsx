// Added: Mailbox icon for shared mailboxes sidebar entry (TMAIL-96)
// Added: ListTodo icon for email queue sidebar entry (TMAIL-58)
// Added: CheckSquare icon for tasks sidebar entry (TMAIL-126)
// Added: Webhook icon for webhooks sidebar entry (TMAIL-131)
// Added: Palette icon for branding sidebar entry (TMAIL-111)
// Added: Archive icon for retention sidebar entry (TMAIL-109)
// Added: Globe icon for custom hostnames sidebar entry (TMAIL-112)
// Added: FileUp icon for shared files sidebar entry (TMAIL-138)
// Added: UserPlus icon for bulk import sidebar entry (TMAIL-136)
// Added: MessageSquare icon for chat integrations sidebar entry (TMAIL-129)
// Added: Calendar icon for calendar/meeting scheduling sidebar entry (TMAIL-127)
// Added: Network icon for LDAP/AD directory sync sidebar entry (TMAIL-100)
// Added: Brain icon for AI configuration sidebar entry (TMAIL-105)
// Added: KeyRound icon for SAML SSO sidebar entry (TMAIL-101)
// Added: LogIn icon for OIDC providers sidebar entry (TMAIL-99)
// Added: Search icon for eDiscovery sidebar entry (TMAIL-137)
// Added: ShieldCheck icon for DLP sidebar entry (TMAIL-108)
// Added: ShieldAlert icon for DANE sidebar entry (TMAIL-125)
// Added: Send icon for BYO-SMTP configuration sidebar entry (TMAIL-48)
// Added: Puzzle icon for plugin management sidebar entry (TMAIL-132)
// Added: BookUser icon for contacts app sidebar entry (TMAIL-119)
// Added: Download icon for POP3 configuration sidebar entry (TMAIL-133)
// Added: HardDrive icon for email archive sidebar entry (TMAIL-107)
import { PenSquare, FileSignature, Users, Shield, Plane, UsersRound, Upload, Gauge, Filter, Mailbox, ListTodo, CheckSquare, Webhook, Palette, Archive, Globe, FileUp, UserPlus, MessageSquare, Calendar, Network, Brain, KeyRound, LogIn, Search, ShieldCheck, ShieldAlert, Send, Puzzle, BookUser, Download, HardDrive } from 'lucide-react';
import { FolderTree } from '../mail/FolderTree';
import { QuotaBar } from './QuotaBar';
import { useMailStore } from '../../stores/mailStore';

export function Sidebar() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const viewMode = useMailStore((s) => s.viewMode);

  return (
    <aside className="sidebar">
      <button className="btn btn--primary btn--compose" onClick={() => setViewMode('compose')}>
        <PenSquare size={18} />
        Compose
      </button>
      <FolderTree />
      <div style={{ borderTop: '1px solid var(--color-border)', marginTop: '12px', paddingTop: '8px' }}>
        <button
          className={`folder-item ${viewMode === 'signatures' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('signatures')}
        >
          <FileSignature size={18} />
          <span className="folder-item__name">Signatures</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'contacts' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('contacts')}
        >
          <Users size={18} />
          <span className="folder-item__name">Contacts</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'security' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('security')}
        >
          <Shield size={18} />
          <span className="folder-item__name">Security</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'vacation' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('vacation')}
        >
          <Plane size={18} />
          <span className="folder-item__name">Vacation</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'groups' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('groups')}
        >
          <UsersRound size={18} />
          <span className="folder-item__name">Groups</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'migration' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('migration')}
        >
          <Upload size={18} />
          <span className="folder-item__name">Migration</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'filters' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('filters')}
        >
          <Filter size={18} />
          <span className="folder-item__name">Filters</span>
        </button>
        {/* Added: Shared mailboxes navigation entry (TMAIL-96) */}
        <button
          className={`folder-item ${viewMode === 'shared' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('shared')}
        >
          <Mailbox size={18} />
          <span className="folder-item__name">Shared Mailboxes</span>
        </button>
        {/* Added: Tasks navigation entry (TMAIL-126) */}
        <button
          className={`folder-item ${viewMode === 'tasks' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('tasks')}
        >
          <CheckSquare size={18} />
          <span className="folder-item__name">Tasks</span>
        </button>
        {/* Added: Chat integrations navigation entry (TMAIL-129) */}
        <button
          className={`folder-item ${viewMode === 'chat' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('chat')}
        >
          <MessageSquare size={18} />
          <span className="folder-item__name">Chat</span>
        </button>
        {/* Added: Webhooks navigation entry (TMAIL-131) */}
        <button
          className={`folder-item ${viewMode === 'webhooks' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('webhooks')}
        >
          <Webhook size={18} />
          <span className="folder-item__name">Webhooks</span>
        </button>
        {/* Added: Email queue navigation entry (TMAIL-58) */}
        <button
          className={`folder-item ${viewMode === 'queue' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('queue')}
        >
          <ListTodo size={18} />
          <span className="folder-item__name">Queue</span>
        </button>
        {/* Added: Branding navigation entry (TMAIL-111) */}
        <button
          className={`folder-item ${viewMode === 'branding' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('branding')}
        >
          <Palette size={18} />
          <span className="folder-item__name">Branding</span>
        </button>
        {/* Added: Custom hostnames navigation entry (TMAIL-112) */}
        <button
          className={`folder-item ${viewMode === 'hostnames' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('hostnames')}
        >
          <Globe size={18} />
          <span className="folder-item__name">Hostnames</span>
        </button>
        {/* Added: Retention navigation entry (TMAIL-109) */}
        <button
          className={`folder-item ${viewMode === 'retention' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('retention')}
        >
          <Archive size={18} />
          <span className="folder-item__name">Retention</span>
        </button>
        {/* Added: Bulk import navigation entry (TMAIL-136) */}
        <button
          className={`folder-item ${viewMode === 'bulk-import' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('bulk-import')}
        >
          <UserPlus size={18} />
          <span className="folder-item__name">Bulk Import</span>
        </button>
        {/* Added: Calendar navigation entry (TMAIL-127) */}
        <button
          className={`folder-item ${viewMode === 'calendar' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('calendar')}
        >
          <Calendar size={18} />
          <span className="folder-item__name">Calendar</span>
        </button>
        {/* Added: Shared files navigation entry (TMAIL-138) */}
        <button
          className={`folder-item ${viewMode === 'shared-files' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('shared-files')}
        >
          <FileUp size={18} />
          <span className="folder-item__name">Shared Files</span>
        </button>
        {/* Added: LDAP/AD directory sync navigation entry (TMAIL-100) */}
        <button
          className={`folder-item ${viewMode === 'ldap' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('ldap')}
        >
          <Network size={18} />
          <span className="folder-item__name">LDAP / AD</span>
        </button>
        {/* Added: AI configuration navigation entry (TMAIL-105) */}
        <button
          className={`folder-item ${viewMode === 'ai-config' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('ai-config')}
        >
          <Brain size={18} />
          <span className="folder-item__name">AI Config</span>
        </button>
        {/* Added: SAML SSO navigation entry (TMAIL-101) */}
        <button
          className={`folder-item ${viewMode === 'saml' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('saml')}
        >
          <KeyRound size={18} />
          <span className="folder-item__name">SAML SSO</span>
        </button>
        {/* Added: OIDC providers navigation entry (TMAIL-99) */}
        <button
          className={`folder-item ${viewMode === 'oidc' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('oidc')}
        >
          <LogIn size={18} />
          <span className="folder-item__name">OIDC</span>
        </button>
        {/* Added: DLP navigation entry (TMAIL-108) */}
        <button
          className={`folder-item ${viewMode === 'dlp' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('dlp')}
        >
          <ShieldCheck size={18} />
          <span className="folder-item__name">DLP</span>
        </button>
        {/* Added: eDiscovery navigation entry (TMAIL-137) */}
        <button
          className={`folder-item ${viewMode === 'ediscovery' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('ediscovery')}
        >
          <Search size={18} />
          <span className="folder-item__name">eDiscovery</span>
        </button>
        {/* Added: DANE navigation entry (TMAIL-125) */}
        <button
          className={`folder-item ${viewMode === 'dane' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('dane')}
        >
          <ShieldAlert size={18} />
          <span className="folder-item__name">DANE</span>
        </button>
        {/* Added: BYO-SMTP configuration navigation entry (TMAIL-48) */}
        <button
          className={`folder-item ${viewMode === 'smtp-config' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('smtp-config')}
        >
          <Send size={18} />
          <span className="folder-item__name">SMTP</span>
        </button>
        {/* Added: Plugins navigation entry (TMAIL-132) */}
        <button
          className={`folder-item ${viewMode === 'plugins' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('plugins')}
        >
          <Puzzle size={18} />
          <span className="folder-item__name">Plugins</span>
        </button>
        {/* Added: Contacts App navigation entry (TMAIL-119) */}
        <button
          className={`folder-item ${viewMode === 'contacts-app' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('contacts-app')}
        >
          <BookUser size={18} />
          <span className="folder-item__name">Contacts App</span>
        </button>
        {/* Added: POP3 configuration navigation entry (TMAIL-133) */}
        <button
          className={`folder-item ${viewMode === 'pop3' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('pop3')}
        >
          <Download size={18} />
          <span className="folder-item__name">POP3</span>
        </button>
        {/* Added: Email archive navigation entry (TMAIL-107) */}
        <button
          className={`folder-item ${viewMode === 'archive' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('archive')}
        >
          <HardDrive size={18} />
          <span className="folder-item__name">Archive</span>
        </button>
        <button
          className={`folder-item ${viewMode === 'bandwidth' ? 'folder-item--active' : ''}`}
          onClick={() => setViewMode('bandwidth')}
        >
          <Gauge size={18} />
          <span className="folder-item__name">Bandwidth</span>
        </button>
      </div>
      <QuotaBar />
    </aside>
  );
}
