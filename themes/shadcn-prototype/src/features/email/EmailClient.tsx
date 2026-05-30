// TMAIL-217: wire to real /api/folders + /api/folders/{folder}/messages.
//
// EmailList + Sidebar still take their original mock-ish shapes; this
// component is the adapter that maps the real backend types to those
// shapes. EmailReader (TMAIL-218) and ComposeModal (TMAIL-219) own their
// own data fetches.
import { useState, useMemo } from 'react';
import { Link } from 'react-router';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Settings, Menu, ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Sidebar } from '@/components/layout/Sidebar';
import { EmailList } from '@/features/email/EmailList';
import { EmailReader } from '@/features/email/EmailReader';
import { ComposeModal } from '@/features/email/ComposeModal';
import { fetchFolders } from '@/api/folders';
import { fetchMessages, flagMessage } from '@/api/messages';
import type {
  Folder as ServerFolder,
  FullMessage,
  MessageEnvelope,
  MessageListResponse,
} from '@/types/mail';
import type { Email, Folder as UiFolder } from '@/types/ui';

// Added: TMAIL-315 — IMAP \Flagged keyword is what backs the alt-UI "starred"
// state. Centralising the literal keeps the optimistic-update + invalidation
// path from drifting from what the backend expects.
const FLAG_STARRED = '\\Flagged';

const FOLDER_ICONS: Record<string, string> = {
  INBOX: 'Inbox',
  Inbox: 'Inbox',
  Sent: 'Send',
  'Sent Items': 'Send',
  Drafts: 'FileText',
  Junk: 'AlertOctagon',
  'Junk Mail': 'AlertOctagon',
  Spam: 'AlertOctagon',
  Trash: 'Trash2',
  'Deleted Items': 'Trash2',
};

export function EmailClient() {
  const [activeFolder, setActiveFolder] = useState('INBOX');
  const [selectedUid, setSelectedUid] = useState<number | null>(null);
  const [isComposing, setIsComposing] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(false);

  const queryClient = useQueryClient();

  const foldersQuery = useQuery<ServerFolder[]>({
    queryKey: ['folders'],
    queryFn: () => fetchFolders(),
  });

  const messagesQuery = useQuery({
    queryKey: ['messages', activeFolder],
    queryFn: () => fetchMessages(activeFolder, 0, 50),
    enabled: !!activeFolder,
  });

  // Added: TMAIL-315 / TMAIL-316 — star toggle mutation. Optimistically flips
  // the IMAP \Flagged keyword on both the cached envelope list AND the cached
  // full-message detail so the UI feels instant whether the click came from
  // the EmailList row (TMAIL-315) or the EmailReader header (TMAIL-316). On
  // settle, invalidate both ['messages', folder] and ['message', folder, uid]
  // so the real IMAP FLAGS reply becomes the source of truth.
  const toggleStarMutation = useMutation({
    mutationFn: async ({ uid, currentlyStarred }: { uid: number; currentlyStarred: boolean }) =>
      flagMessage(activeFolder, uid, FLAG_STARRED, !currentlyStarred),
    onMutate: async ({ uid, currentlyStarred }) => {
      const listKey = ['messages', activeFolder];
      const detailKey = ['message', activeFolder, uid];
      await queryClient.cancelQueries({ queryKey: listKey });
      await queryClient.cancelQueries({ queryKey: detailKey });

      const previousList = queryClient.getQueryData<MessageListResponse>(listKey);
      const previousDetail = queryClient.getQueryData<FullMessage>(detailKey);

      if (previousList?.messages) {
        queryClient.setQueryData<MessageListResponse>(listKey, {
          ...previousList,
          messages: previousList.messages.map((m) => {
            if (m.uid !== uid) return m;
            const without = (m.flags ?? []).filter((f) => !f.includes('Flagged'));
            return {
              ...m,
              flags: currentlyStarred ? without : [...without, FLAG_STARRED],
            };
          }),
        });
      }
      if (previousDetail) {
        const without = (previousDetail.flags ?? []).filter((f) => !f.includes('Flagged'));
        queryClient.setQueryData<FullMessage>(detailKey, {
          ...previousDetail,
          flags: currentlyStarred ? without : [...without, FLAG_STARRED],
        });
      }

      return { previousList, previousDetail, listKey, detailKey };
    },
    onError: (_err, _vars, ctx) => {
      // Roll back to the snapshots taken in onMutate so the star reflects
      // actual server state when the IMAP call fails.
      if (ctx?.previousList) {
        queryClient.setQueryData(ctx.listKey, ctx.previousList);
      }
      if (ctx?.previousDetail) {
        queryClient.setQueryData(ctx.detailKey, ctx.previousDetail);
      }
    },
    onSettled: (_data, _err, vars) => {
      queryClient.invalidateQueries({ queryKey: ['messages', activeFolder] });
      queryClient.invalidateQueries({ queryKey: ['message', activeFolder, vars.uid] });
    },
  });

  // Adapt /api/folders shape → Sidebar's Folder shape.
  const sidebarFolders: UiFolder[] = useMemo(() => {
    const live = foldersQuery.data ?? [];
    return live.map((f) => ({
      id: f.name,
      name: f.name,
      icon: FOLDER_ICONS[f.name] ?? 'Briefcase',
      count: f.unseen ?? 0,
    }));
  }, [foldersQuery.data]);

  // Adapt /api/folders/{folder}/messages → EmailList's Email shape.
  // The shadcn EmailList renders preview/body/attachments — we don't have
  // those in the envelope list, so leave placeholders; the reader (TMAIL-218)
  // hydrates the full body on click.
  const emailListItems: Email[] = useMemo(() => {
    const envelopes: MessageEnvelope[] = messagesQuery.data?.messages ?? [];
    return envelopes.map((m) => ({
      id: String(m.uid),
      from: m.from || '(unknown sender)',
      fromEmail: m.from || '',
      to: '',
      subject: m.subject || '(no subject)',
      preview: '',
      body: '',
      timestamp: m.date ? new Date(m.date) : new Date(),
      read: m.flags?.some((f: string) => f.includes('Seen')) ?? true,
      starred: m.flags?.some((f: string) => f.includes('Flagged')) ?? false,
      folder: activeFolder,
      attachments: undefined,
    }));
  }, [messagesQuery.data, activeFolder]);

  const selectedEmail = emailListItems.find((e) => e.id === String(selectedUid)) ?? null;
  const mobileView = selectedUid != null ? 'reader' : 'list';

  return (
    <div className="flex h-full relative">
      {sidebarOpen && (
        <div className="fixed inset-0 bg-black/40 z-30 md:hidden" onClick={() => setSidebarOpen(false)} />
      )}

      <div
        className={`
          fixed inset-y-0 left-0 z-40 transition-transform duration-300
          md:static md:translate-x-0 md:z-auto
          ${sidebarOpen ? 'translate-x-0' : '-translate-x-full'}
        `}
      >
        <Sidebar
          activeFolder={activeFolder}
          folders={sidebarFolders}
          onFolderChange={(folderId) => {
            setActiveFolder(folderId);
            setSelectedUid(null);
            setSidebarOpen(false);
          }}
          onCompose={() => {
            setIsComposing(true);
            setSidebarOpen(false);
          }}
        />
      </div>

      <div className="flex-1 flex overflow-hidden">
        <div
          className={`
            border-r border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950
            flex flex-col
            w-full md:w-80 lg:w-96
            ${mobileView === 'reader' ? 'hidden md:flex' : 'flex'}
          `}
        >
          <div className="h-14 border-b border-zinc-200 dark:border-zinc-800 flex items-center justify-between px-4 shrink-0">
            <div className="flex items-center gap-2">
              <Button variant="ghost" size="icon" className="md:hidden" onClick={() => setSidebarOpen(true)}>
                <Menu className="size-5" />
              </Button>
              <h2 className="font-semibold capitalize">{activeFolder}</h2>
            </div>
            <Link to="/admin">
              <Button variant="ghost" size="icon" title="Admin Dashboard">
                <Settings className="size-4" />
              </Button>
            </Link>
          </div>
          <div className="flex-1 overflow-y-auto">
            {messagesQuery.isLoading ? (
              <div className="p-6 text-zinc-500 text-sm">Loading messages…</div>
            ) : messagesQuery.isError ? (
              <div className="p-6 text-red-600 text-sm">
                Couldn't load messages. {String(messagesQuery.error)}
              </div>
            ) : (
              <EmailList
                emails={emailListItems}
                selectedEmailId={selectedUid != null ? String(selectedUid) : null}
                onSelectEmail={(id) => setSelectedUid(parseInt(id, 10))}
                onToggleStar={(id, currentlyStarred) =>
                  toggleStarMutation.mutate({ uid: parseInt(id, 10), currentlyStarred })
                }
              />
            )}
          </div>
        </div>

        <div
          className={`
            flex-1 bg-white dark:bg-zinc-950 flex flex-col overflow-hidden
            ${mobileView === 'reader' ? 'flex' : 'hidden md:flex'}
          `}
        >
          {selectedUid != null && (
            <div className="md:hidden h-11 border-b border-zinc-200 dark:border-zinc-800 flex items-center px-3">
              <Button variant="ghost" size="sm" className="gap-1 text-blue-600" onClick={() => setSelectedUid(null)}>
                <ArrowLeft className="size-4" />
                Back
              </Button>
            </div>
          )}
          <div className="flex-1 overflow-hidden">
            <EmailReader
              folder={activeFolder}
              uid={selectedUid}
              listItem={selectedEmail}
              onCompose={() => setIsComposing(true)}
              onToggleStar={(uid, currentlyStarred) =>
                toggleStarMutation.mutate({ uid, currentlyStarred })
              }
            />
          </div>
        </div>
      </div>

      <ComposeModal isOpen={isComposing} onClose={() => setIsComposing(false)} />
    </div>
  );
}
