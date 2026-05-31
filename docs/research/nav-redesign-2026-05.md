# TASMail Sidebar Navigation Redesign Brief
**Date:** 2026-05-31  
**Issue:** New users see 41 flat settings buttons below inbox; can't find email.  
**Goal:** Reduce cognitive load by grouping settings, hiding admin, making inbox unmistakable.

---

## Best-Practice Patterns Across Mature Webmail Clients

### Gmail (Google)
- **Sidebar items:** ~6 top-level (Inbox, Starred, Snoozed, Sent, Drafts, More; More expands to show Labels, Trash, Spam)
- **Settings location:** Gear icon (top-right) → "See all settings" opens dedicated Settings Hub with left-rail tabs (General, Labels, Inbox, Accounts, etc.)
- **Admin:** N/A for personal; Google Workspace admins access separate Admin Console (different product)
- **Visual anchor:** Large "Compose" button below search. Inbox is 1st in folder list, bold weight on selected folder.
- **Empty state:** "No messages" or "You're all caught up"
- **First login:** Optional tour (tooltips on Compose, Search, Inbox); can be dismissed
- **Sources:** https://support.google.com/mail/answer/4520, https://www.google.com/intl/en/gmail/about/

### Outlook Web (Microsoft 365)
- **Sidebar items:** ~5 top-level (Inbox, Focused/Other, Flagged, Drafts, Sent; expandable for Folders, More Actions)
- **Settings location:** Gear icon (top-right) → "View all Outlook settings" opens Settings page with categories (Mail, Calendar, People, etc.)
- **Admin:** Exchange admin center (separate portal for org admins)
- **Visual anchor:** Large "New message" button. Inbox count badge at top. Focused inbox is default view (auto-categorized).
- **Empty state:** "You're all caught up!" message per folder
- **First login:** "What's New" slide-over can be dismissed; Ribbon menu shows help
- **Sources:** https://support.microsoft.com/en-us/office/organize-your-inbox-in-outlook-on-the-web-1a41c1a4-3fa2-48f6-9f62-4ea3b2b0edff

### Zoho Mail
- **Sidebar items:** ~8 top-level (Inbox, Drafts, Sent, Trash, Spam, Custom Folders, plus icons for Calendar, Contacts, Tasks, Files)
- **Settings location:** Gear icon (top-right) → Settings page with left-rail tabs (General, Mail, Signatures, Filters, etc.)
- **Admin:** Zoho Organizations (separate admin panel for domain admins)
- **Visual anchor:** Large "Compose" CTA. Inbox with unread count. Custom folders collapsible.
- **Empty state:** "No emails to display" per folder
- **First login:** No mandatory tour; Onboarding wizard for initial account setup
- **Sources:** https://www.zoho.com/mail/, https://help.zoho.com/portal/en/kb/mail

### FastMail
- **Sidebar items:** ~6 top-level (Inbox, All Mail, Drafts, Sent, Archive, Trash; expandable for Custom Folders and Tags)
- **Settings location:** Avatar icon (top-right) → "Settings" opens multi-tab Settings page (Account, Mail, Appearance, etc.)
- **Admin:** FastMail Teams (multi-user org accounts have separate admin settings)
- **Visual anchor:** Large "Compose" button. Inbox is primary. Folder counts show unread.
- **Empty state:** "No messages in this folder"
- **First login:** Quick setup guide (optional popover)
- **Sources:** https://www.fastmail.com/, https://www.fastmail.help/hc/en-us/

### ProtonMail
- **Sidebar items:** ~6 top-level (Inbox, Drafts, Sent, Trash, Archive, Spam; expandable for Custom Folders)
- **Settings location:** Avatar (top-right) → "Settings" → left-rail tabs (Account, Appearance, Privacy, Security, Keyboard Shortcuts, Labels, Folders, Filters)
- **Admin:** ProtonMail Business (separate admin panel for org managers)
- **Visual anchor:** Large "Compose" button. Inbox emphasized with count badge.
- **Empty state:** "Nothing to see here" message
- **First login:** Welcome tour (3–4 tooltips on key features; can skip)
- **Sources:** https://protonmail.com/, https://proton.me/support/

### Tutanota
- **Sidebar items:** ~5 top-level (Inbox, Drafts, Sent, Trash, Custom Folders/Tags; Contacts, Calendar separate apps)
- **Settings location:** Menu icon (top-right) → "Settings" → multi-tab (General, Mail, Appearance, etc.)
- **Admin:** Tutanota Teams (org admins access group management panel)
- **Visual anchor:** Large "New email" button. Inbox is topmost folder.
- **Empty state:** "No mails in this folder"
- **First login:** No mandatory onboarding; Security video available
- **Sources:** https://tutanota.com/, https://tutanota.com/blog/posts/welcome-to-tutanota/

### Roundcube (Open-source)
- **Sidebar items:** ~5 top-level (Inbox, Drafts, Sent, Trash, Spam; expandable for Custom Folders; Settings, Address Book, Calendar as separate top-level links)
- **Settings location:** "Settings" link in sidebar → dedicated page (Server settings, Preferences, Folders, Identities, Sieve)
- **Admin:** Host-provided (not in Roundcube; admin manages mail server directly)
- **Visual anchor:** Large "Compose" button. Inbox first in folder list.
- **Empty state:** "No messages" per folder
- **First login:** No tour (self-hosted, typically used by technically proficient users)
- **Sources:** https://roundcube.net/, https://github.com/roundcube/roundcubemail

---

## Pattern Synthesis: What They Agree On

1. **Inbox prominence:** Always folder #1 in sidebar, often bold or with unread count badge.
2. **Compose CTA:** Large primary button at top of sidebar (not buried in menu).
3. **Folder ecosystem:** 4–6 default folders (Inbox/Sent/Drafts/Trash/Spam/Archive) are visually dominant. Custom folders collapsible below.
4. **Settings as modal/page, not sidebar clutter:** ALL mature clients gate settings behind a gear icon + dedicated Settings Hub, NOT inline sidebar buttons.
5. **Settings categorization:** Settings pages use left-rail tabs (General, Mail, Security, Appearance, Filters, Signatures, etc.) so users can navigate by topic.
6. **Peer apps:** Calendar, Contacts, Tasks are **top-level nav entries or separate apps**, not settings. (ProtonMail, Zoho, Tutanota all treat Calendar as a peer app icon.)
7. **Admin is gated & separate:** Organization/domain admins access a **different product/portal** (Google Workspace Admin, Exchange Admin Center, Zoho Organizations, ProtonMail Business). NOT inline in the user's SPA sidebar.
8. **Empty-inbox UX:** A single-line message ("You're all caught up!") tells users what to expect when Inbox is empty.
9. **First-login onboarding:** Lightweight tooltips or optional tour on Compose, Search, Inbox — **NOT mandatory 10-step wizard**. Users who skip still see the product.

---

## TASMail-Specific Proposal: Six Buckets

### Bucket 1: Mail Folders (FolderTree) — Top Priority
```
┌─ [Compose] (large blue button)
├─ Inbox [3]                    ← Unread count badge
├─ Sent
├─ Drafts [1]
├─ Spam
├─ Trash
├─ Archive
└─ [+ Add Label]               ← Collapsible custom labels
```
**Rationale:** Identical to Gmail/Outlook/Zoho structure. Folders stay at top, unread counts are visible, users know where email lives.

### Bucket 2: Apps (Peer-to-Email) — High Visual Weight
Four top-level entries with distinct icons, same weight as folders:
```
├─ 📅 Calendar
├─ 👥 Contacts
├─ ✓ Tasks
└─ 🎨 Templates
```
**Rationale:** These are feature-rich enough for their own UIs (not form fields). Gmail/ProtonMail/Zoho all expose Calendar & Contacts as top-level. Users expect them here, not hidden.

### Bucket 3: User Settings (Consolidated) — Single Gear Entry
```
└─ ⚙ Settings
```
Opens a new **Settings Hub page** (NOT a modal) with left-rail tabs:
- **Account** — IMAP/SMTP config, Encryption, Change Password
- **Mail** — Signatures, Filters, Templates, Spam Rules, Auto-reply/Vacation
- **Notifications** — Push devices, Email frequency, Do-not-disturb
- **Security** — 2FA (TOTP, SMS, WebAuthn), Login activity, Sessions
- **Storage** — Quota, Archive policy, Retention rules
- **Appearance** — Theme, Font size, Sidebar collapse/expand
- **Integration** — CalDAV/CardDAV, POP3, Migration, Shared Files
- **Billing** — Subscription, Invoices, Usage

**Rationale:** Mirrors Gmail/Outlook/ProtonMail exactly. One entry in sidebar; categorized content inside. Users navigate by need, not overwhelmed by 25+ buttons.

### Bucket 4: Admin (Org-Only, Gated) — Collapsible or Separate Tab
Visible ONLY when `user.isAdmin === true`:
```
├─ [Divider: "Administration"]
├─ 🛡 Domain & Users
├─ 🔐 Security (LDAP/AD, SAML, OIDC, DANE)
├─ 📊 Compliance (DLP, eDiscovery, Retention, Webhooks)
├─ 🎯 Branding & Hostnames
├─ 🔧 Advanced (Plugins, Ollama, Rspamd, ActiveSync, CalDAV/CardDAV)
├─ 💳 Billing
└─ 📈 Deliverability & Monitoring
```
**Rationale:** Admins are small subset of users; hiding these by default unclutters the SPA for the majority. `RequireAdmin` component (already exists) gates access.

### Bucket 5: Empty-Inbox State & First-Login Copy
When Inbox is empty:
```
You're all caught up!
Emails from your IMAP server will appear here.
```

**First-login tour (optional, dismissible):**
1. **Compose button:** "Click here to write a new email"
2. **FolderTree:** "Your emails are organized in folders"
3. **Settings gear:** "Customize signatures, filters, and more"

**Rationale:** Users immediately know what to expect. Tour can be re-triggered via Help menu if dismissed.

### Bucket 6: Keyboard Shortcuts (Preserve Existing)
Keep `useKeyboardShortcuts` hook working:
- `g i` → Inbox
- `g s` → Sent
- `c` → Compose
- `?` → Help (show shortcuts modal)

Adds one more shortcut:
- `g e` → Settings (easier discoverability)

---

## Concrete Mockup

See `nav-redesign-mockup.html` (linked below) for an interactive HTML mockup of the proposed sidebar structure.

---

## Implementation Hints (For Workers)

### 1. Replace Flat Sidebar with Registry-Driven Config
**File:** `frontend/src/config/navConfig.ts` (new)
```typescript
export const NAV_CONFIG = [
  // Apps (always visible)
  { key: 'calendar', icon: Calendar, label: 'Calendar', group: 'apps', requiresAuth: true },
  { key: 'contacts', icon: Users, label: 'Contacts', group: 'apps', requiresAuth: true },
  { key: 'tasks', icon: CheckSquare, label: 'Tasks', group: 'apps', requiresAuth: true },
  { key: 'templates', icon: FileText, label: 'Templates', group: 'apps', requiresAuth: true },

  // Settings (single entry, opens hub)
  { key: 'settings', icon: Settings, label: 'Settings', group: 'user', requiresAuth: true },

  // Admin (gated by isAdmin flag)
  { key: 'admin-users', icon: Users, label: 'Users & Domains', group: 'admin', adminOnly: true },
  { key: 'admin-security', icon: Shield, label: 'Security', group: 'admin', adminOnly: true },
  // ... etc
];
```

**Update Sidebar.tsx:**
```tsx
export function Sidebar() {
  const { isAdmin } = useAuth();
  const visibleItems = NAV_CONFIG.filter(item => 
    !item.adminOnly || (item.adminOnly && isAdmin)
  );
  return (
    <aside className="sidebar">
      <button className="btn btn--primary btn--compose">Compose</button>
      <FolderTree />
      {visibleItems.map(item => (
        <SidebarItem key={item.key} {...item} />
      ))}
      <QuotaBar />
    </aside>
  );
}
```

### 2. Build SettingsHub Component
**File:** `frontend/src/components/settings/SettingsHub.tsx` (replaces scattered managers)
- Left-rail tabs: Account, Mail, Notifications, Security, Storage, Appearance, Integration, Billing
- Each tab lazy-loads its manager component (`<AccountSettings />`, `<MailSettings />`, etc.)
- Router: `/settings?tab=account` (or `/settings/account`)
- View mode: when `viewMode === 'settings'`, render `<SettingsHub />` in main pane

**Manager consolidation:**
- `SignaturesManager` → SettingsHub > Mail tab
- `FiltersManager` → SettingsHub > Mail tab
- `SecurityManager` + `TotpManager` + `WebAuthnManager` → SettingsHub > Security tab
- `NotificationsManager` → SettingsHub > Notifications tab
- etc.

### 3. Gate Admin Items
Reuse existing `RequireAdmin` pattern (`frontend/src/components/admin/RequireAdmin.tsx`):
```tsx
<RequireAdmin>
  <button className="folder-item" onClick={() => handleNavClick('admin-users')}>
    <Users size={18} />
    <span>Users & Domains</span>
  </button>
</RequireAdmin>
```

### 4. Preserve Keyboard Shortcuts
`frontend/src/hooks/useKeyboardShortcuts.ts` already handles `g i`, `g s`, `c`. Add:
```typescript
if (e.key === 'e' && e.ctrlKey && e.altKey) {
  // Ctrl+Alt+E → Settings (or g e for consistency with Gmail shortcuts)
  setViewMode('settings');
}
```

### 5. Empty-Inbox State & Tour
**Empty state (existing `MessageList` component):**
```tsx
{messages.length === 0 ? (
  <div className="empty-state">
    <p>You're all caught up!</p>
    <p>Emails from your IMAP server will appear here.</p>
  </div>
) : ...}
```

**First-login tour (new `FirstLoginTour` component):**
Use existing `react-joyride` or lightweight `react-tooltip` to highlight Compose, FolderTree, Settings on first login (tracked via localStorage `hasSeenTour`).

### 6. Update Traceability Baseline (If Applicable)
If `scripts/trace-check.py` flags new orphaned routes, update `docs/traceability/orphans-baseline.json` to include any admin-only routes that intentionally have no SPA consumer (they're only used via admin API or scripting).

---

## Migration Path

1. **Phase 1 (Week 1):** Build `navConfig.ts` + `SettingsHub` + admin gating. Route `/settings` → hub. Run `npm run trace-check` to validate no orphans.
2. **Phase 2 (Week 2):** Consolidate the 8 user-facing settings managers into SettingsHub tabs. Test each tab works.
3. **Phase 3 (Week 3):** Fold admin routes into sidebar (visible only to admins). Keyboard shortcut tests.
4. **Phase 4 (Week 4):** Empty-inbox copy + optional first-login tour. QA on mobile (sidebar collapse/expand behavior).

---

## Sources

- Gmail: https://support.google.com/mail/answer/4520
- Outlook Web: https://support.microsoft.com/en-us/office/organize-your-inbox-in-outlook-on-the-web-1a41c1a4-3fa2-48f6-9f62-4ea3b2b0edff
- Zoho Mail: https://help.zoho.com/portal/en/kb/mail
- FastMail: https://www.fastmail.help/hc/en-us/
- ProtonMail: https://proton.me/support/
- Tutanota: https://tutanota.com/
- Roundcube: https://roundcube.net/
