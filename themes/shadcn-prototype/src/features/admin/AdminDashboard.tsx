// TMAIL-232 + TMAIL-233: live admin dashboard wired to /api/admin/users,
// /api/admin/domains, and /api/quota. The previous mocks (mockMailboxes /
// mockStats / hardcoded mydomain.com block) are gone — every number on this
// page now comes from the live PostgreSQL database via the Axum backend.
//
// TMAIL-352: split into two tabs — Overview (mailboxes + domains + stats,
// the historical view) and Audit log (paginated, filterable viewer for
// /api/admin/audit-log). Tabs are URL-synced via the `?tab=` query param
// so deep links and reloads keep the active pane.
//
// TMAIL-353: added four more sub-tabs — Branding, SAML providers, OIDC
// providers, LDAP sources — each a separate component so AdminDashboard
// stays a thin shell that just routes the active tab. The tab IDs live
// in audit-log-helpers (now the AdminTab union) so the parser stays
// authoritative and adding a tab is a registry-style change.
//
// TMAIL-354: added four compliance sub-tabs — Retention policies, Legal
// holds, DLP rules + violations, eDiscovery cases. Same shell pattern —
// each sub-tab is its own component file, wired through the registry
// in audit-log-helpers.ts so adding the next compliance feature is a
// single registry-entry change.
import { useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  ArrowLeft, Plus, Trash2, Users, HardDrive, Mail, Globe,
  CheckCircle, XCircle, AlertCircle, ShieldCheck,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { adminUsersApi, type UserInfo, type CreateUserRequest } from '@/api/admin-users';
import { adminDomainsApi, type Domain } from '@/api/admin-domains';
import { quotaApi } from '@/api/quota';
import { isAdmin } from '@/lib/jwt';
import { AuditLogTab } from './AuditLogTab';
import { BrandingTab } from './BrandingTab';
import { SamlTab } from './SamlTab';
import { OidcTab } from './OidcTab';
import { LdapTab } from './LdapTab';
import { RetentionTab } from './RetentionTab';
import { LegalHoldsTab } from './LegalHoldsTab';
import { DlpTab } from './DlpTab';
import { EdiscoveryTab } from './EdiscoveryTab';
import {
  ADMIN_TAB_AUDIT,
  ADMIN_TAB_BRANDING,
  ADMIN_TAB_DLP,
  ADMIN_TAB_EDISCOVERY,
  ADMIN_TAB_LDAP,
  ADMIN_TAB_LEGAL_HOLDS,
  ADMIN_TAB_OIDC,
  ADMIN_TAB_OVERVIEW,
  ADMIN_TAB_RETENTION,
  ADMIN_TAB_SAML,
  parseAdminTab,
  type AdminTab,
} from './audit-log-helpers';

function bytesToGB(bytes: number): number {
  return Math.round((bytes / (1024 ** 3)) * 100) / 100;
}

export function AdminDashboard() {
  const qc = useQueryClient();
  const admin = isAdmin();
  // TMAIL-352: URL-synced active tab so reloading /modern/admin?tab=audit-log
  // keeps the audit pane open and deep links work.
  const [searchParams, setSearchParams] = useSearchParams();
  const activeTab: AdminTab = parseAdminTab(searchParams.get('tab'));
  const setActiveTab = (next: AdminTab) => {
    const params = new URLSearchParams(searchParams);
    if (next === ADMIN_TAB_OVERVIEW) {
      params.delete('tab');
    } else {
      params.set('tab', next);
    }
    setSearchParams(params, { replace: true });
  };

  const usersQ = useQuery<UserInfo[]>({
    queryKey: ['admin', 'users'],
    queryFn: () => adminUsersApi.list(),
    enabled: admin,
  });
  const domainsQ = useQuery<Domain[]>({
    queryKey: ['admin', 'domains'],
    queryFn: () => adminDomainsApi.list(),
    enabled: admin,
  });
  const myQuotaQ = useQuery({
    queryKey: ['quota', 'me'],
    queryFn: () => quotaApi.getQuota(),
    enabled: admin,
  });

  const [search, setSearch] = useState('');
  const [showAddUser, setShowAddUser] = useState(false);
  const [showAddDomain, setShowAddDomain] = useState(false);
  const [newUser, setNewUser] = useState<CreateUserRequest>({
    username: '', password: '', domain_id: '', display_name: '',
  });
  const [newDomainName, setNewDomainName] = useState('');
  const [actionError, setActionError] = useState<string | null>(null);

  const createUserMut = useMutation({
    mutationFn: (body: CreateUserRequest) => adminUsersApi.create(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'users'] });
      setShowAddUser(false);
      setNewUser({ username: '', password: '', domain_id: '', display_name: '' });
      setActionError(null);
    },
    onError: (err: Error) => setActionError(err.message),
  });
  const deleteUserMut = useMutation({
    mutationFn: (id: string) => adminUsersApi.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'users'] }),
    onError: (err: Error) => setActionError(err.message),
  });
  const createDomainMut = useMutation({
    mutationFn: (name: string) => adminDomainsApi.create(name),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'domains'] });
      setShowAddDomain(false);
      setNewDomainName('');
      setActionError(null);
    },
    onError: (err: Error) => setActionError(err.message),
  });
  const deleteDomainMut = useMutation({
    mutationFn: (id: string) => adminDomainsApi.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'domains'] }),
    onError: (err: Error) => setActionError(err.message),
  });

  // Per-domain mailbox count, derived from the users list.
  const usersByDomain = useMemo(() => {
    const map = new Map<string, number>();
    (usersQ.data ?? []).forEach((u) => {
      map.set(u.domain_id, (map.get(u.domain_id) ?? 0) + 1);
    });
    return map;
  }, [usersQ.data]);

  const visibleUsers = useMemo(() => {
    const list = usersQ.data ?? [];
    if (!search) return list;
    const q = search.toLowerCase();
    return list.filter((u) =>
      u.username.toLowerCase().includes(q) || (u.display_name ?? '').toLowerCase().includes(q),
    );
  }, [usersQ.data, search]);

  if (!admin) {
    return (
      <div className="h-full overflow-y-auto bg-zinc-50 dark:bg-zinc-950 p-12">
        <div className="max-w-xl mx-auto text-center space-y-4">
          <h1 className="text-3xl font-semibold">Admin only</h1>
          <p className="text-zinc-600 dark:text-zinc-400">
            This area is restricted to operators with the <code>is_admin</code> flag set on
            their mailbox. You're signed in but don't have the admin role.
          </p>
          <Link to="/">
            <Button>Back to mailbox</Button>
          </Link>
        </div>
      </div>
    );
  }

  const totalUsers = usersQ.data?.length ?? 0;
  const activeDomains = (domainsQ.data ?? []).filter((d) => d.active).length;
  const totalQuotaBytes = (usersQ.data ?? []).reduce((sum, u) => sum + (u.quota_bytes ?? 0), 0);
  const meUsedBytes = myQuotaQ.data?.used_bytes ?? 0;

  return (
    <div className="h-full overflow-y-auto bg-zinc-50 dark:bg-zinc-950">
      <div className="max-w-7xl mx-auto p-6 space-y-6">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-3 mb-2">
              <Link to="/">
                <Button variant="outline" size="icon">
                  <ArrowLeft className="size-4" />
                </Button>
              </Link>
              <h1 className="text-3xl font-semibold">Admin Dashboard</h1>
            </div>
            <p className="text-zinc-600 dark:text-zinc-400">
              Manage mailboxes, domains, and system settings
            </p>
          </div>
        </div>

        {actionError && (
          <Card className="p-3 border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950">
            <div className="text-sm text-red-700 dark:text-red-300">{actionError}</div>
          </Card>
        )}

        {/* TMAIL-352: Tabs shell. Overview keeps the historical Mailboxes
            + Domains + stat-cards layout; Audit log is the new pane. */}
        <Tabs
          value={activeTab}
          onValueChange={(v) => setActiveTab(parseAdminTab(v))}
          className="space-y-4"
        >
          <TabsList className="flex-wrap h-auto">
            <TabsTrigger value={ADMIN_TAB_OVERVIEW} data-testid="admin-tab-overview">Overview</TabsTrigger>
            <TabsTrigger value={ADMIN_TAB_AUDIT} data-testid="admin-tab-audit-log">Audit log</TabsTrigger>
            <TabsTrigger value={ADMIN_TAB_BRANDING} data-testid="admin-tab-branding">Branding</TabsTrigger>
            <TabsTrigger value={ADMIN_TAB_SAML} data-testid="admin-tab-saml">SAML</TabsTrigger>
            <TabsTrigger value={ADMIN_TAB_OIDC} data-testid="admin-tab-oidc">OIDC</TabsTrigger>
            <TabsTrigger value={ADMIN_TAB_LDAP} data-testid="admin-tab-ldap">LDAP</TabsTrigger>
            <TabsTrigger value={ADMIN_TAB_RETENTION} data-testid="admin-tab-retention">Retention</TabsTrigger>
            <TabsTrigger value={ADMIN_TAB_LEGAL_HOLDS} data-testid="admin-tab-legal-holds">Legal holds</TabsTrigger>
            <TabsTrigger value={ADMIN_TAB_DLP} data-testid="admin-tab-dlp">DLP</TabsTrigger>
            <TabsTrigger value={ADMIN_TAB_EDISCOVERY} data-testid="admin-tab-ediscovery">eDiscovery</TabsTrigger>
          </TabsList>

          <TabsContent value={ADMIN_TAB_OVERVIEW} className="space-y-6">

        {/* Stats Cards */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <Card className="p-6">
            <div className="flex items-center justify-between mb-2">
              <Users className="size-8 text-blue-600" />
              <span className="text-sm text-zinc-500">Total</span>
            </div>
            <div className="text-3xl font-semibold">{totalUsers}</div>
            <div className="text-sm text-zinc-600 dark:text-zinc-400">Active Mailboxes</div>
          </Card>

          <Card className="p-6">
            <div className="flex items-center justify-between mb-2">
              <HardDrive className="size-8 text-green-600" />
              <span className="text-sm text-zinc-500">
                {totalQuotaBytes > 0 ? `${Math.round((meUsedBytes / totalQuotaBytes) * 100)}%` : '—'}
              </span>
            </div>
            <div className="text-3xl font-semibold">
              {bytesToGB(meUsedBytes)} GB
            </div>
            <div className="text-sm text-zinc-600 dark:text-zinc-400">
              of {bytesToGB(totalQuotaBytes)} GB allocated
            </div>
          </Card>

          <Card className="p-6">
            <div className="flex items-center justify-between mb-2">
              <Mail className="size-8 text-purple-600" />
              <span className="text-sm text-zinc-500">Mine</span>
            </div>
            <div className="text-3xl font-semibold">{myQuotaQ.data?.message_count ?? 0}</div>
            <div className="text-sm text-zinc-600 dark:text-zinc-400">
              messages in my mailbox
            </div>
          </Card>

          <Card className="p-6">
            <div className="flex items-center justify-between mb-2">
              <Globe className="size-8 text-orange-600" />
              <span className="text-sm text-zinc-500">Active</span>
            </div>
            <div className="text-3xl font-semibold">{activeDomains}</div>
            <div className="text-sm text-zinc-600 dark:text-zinc-400">Domains Configured</div>
          </Card>
        </div>

        {/* Mailboxes Table */}
        <Card className="p-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-xl font-semibold">Mailboxes</h2>
            <div className="flex gap-2">
              <Input
                type="search"
                placeholder="Search mailboxes..."
                className="w-64"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
              <Button onClick={() => setShowAddUser((v) => !v)}>
                <Plus className="size-4 mr-2" />
                Add Mailbox
              </Button>
            </div>
          </div>

          {showAddUser && (
            <div className="mb-4 p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg space-y-2">
              <div className="grid grid-cols-1 md:grid-cols-4 gap-2">
                <Input
                  placeholder="username (no @)"
                  value={newUser.username}
                  onChange={(e) => setNewUser((p) => ({ ...p, username: e.target.value }))}
                />
                <Input
                  type="password"
                  placeholder="password"
                  value={newUser.password}
                  onChange={(e) => setNewUser((p) => ({ ...p, password: e.target.value }))}
                />
                <select
                  className="h-9 rounded-md border border-zinc-200 dark:border-zinc-800 bg-transparent px-3 text-sm"
                  value={newUser.domain_id}
                  onChange={(e) => setNewUser((p) => ({ ...p, domain_id: e.target.value }))}
                >
                  <option value="">— pick domain —</option>
                  {(domainsQ.data ?? []).map((d) => (
                    <option key={d.id} value={d.id}>{d.name}</option>
                  ))}
                </select>
                <Input
                  placeholder="Display name (optional)"
                  value={newUser.display_name ?? ''}
                  onChange={(e) => setNewUser((p) => ({ ...p, display_name: e.target.value }))}
                />
              </div>
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={() => setShowAddUser(false)}>Cancel</Button>
                <Button
                  disabled={!newUser.username || !newUser.password || !newUser.domain_id || createUserMut.isPending}
                  onClick={() => createUserMut.mutate(newUser)}
                >
                  {createUserMut.isPending ? 'Creating…' : 'Create mailbox'}
                </Button>
              </div>
            </div>
          )}

          <div className="overflow-x-auto">
            {usersQ.isLoading ? (
              <div className="p-6 text-zinc-500 text-sm">Loading mailboxes…</div>
            ) : usersQ.isError ? (
              <div className="p-6 text-red-600 text-sm">Couldn't load mailboxes. {String(usersQ.error)}</div>
            ) : visibleUsers.length === 0 ? (
              <div className="p-6 text-zinc-500 text-sm">No mailboxes yet — add one above.</div>
            ) : (
              <table className="w-full">
                <thead>
                  <tr className="border-b border-zinc-200 dark:border-zinc-800">
                    <th className="text-left py-3 px-4 font-medium">User</th>
                    <th className="text-left py-3 px-4 font-medium">Domain</th>
                    <th className="text-left py-3 px-4 font-medium">Quota</th>
                    <th className="text-left py-3 px-4 font-medium">Role</th>
                    <th className="text-left py-3 px-4 font-medium">Status</th>
                    <th className="text-right py-3 px-4 font-medium">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {visibleUsers.map((u) => {
                    const domain = (domainsQ.data ?? []).find((d) => d.id === u.domain_id);
                    return (
                      <tr
                        key={u.id}
                        className="border-b border-zinc-200 dark:border-zinc-800 hover:bg-zinc-50 dark:hover:bg-zinc-900"
                      >
                        <td className="py-3 px-4 font-medium">
                          {u.display_name || u.username}
                          <div className="text-xs text-zinc-500">{u.username}@{domain?.name ?? '?'}</div>
                        </td>
                        <td className="py-3 px-4 text-zinc-600 dark:text-zinc-400">
                          {domain?.name ?? <span className="text-zinc-400 italic">unknown</span>}
                        </td>
                        <td className="py-3 px-4 text-sm">{bytesToGB(u.quota_bytes)} GB</td>
                        <td className="py-3 px-4">
                          {u.is_admin ? (
                            <span className="inline-flex items-center gap-1 text-green-600 text-sm">
                              <ShieldCheck className="size-4" /> admin
                            </span>
                          ) : (
                            <span className="text-zinc-400 text-sm">user</span>
                          )}
                        </td>
                        <td className="py-3 px-4">
                          {u.active ? (
                            <span className="inline-flex items-center gap-2 text-green-600">
                              <CheckCircle className="size-4" /> Active
                            </span>
                          ) : (
                            <span className="inline-flex items-center gap-2 text-red-600">
                              <XCircle className="size-4" /> Disabled
                            </span>
                          )}
                        </td>
                        <td className="py-3 px-4 text-right">
                          <Button
                            variant="ghost"
                            size="sm"
                            className="text-red-600 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-950"
                            disabled={deleteUserMut.isPending}
                            onClick={() => {
                              if (window.confirm(`Delete mailbox ${u.username}@${domain?.name ?? ''}?`)) {
                                deleteUserMut.mutate(u.id);
                              }
                            }}
                          >
                            <Trash2 className="size-4" />
                          </Button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>
        </Card>

        {/* Domain Management */}
        <Card className="p-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-xl font-semibold">Domain Management</h2>
            <Button onClick={() => setShowAddDomain((v) => !v)}>
              <Plus className="size-4 mr-2" />
              Add Domain
            </Button>
          </div>

          {showAddDomain && (
            <div className="mb-4 p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg flex gap-2">
              <Input
                placeholder="example.com"
                value={newDomainName}
                onChange={(e) => setNewDomainName(e.target.value)}
                className="flex-1"
              />
              <Button variant="outline" onClick={() => setShowAddDomain(false)}>Cancel</Button>
              <Button
                disabled={!newDomainName || createDomainMut.isPending}
                onClick={() => createDomainMut.mutate(newDomainName)}
              >
                {createDomainMut.isPending ? 'Creating…' : 'Create domain'}
              </Button>
            </div>
          )}

          <div className="space-y-3">
            {domainsQ.isLoading ? (
              <div className="p-6 text-zinc-500 text-sm">Loading domains…</div>
            ) : domainsQ.isError ? (
              <div className="p-6 text-red-600 text-sm">Couldn't load domains. {String(domainsQ.error)}</div>
            ) : (domainsQ.data ?? []).length === 0 ? (
              <div className="p-6 text-zinc-500 text-sm">No domains yet — add one above.</div>
            ) : (
              (domainsQ.data ?? []).map((d) => (
                <div
                  key={d.id}
                  className="flex items-center justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg"
                >
                  <div className="flex items-center gap-3">
                    <Globe className="size-5 text-blue-600" />
                    <div>
                      <div className="font-medium">{d.name}</div>
                      <div className="text-sm text-zinc-500">
                        {usersByDomain.get(d.id) ?? 0} mailboxes
                        {' • '}
                        {d.active ? 'active' : 'inactive'}
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {d.active ? (
                      <CheckCircle className="size-5 text-green-600" />
                    ) : (
                      <AlertCircle className="size-5 text-yellow-600" />
                    )}
                    <Button
                      variant="ghost"
                      size="sm"
                      className="text-red-600 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-950"
                      disabled={deleteDomainMut.isPending}
                      onClick={() => {
                        if (window.confirm(`Delete domain ${d.name}? Mailboxes on it will be removed too.`)) {
                          deleteDomainMut.mutate(d.id);
                        }
                      }}
                    >
                      <Trash2 className="size-4" />
                    </Button>
                  </div>
                </div>
              ))
            )}
          </div>
        </Card>

          </TabsContent>

          <TabsContent value={ADMIN_TAB_AUDIT}>
            <AuditLogTab />
          </TabsContent>

          <TabsContent value={ADMIN_TAB_BRANDING}>
            <BrandingTab />
          </TabsContent>

          <TabsContent value={ADMIN_TAB_SAML}>
            <SamlTab />
          </TabsContent>

          <TabsContent value={ADMIN_TAB_OIDC}>
            <OidcTab />
          </TabsContent>

          <TabsContent value={ADMIN_TAB_LDAP}>
            <LdapTab />
          </TabsContent>

          <TabsContent value={ADMIN_TAB_RETENTION}>
            <RetentionTab />
          </TabsContent>

          <TabsContent value={ADMIN_TAB_LEGAL_HOLDS}>
            <LegalHoldsTab />
          </TabsContent>

          <TabsContent value={ADMIN_TAB_DLP}>
            <DlpTab />
          </TabsContent>

          <TabsContent value={ADMIN_TAB_EDISCOVERY}>
            <EdiscoveryTab />
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}
