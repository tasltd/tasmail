// TMAIL-217: wire to real /api/folders + /api/folders/{folder}/messages.
//
// EmailList + Sidebar still take their original mock-ish shapes; this
// component is the adapter that maps the real backend types to those
// shapes. EmailReader (TMAIL-218) and ComposeModal (TMAIL-219) own their
// own data fetches.
import { useEffect, useState, useMemo } from 'react';
import { Link, useSearchParams } from 'react-router';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Settings, Menu, ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Sidebar } from '@/components/layout/Sidebar';
import { EmailList } from '@/features/email/EmailList';
import { EmailReader } from '@/features/email/EmailReader';
import { ComposeModal } from '@/features/email/ComposeModal';
import { buildReplyContext, type ReplyContext, type ReplyKind } from '@/features/email/replyContext';
import { fetchFolders } from '@/api/folders';
import { deleteMessage, fetchMessages, flagMessage, moveMessage } from '@/api/messages';
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

// Added: TMAIL-317 — destination folder for the EmailReader Archive button.
// The backend `move_message` will auto-CREATE this on first use if the IMAP
// server doesn't already have it (see imap_service.rs TMAIL-317 note).
const ARCHIVE_FOLDER = 'Archive';

// Added: TMAIL-318 — trash-like folder names. The backend `delete_message`
// resolves the per-user trash folder (Stalwart "Deleted Items", Dovecot
// "Trash", Gmail "[Gmail]/Trash" — see imap_service.rs::trash_folder()).
// The frontend uses the set below only to decide whether the active folder
// is the trash folder so it can prompt for confirmation before triggering a
// permanent EXPUNGE. The backend is still the authority on routing — when
// activeFolder is not the trash folder, DELETE soft-deletes by moving to the
// resolved trash folder, so we don't need to know its exact name to call it.
const TRASH_FOLDER_NAMES = new Set(['Trash', 'Deleted Items', 'Bin']);

function isTrashFolder(folderName: string): boolean {
  return TRASH_FOLDER_NAMES.has(folderName);
}

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
  // Added (TMAIL-322): when SearchResultsPage links into `/?folder=X&uid=Y`,
  // seed the active folder + selected UID from the URL so the reader pane
  // opens on the chosen message. Defaults stay INBOX + nothing-selected
  // for the normal "open the mailbox" case where no params are present.
  const [searchParams] = useSearchParams();
  const initialFolder = searchParams.get('folder')?.trim() || 'INBOX';
  const initialUidRaw = searchParams.get('uid');
  const initialUid =
    initialUidRaw && /^\d+$/.test(initialUidRaw) ? parseInt(initialUidRaw, 10) : null;

  const [activeFolder, setActiveFolder] = useState(initialFolder);
  const [selectedUid, setSelectedUid] = useState<number | null>(initialUid);
  const [isComposing, setIsComposing] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(false);

  // Added (TMAIL-322): re-apply deep-link params if the user clicks a second
  // search result while EmailClient is already mounted (so we re-select the
  // new uid). Skips when the URL has no params so the user's in-app folder
  // navigation isn't fought by the effect.
  useEffect(() => {
    const f = searchParams.get('folder')?.trim();
    const u = searchParams.get('uid');
    if (f) setActiveFolder(f);
    if (u && /^\d+$/.test(u)) {
      setSelectedUid(parseInt(u, 10));
    }
  }, [searchParams]);
  // Added: TMAIL-319 — the active Reply / Reply All / Forward prefill payload
  // for ComposeModal. EmailReader passes the loaded FullMessage + the kind
  // (reply | replyAll | forward) when its toolbar buttons fire; we then build
  // the ReplyContext here (so the helper stays a pure module call) and stash
  // it for the composer to consume. `null` means "open a blank compose" — the
  // sidebar's floating Compose button takes that path.
  const [replyContext, setReplyContext] = useState<ReplyContext | null>(null);

  // Added: TMAIL-319 — the logged-in user's own address. Used by the Reply All
  // builder to filter the user out of `to` / `cc` so they don't email
  // themselves a copy of their own reply. Pulled out of the JWT once at mount
  // so the modal doesn't need to re-decode it.
  const selfAddress = useMemo<string | null>(() => {
    const token = typeof window !== 'undefined' ? window.localStorage.getItem('access_token') : null;
    if (!token) return null;
    try {
      const [, payload] = token.split('.');
      if (!payload) return null;
      // JWT base64url → base64 then atob. Lossy decode (no UTF-8 reconstruction)
      // is fine: email addresses are ASCII per RFC 5321 §4.5.3.
      const json = atob(payload.replace(/-/g, '+').replace(/_/g, '/'));
      const parsed = JSON.parse(json) as { email?: string; sub?: string };
      return (parsed.email ?? null)?.toLowerCase() ?? null;
    } catch {
      // Malformed JWT — fall back gracefully; Reply All just won't filter
      // self addresses, which is annoying but not broken.
      return null;
    }
  }, []);

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

  // Added: TMAIL-317 — Archive mutation. Moves the message to the IMAP
  // "Archive" folder via /api/folders/{folder}/messages/{uid}/move. Optimistically
  // drops the row from the cached envelope list so the reader-pane Archive
  // click feels instant, then on settle invalidates ['messages', folder] +
  // ['folders'] so envelope counts and the (possibly newly-created) Archive
  // folder appear in the sidebar. Always clears selectedUid so the reader
  // pane returns to the empty state (the archived UID is no longer present
  // in the active folder).
  const archiveMutation = useMutation({
    mutationFn: async ({ uid }: { uid: number }) =>
      moveMessage(activeFolder, uid, ARCHIVE_FOLDER),
    onMutate: async ({ uid }) => {
      const listKey = ['messages', activeFolder];
      await queryClient.cancelQueries({ queryKey: listKey });

      const previousList = queryClient.getQueryData<MessageListResponse>(listKey);
      if (previousList?.messages) {
        queryClient.setQueryData<MessageListResponse>(listKey, {
          ...previousList,
          messages: previousList.messages.filter((m) => m.uid !== uid),
        });
      }
      // Clear reader selection immediately — the archived UID is no longer
      // valid in the active folder. Done in onMutate (not onSuccess) so the
      // reader pane updates with the optimistic list rather than briefly
      // showing a stale message body.
      setSelectedUid(null);
      return { previousList, listKey };
    },
    onError: (_err, _vars, ctx) => {
      if (ctx?.previousList) {
        queryClient.setQueryData(ctx.listKey, ctx.previousList);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['messages', activeFolder] });
      // Archive may have just been created (TMAIL-317 backend CREATE retry)
      // — refresh the sidebar so the new folder + unseen count appear.
      queryClient.invalidateQueries({ queryKey: ['folders'] });
    },
  });

  // Added: TMAIL-318 — Delete mutation. Calls DELETE /api/folders/{folder}/
  // messages/{uid} which the backend routes per-user: from any non-trash
  // folder it moves the message to the resolved trash folder (Stalwart
  // "Deleted Items", Dovecot "Trash"); from the trash folder itself it does
  // a permanent +FLAGS \Deleted + EXPUNGE. Optimistically drops the row from
  // the cached envelope list so the click feels instant whether soft or
  // permanent. Clears selectedUid in onMutate so the reader pane returns to
  // the empty state — the deleted UID is no longer present in the active
  // folder. On settle invalidates ['messages', folder] AND ['folders'] so
  // envelope counts (and the trash folder unseen badge, on soft delete)
  // refresh from the live backend. The permanent-delete confirm prompt is
  // owned by the EmailReader handler below — by the time the mutation fires
  // the user has already confirmed (mirrors AdminDashboard's window.confirm
  // pattern for destructive admin actions).
  const deleteMutation = useMutation({
    mutationFn: async ({ uid }: { uid: number }) => deleteMessage(activeFolder, uid),
    onMutate: async ({ uid }) => {
      const listKey = ['messages', activeFolder];
      await queryClient.cancelQueries({ queryKey: listKey });

      const previousList = queryClient.getQueryData<MessageListResponse>(listKey);
      if (previousList?.messages) {
        queryClient.setQueryData<MessageListResponse>(listKey, {
          ...previousList,
          messages: previousList.messages.filter((m) => m.uid !== uid),
        });
      }
      setSelectedUid(null);
      return { previousList, listKey };
    },
    onError: (_err, _vars, ctx) => {
      if (ctx?.previousList) {
        queryClient.setQueryData(ctx.listKey, ctx.previousList);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['messages', activeFolder] });
      queryClient.invalidateQueries({ queryKey: ['folders'] });
    },
  });

  // Added: TMAIL-318 — true when the active folder is the user's trash
  // folder. Drives both the reader Delete button's aria-label and the
  // confirm() gate on the onDelete handler. Uses a name-based heuristic
  // (TRASH_FOLDER_NAMES) because GET /api/folders does not yet surface a
  // role/kind field per folder.
  const isPermanentDelete = isTrashFolder(activeFolder);

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
            // Sidebar Compose button = blank compose. Drop any stale reply
            // context so the modal opens with empty fields.
            setReplyContext(null);
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
              // Added: TMAIL-319 — build the ReplyContext here so the helper
              // stays a pure function and EmailClient owns "which compose are
              // we in" state. `kind == null` means the caller wanted a blank
              // compose (kept for shape-compatibility with the legacy zero-arg
              // signature — currently unused by EmailReader but documented so
              // future call sites have an obvious blank path).
              onCompose={(kind: ReplyKind | null, message: FullMessage | null) => {
                if (kind && message) {
                  setReplyContext(buildReplyContext(message, kind, selfAddress));
                } else {
                  setReplyContext(null);
                }
                setIsComposing(true);
              }}
              onToggleStar={(uid, currentlyStarred) =>
                toggleStarMutation.mutate({ uid, currentlyStarred })
              }
              onArchive={(uid) => archiveMutation.mutate({ uid })}
              onDelete={(uid) => {
                // TMAIL-318: window.confirm() gates permanent EXPUNGE. The
                // soft-delete (move-to-trash) path skips confirmation —
                // matches Gmail/Outlook UX where moving to trash is one-click
                // recoverable and only permanent expunge needs a prompt.
                if (isPermanentDelete) {
                  const ok = window.confirm(
                    'Permanently delete this email? This cannot be undone.',
                  );
                  if (!ok) return;
                }
                deleteMutation.mutate({ uid });
              }}
              isPermanentDelete={isPermanentDelete}
            />
          </div>
        </div>
      </div>

      <ComposeModal
        isOpen={isComposing}
        // Drop the reply context on close so re-opening via the floating
        // Compose button starts blank rather than re-prefilling the last
        // reply.
        onClose={() => {
          setIsComposing(false);
          setReplyContext(null);
        }}
        replyContext={replyContext}
      />
    </div>
  );
}
