import { Link } from 'react-router';
import { ArrowLeft, Plus, Trash2, Users, HardDrive, Mail, Globe, CheckCircle, XCircle, AlertCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import { mockMailboxes, mockStats } from '@/data/mockData';

export function AdminDashboard() {
  return (
    <div className="h-full overflow-y-auto bg-zinc-50 dark:bg-zinc-950">
      <div className="max-w-7xl mx-auto p-6 space-y-6">
        {/* Header */}
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

        {/* Stats Cards */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <Card className="p-6">
            <div className="flex items-center justify-between mb-2">
              <Users className="size-8 text-blue-600" />
              <span className="text-sm text-zinc-500">Total</span>
            </div>
            <div className="text-3xl font-semibold">{mockStats.totalUsers}</div>
            <div className="text-sm text-zinc-600 dark:text-zinc-400">Active Mailboxes</div>
          </Card>

          <Card className="p-6">
            <div className="flex items-center justify-between mb-2">
              <HardDrive className="size-8 text-green-600" />
              <span className="text-sm text-zinc-500">
                {Math.round((mockStats.storageUsed / mockStats.storageTotal) * 100)}%
              </span>
            </div>
            <div className="text-3xl font-semibold">
              {mockStats.storageUsed} GB
            </div>
            <div className="text-sm text-zinc-600 dark:text-zinc-400">
              of {mockStats.storageTotal} GB used
            </div>
          </Card>

          <Card className="p-6">
            <div className="flex items-center justify-between mb-2">
              <Mail className="size-8 text-purple-600" />
              <span className="text-sm text-zinc-500">Today</span>
            </div>
            <div className="text-3xl font-semibold">{mockStats.messagesToday}</div>
            <div className="text-sm text-zinc-600 dark:text-zinc-400">
              {mockStats.messagesThisWeek} this week
            </div>
          </Card>

          <Card className="p-6">
            <div className="flex items-center justify-between mb-2">
              <Globe className="size-8 text-orange-600" />
              <span className="text-sm text-zinc-500">Active</span>
            </div>
            <div className="text-3xl font-semibold">{mockStats.activeDomains}</div>
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
              />
              <Button>
                <Plus className="size-4 mr-2" />
                Add Mailbox
              </Button>
            </div>
          </div>

          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-zinc-200 dark:border-zinc-800">
                  <th className="text-left py-3 px-4 font-medium">User</th>
                  <th className="text-left py-3 px-4 font-medium">Domain</th>
                  <th className="text-left py-3 px-4 font-medium">Quota Used</th>
                  <th className="text-left py-3 px-4 font-medium">Quota Total</th>
                  <th className="text-left py-3 px-4 font-medium">Status</th>
                  <th className="text-right py-3 px-4 font-medium">Actions</th>
                </tr>
              </thead>
              <tbody>
                {mockMailboxes.map((mailbox) => {
                  const quotaPercent = (mailbox.quotaUsed / mailbox.quotaTotal) * 100;
                  return (
                    <tr
                      key={mailbox.id}
                      className="border-b border-zinc-200 dark:border-zinc-800 hover:bg-zinc-50 dark:hover:bg-zinc-900"
                    >
                      <td className="py-3 px-4 font-medium">{mailbox.user}</td>
                      <td className="py-3 px-4 text-zinc-600 dark:text-zinc-400">
                        {mailbox.domain}
                      </td>
                      <td className="py-3 px-4">
                        <div className="flex items-center gap-2">
                          <div className="flex-1 h-2 bg-zinc-200 dark:bg-zinc-800 rounded-full overflow-hidden max-w-[100px]">
                            <div
                              className={`h-full ${
                                quotaPercent > 90
                                  ? 'bg-red-600'
                                  : quotaPercent > 70
                                  ? 'bg-yellow-600'
                                  : 'bg-green-600'
                              }`}
                              style={{ width: `${quotaPercent}%` }}
                            />
                          </div>
                          <span className="text-sm">{mailbox.quotaUsed} GB</span>
                        </div>
                      </td>
                      <td className="py-3 px-4 text-zinc-600 dark:text-zinc-400">
                        {mailbox.quotaTotal} GB
                      </td>
                      <td className="py-3 px-4">
                        <div className="flex items-center gap-2">
                          {mailbox.status === 'active' && (
                            <>
                              <CheckCircle className="size-4 text-green-600" />
                              <span className="text-green-600">Active</span>
                            </>
                          )}
                          {mailbox.status === 'suspended' && (
                            <>
                              <AlertCircle className="size-4 text-yellow-600" />
                              <span className="text-yellow-600">Suspended</span>
                            </>
                          )}
                          {mailbox.status === 'disabled' && (
                            <>
                              <XCircle className="size-4 text-red-600" />
                              <span className="text-red-600">Disabled</span>
                            </>
                          )}
                        </div>
                      </td>
                      <td className="py-3 px-4 text-right">
                        <Button variant="ghost" size="sm">
                          Edit
                        </Button>
                        <Button variant="ghost" size="sm" className="text-red-600 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-950">
                          <Trash2 className="size-4" />
                        </Button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </Card>

        {/* Domain Management */}
        <Card className="p-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-xl font-semibold">Domain Management</h2>
            <Button>
              <Plus className="size-4 mr-2" />
              Add Domain
            </Button>
          </div>

          <div className="space-y-3">
            <div className="flex items-center justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg">
              <div className="flex items-center gap-3">
                <Globe className="size-5 text-blue-600" />
                <div>
                  <div className="font-medium">mydomain.com</div>
                  <div className="text-sm text-zinc-500">8 mailboxes • DNS configured</div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <CheckCircle className="size-5 text-green-600" />
                <Button variant="outline" size="sm">Manage DNS</Button>
                <Button variant="ghost" size="sm">Settings</Button>
              </div>
            </div>

            <div className="flex items-center justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg">
              <div className="flex items-center gap-3">
                <Globe className="size-5 text-blue-600" />
                <div>
                  <div className="font-medium">seconddomain.com</div>
                  <div className="text-sm text-zinc-500">2 mailboxes • DNS pending</div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <AlertCircle className="size-5 text-yellow-600" />
                <Button variant="outline" size="sm">Manage DNS</Button>
                <Button variant="ghost" size="sm">Settings</Button>
              </div>
            </div>

            <div className="flex items-center justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg">
              <div className="flex items-center gap-3">
                <Globe className="size-5 text-blue-600" />
                <div>
                  <div className="font-medium">thirddomain.net</div>
                  <div className="text-sm text-zinc-500">2 mailboxes • DNS configured</div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <CheckCircle className="size-5 text-green-600" />
                <Button variant="outline" size="sm">Manage DNS</Button>
                <Button variant="ghost" size="sm">Settings</Button>
              </div>
            </div>
          </div>
        </Card>
      </div>
    </div>
  );
}
