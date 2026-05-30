// TMAIL-217: wire to real /api/folders + /api/folders/{folder}/messages.
//
// EmailList + Sidebar still take their original mock-ish shapes; this
// component is the adapter that maps the real backend types to those
// shapes. EmailReader (TMAIL-218) and ComposeModal (TMAIL-219) own their
// own data fetches.
import { useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router';
import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
  type InfiniteData,
} from '@tanstack/react-query';
import {
  Settings,
  Menu,
  ArrowLeft,
  X,
  Mail,
  MailOpen,
  Star as StarIcon,
  Archive as ArchiveIcon,
  Trash2,
  FolderInput,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Sidebar } from '@/components/layout/Sidebar';
import { EmailList } from '@/features/email/EmailList';
import { EmailReader } from '@/features/email/EmailReader';
import { ComposeModal } from '@/features/email/ComposeModal';
import { buildReplyContext, type ReplyContext, type ReplyKind } from '@/features/email/replyContext';
import { fetchFolders, createFolder, deleteFolder } from '@/api/folders';
import { deleteMessage, fetchMessages, flagMessage, moveMessage } from '@/api/messages';
import {
  MESSAGES_PAGE_SIZE,
  nextMessagesPageParam,
  updateInfiniteMessages,
} from '@/features/email/messagesCache';
import {
  clearSelection,
  isAllSelected,
  isPartiallySelected,
  rangeSelect,
  selectAll,
  toggleSelection,
} from '@/features/email/bulkSelection';
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

// Added: TMAIL-326 — IMAP \Seen keyword backs the "read" state. The bulk
// action bar exposes Mark Read / Mark Unread which add or remove this flag
// across every selected uid via the same /flag endpoint the single-row
// star toggle uses.
const FLAG_SEEN = '\\Seen';

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

// TMAIL-324: built-in folders the sidebar must NOT render a delete (×) button
// for. Mirrors handlers/folders.rs::PROTECTED_FOLDER_NAMES so the UI affordance
// matches what the backend will accept.
const BUILT_IN_FOLDER_NAMES = new Set([
  'INBOX',
  'Inbox',
  'Sent',
  'Sent Items',
  'Drafts',
  'Trash',
  'Deleted Items',
  'Bin',
  'Junk',
  'Junk Mail',
  'Spam',
  'Archive',
]);

function isBuiltInFolderName(name: string): boolean {
  if (BUILT_IN_FOLDER_NAMES.has(name)) return true;
  const lower = name.toLowerCase();
  for (const builtin of BUILT_IN_FOLDER_NAMES) {
    if (builtin.toLowerCase() === lower) return true;
  }
  return false;
}

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

  // Added (TMAIL-326): multi-select state for the bulk-action bar. The Set
  // holds raw IMAP uids (numbers); the EmailList works in string ids so the
  // forward/back boundary converts between them. The anchor tracks the most
  // recently single-toggled uid so shift-click range select knows where to
  // start the range. Both reset when the user changes folders so a stale
  // selection from Inbox doesn't accidentally drive a bulk action on Sent.
  const [selectedUids, setSelectedUids] = useState<Set<number>>(() => new Set());
  const [selectionAnchorUid, setSelectionAnchorUid] = useState<number | null>(null);

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

  // TMAIL-324: real folder CRUD against POST/DELETE /api/folders. The Sidebar
  // used to keep its own `extraLocalFolders` state which evaporated on reload
  // and never reached the IMAP server. These two mutations replace it — on
  // success we invalidate ['folders'] so the live list re-fetches and shows
  // (or stops showing) the affected folder.
  const createFolderMutation = useMutation({
    mutationFn: (name: string) => createFolder(name),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['folders'] });
    },
    onError: (err: unknown) => {
      // Surface a basic alert so the user sees server-side validation errors
      // (e.g. "INBOX is a built-in folder and cannot be created or deleted").
      // The alt-UI doesn't have a global toast layer yet — TMAIL-324 follow-up
      // can swap this for the shadcn toast once it's wired.
      const msg = err instanceof Error ? err.message : 'Failed to create folder';
      if (typeof window !== 'undefined') {
        window.alert(msg);
      }
    },
  });

  const deleteFolderMutation = useMutation({
    mutationFn: (name: string) => deleteFolder(name),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['folders'] });
    },
    onError: (err: unknown) => {
      const msg = err instanceof Error ? err.message : 'Failed to delete folder';
      if (typeof window !== 'undefined') {
        window.alert(msg);
      }
    },
  });

  // Changed (TMAIL-325): switched from a fixed-50 useQuery to useInfiniteQuery
  // so the EmailList can grow as the user scrolls. Each page request goes to
  // `/api/folders/{folder}/messages?page=N&page_size=50` (backend already
  // paginated — handlers/messages.rs::list_messages). `getNextPageParam`
  // returns undefined once we've consumed all `total` envelopes so the
  // intersection-observer sentinel stops trying to fetch.
  const messagesQuery = useInfiniteQuery({
    queryKey: ['messages', activeFolder],
    queryFn: ({ pageParam }) =>
      fetchMessages(activeFolder, pageParam as number, MESSAGES_PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: nextMessagesPageParam,
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

      // Changed (TMAIL-325): cache shape is now InfiniteData<MessageListResponse>
      // so the optimistic flag flip must walk every loaded page rather than the
      // single old payload. updateInfiniteMessages is exported above so the
      // unit tests can exercise the paginated walk without standing up React.
      const previousList = queryClient.getQueryData<InfiniteData<MessageListResponse>>(listKey);
      const previousDetail = queryClient.getQueryData<FullMessage>(detailKey);

      if (previousList) {
        queryClient.setQueryData<InfiniteData<MessageListResponse>>(
          listKey,
          updateInfiniteMessages(previousList, (msgs) =>
            msgs.map((m) => {
              if (m.uid !== uid) return m;
              const without = (m.flags ?? []).filter((f) => !f.includes('Flagged'));
              return {
                ...m,
                flags: currentlyStarred ? without : [...without, FLAG_STARRED],
              };
            }),
          ),
        );
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

      // Changed (TMAIL-325): cache shape is now InfiniteData<MessageListResponse>
      // — drop the archived row from every loaded page so the reader pane's
      // optimistic update behaves the same whether the row was on page 0 or
      // page 5 of an infinite-scrolled inbox.
      const previousList = queryClient.getQueryData<InfiniteData<MessageListResponse>>(listKey);
      if (previousList) {
        queryClient.setQueryData<InfiniteData<MessageListResponse>>(
          listKey,
          updateInfiniteMessages(previousList, (msgs) =>
            msgs.filter((m) => m.uid !== uid),
          ),
        );
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

      // Changed (TMAIL-325): same paginated-cache traversal as archiveMutation
      // above — the user may be deleting a row that lives on a later page than
      // the initial 50 envelopes.
      const previousList = queryClient.getQueryData<InfiniteData<MessageListResponse>>(listKey);
      if (previousList) {
        queryClient.setQueryData<InfiniteData<MessageListResponse>>(
          listKey,
          updateInfiniteMessages(previousList, (msgs) =>
            msgs.filter((m) => m.uid !== uid),
          ),
        );
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

  // Added (TMAIL-326): bulk flag mutation. Adds OR removes the same flag
  // (\Seen for read/unread, \Flagged for star/unstar) across every selected
  // uid by firing the per-UID /flag endpoint in parallel via
  // Promise.allSettled — keeps a single failure from aborting the rest of
  // the batch the way Promise.all would. Optimistically updates every
  // affected envelope in the InfiniteData cache so the EmailList reflects
  // the new flags instantly; on settle the IMAP FLAGS reply re-syncs via
  // ['messages', activeFolder] invalidation.
  const bulkFlagMutation = useMutation({
    mutationFn: async ({ uids, flag, add }: { uids: number[]; flag: string; add: boolean }) => {
      const results = await Promise.allSettled(
        uids.map((uid) => flagMessage(activeFolder, uid, flag, add)),
      );
      // Surface the first error so the onError rollback runs when any
      // individual call failed. We deliberately do NOT throw on partial
      // success — the optimistic update + invalidation already reconcile.
      const allFailed = results.every((r) => r.status === 'rejected');
      if (allFailed && results.length > 0) {
        const first = results[0] as PromiseRejectedResult;
        throw first.reason instanceof Error ? first.reason : new Error('Bulk flag failed');
      }
    },
    onMutate: async ({ uids, flag, add }) => {
      const listKey = ['messages', activeFolder];
      await queryClient.cancelQueries({ queryKey: listKey });
      const previousList = queryClient.getQueryData<InfiniteData<MessageListResponse>>(listKey);
      const uidSet = new Set(uids);
      if (previousList) {
        queryClient.setQueryData<InfiniteData<MessageListResponse>>(
          listKey,
          updateInfiniteMessages(previousList, (msgs) =>
            msgs.map((m) => {
              if (!uidSet.has(m.uid)) return m;
              // Use a substring match (Flagged / Seen) for robustness against
              // backslash escaping in the cached values — mirrors the
              // toggleStarMutation pattern above.
              const flagBareName = flag.replace(/\\/g, '');
              const without = (m.flags ?? []).filter(
                (f) => !f.includes(flagBareName),
              );
              return {
                ...m,
                flags: add ? [...without, flag] : without,
              };
            }),
          ),
        );
      }
      return { previousList, listKey };
    },
    onError: (_err, _vars, ctx) => {
      if (ctx?.previousList) {
        queryClient.setQueryData(ctx.listKey, ctx.previousList);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['messages', activeFolder] });
      // Read/unread changes alter folder unseen counts — refresh the sidebar
      // so the badge stays in lockstep with the bulk action.
      queryClient.invalidateQueries({ queryKey: ['folders'] });
    },
  });

  // Added (TMAIL-326): bulk move. Same shape as archiveMutation but
  // (a) drops every selected uid optimistically and (b) clears the bulk
  // selection + reader pane in onMutate so the UI feels instant. Used for
  // both the toolbar Archive button (move → "Archive") and the Move-to
  // dropdown (move → any folder).
  const bulkMoveMutation = useMutation({
    mutationFn: async ({ uids, toFolder }: { uids: number[]; toFolder: string }) => {
      const results = await Promise.allSettled(
        uids.map((uid) => moveMessage(activeFolder, uid, toFolder)),
      );
      const allFailed = results.every((r) => r.status === 'rejected');
      if (allFailed && results.length > 0) {
        const first = results[0] as PromiseRejectedResult;
        throw first.reason instanceof Error ? first.reason : new Error('Bulk move failed');
      }
    },
    onMutate: async ({ uids }) => {
      const listKey = ['messages', activeFolder];
      await queryClient.cancelQueries({ queryKey: listKey });
      const previousList = queryClient.getQueryData<InfiniteData<MessageListResponse>>(listKey);
      const uidSet = new Set(uids);
      if (previousList) {
        queryClient.setQueryData<InfiniteData<MessageListResponse>>(
          listKey,
          updateInfiniteMessages(previousList, (msgs) =>
            msgs.filter((m) => !uidSet.has(m.uid)),
          ),
        );
      }
      // If the open reader is one of the moved uids, clear it so the empty
      // state appears rather than a 404 from the next fetchMessage.
      if (selectedUid != null && uidSet.has(selectedUid)) {
        setSelectedUid(null);
      }
      setSelectedUids(clearSelection());
      setSelectionAnchorUid(null);
      return { previousList, listKey };
    },
    onError: (_err, _vars, ctx) => {
      if (ctx?.previousList) {
        queryClient.setQueryData(ctx.listKey, ctx.previousList);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['messages', activeFolder] });
      // Move may have just created a destination folder (Archive in
      // particular — backend creates on first use), so refresh the sidebar.
      queryClient.invalidateQueries({ queryKey: ['folders'] });
    },
  });

  // Added (TMAIL-326): bulk delete. Same routing as the single-row delete —
  // backend soft-deletes by moving to the per-user trash folder from any
  // non-trash folder, and permanently expunges from the trash folder. The
  // window.confirm() prompt for permanent delete is owned by the bulk-action
  // bar handler below.
  const bulkDeleteMutation = useMutation({
    mutationFn: async ({ uids }: { uids: number[] }) => {
      const results = await Promise.allSettled(
        uids.map((uid) => deleteMessage(activeFolder, uid)),
      );
      const allFailed = results.every((r) => r.status === 'rejected');
      if (allFailed && results.length > 0) {
        const first = results[0] as PromiseRejectedResult;
        throw first.reason instanceof Error ? first.reason : new Error('Bulk delete failed');
      }
    },
    onMutate: async ({ uids }) => {
      const listKey = ['messages', activeFolder];
      await queryClient.cancelQueries({ queryKey: listKey });
      const previousList = queryClient.getQueryData<InfiniteData<MessageListResponse>>(listKey);
      const uidSet = new Set(uids);
      if (previousList) {
        queryClient.setQueryData<InfiniteData<MessageListResponse>>(
          listKey,
          updateInfiniteMessages(previousList, (msgs) =>
            msgs.filter((m) => !uidSet.has(m.uid)),
          ),
        );
      }
      if (selectedUid != null && uidSet.has(selectedUid)) {
        setSelectedUid(null);
      }
      setSelectedUids(clearSelection());
      setSelectionAnchorUid(null);
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
  // TMAIL-324: tag any non-built-in folder as `isCustom` so the sidebar shows
  // the delete (×) button on hover. The set of built-ins mirrors the
  // backend's PROTECTED_FOLDER_NAMES list in handlers/folders.rs — keep the
  // two in sync if either is extended.
  const sidebarFolders: UiFolder[] = useMemo(() => {
    const live = foldersQuery.data ?? [];
    return live.map((f) => ({
      id: f.name,
      name: f.name,
      icon: FOLDER_ICONS[f.name] ?? 'Briefcase',
      count: f.unseen ?? 0,
      isCustom: !isBuiltInFolderName(f.name),
    }));
  }, [foldersQuery.data]);

  // Adapt /api/folders/{folder}/messages → EmailList's Email shape.
  // The shadcn EmailList renders preview/body/attachments — we don't have
  // those in the envelope list, so leave placeholders; the reader (TMAIL-218)
  // hydrates the full body on click.
  //
  // Changed (TMAIL-325): with useInfiniteQuery the envelope set now lives at
  // `data.pages[].messages` rather than `data.messages`. Flatten across loaded
  // pages so the EmailList keeps rendering one continuous scroll while new
  // pages are appended by the intersection-observer sentinel below.
  const emailListItems: Email[] = useMemo(() => {
    const envelopes: MessageEnvelope[] =
      messagesQuery.data?.pages.flatMap((p) => p.messages) ?? [];
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

  // Added (TMAIL-326): ordered list of visible uids — the source of truth
  // for shift-click range select. Recomputed from emailListItems so it stays
  // in lockstep with what the EmailList renders, including newly paginated
  // pages appended by the IntersectionObserver sentinel.
  const visibleUids = useMemo<number[]>(
    () => emailListItems.map((e) => parseInt(e.id, 10)),
    [emailListItems],
  );

  // Added (TMAIL-326): EmailList ids are strings; the helpers + mutations
  // work in numeric uids. selectedIds is a derived string-set passed down to
  // the list for checkbox state. Memoised so EmailList doesn't see a fresh
  // Set identity on every parent render.
  const selectedIds = useMemo<Set<string>>(() => {
    const s = new Set<string>();
    for (const uid of selectedUids) s.add(String(uid));
    return s;
  }, [selectedUids]);

  // Added (TMAIL-326): EmailList click forwards (id, shiftKey). Route shift
  // to range-select against the visible list and the current anchor;
  // otherwise toggle the single uid and reset the anchor to the just-clicked
  // row so a subsequent shift-click extends from there (matches Gmail).
  const handleToggleSelect = (id: string, shiftKey: boolean) => {
    const uid = parseInt(id, 10);
    if (Number.isNaN(uid)) return;
    if (shiftKey) {
      setSelectedUids((prev) =>
        rangeSelect(prev, visibleUids, selectionAnchorUid, uid),
      );
      // Range-select leaves the anchor at the last single-toggled row so
      // the user can extend further with another shift-click without first
      // resetting the anchor.
      return;
    }
    setSelectedUids((prev) => toggleSelection(prev, uid));
    setSelectionAnchorUid(uid);
  };

  // Added (TMAIL-326): bulk-action bar derived state. `selectedCount` drives
  // visibility of the bar, the toolbar labels, and the "Select all" /
  // indeterminate state of the master checkbox above the list.
  const selectedCount = selectedUids.size;
  const allVisibleSelected = isAllSelected(visibleUids, selectedUids);
  const someVisibleSelected = isPartiallySelected(visibleUids, selectedUids);

  // Added (TMAIL-326): true iff EVERY selected uid currently in the visible
  // list is marked read (\Seen). Drives the "Mark as read" vs "Mark as
  // unread" label on the bulk-action bar so the button reflects the action
  // the user is about to take. Heuristic — operates on the visible
  // envelopes; uids that have scrolled out fall back to "mark as read"
  // because we don't have their flag state cached. That's strictly safer
  // than guessing wrong (marking already-read items read is a no-op IMAP
  // command, marking unread items read by mistake is destructive UX).
  const allSelectedRead = useMemo(() => {
    if (selectedCount === 0) return false;
    let seenAtLeastOne = false;
    for (const e of emailListItems) {
      const uid = parseInt(e.id, 10);
      if (!selectedUids.has(uid)) continue;
      seenAtLeastOne = true;
      if (!e.read) return false;
    }
    return seenAtLeastOne;
  }, [emailListItems, selectedUids, selectedCount]);

  // Added (TMAIL-326): true iff EVERY selected uid currently visible is
  // starred. Same heuristic as `allSelectedRead`.
  const allSelectedStarred = useMemo(() => {
    if (selectedCount === 0) return false;
    let seenAtLeastOne = false;
    for (const e of emailListItems) {
      const uid = parseInt(e.id, 10);
      if (!selectedUids.has(uid)) continue;
      seenAtLeastOne = true;
      if (!e.starred) return false;
    }
    return seenAtLeastOne;
  }, [emailListItems, selectedUids, selectedCount]);

  // Added (TMAIL-326): folders the bulk-action bar's Move-to dropdown
  // offers. Excludes the active folder (moving to where you already are is
  // a no-op) and excludes folders the IMAP server does not let us move
  // to — Drafts has no real "move into" semantics for non-draft messages,
  // Junk/Spam are technically valid but better handled by a future
  // "Report Spam" affordance per Gmail conventions.
  const moveTargetFolders = useMemo<UiFolder[]>(() => {
    const blocked = new Set(['Drafts', 'Junk', 'Junk Mail', 'Spam']);
    return sidebarFolders.filter((f) => f.id !== activeFolder && !blocked.has(f.id));
  }, [sidebarFolders, activeFolder]);

  // Added (TMAIL-326): handlers for the bulk-action bar. Each one snapshots
  // the current uid array (so the mutation doesn't race a concurrent
  // setSelectedUids), then fires the matching mutation. Mutations clear the
  // selection themselves in onMutate so the UI returns to its idle state.
  const uidsArray = () => Array.from(selectedUids);
  const handleBulkMarkRead = () =>
    bulkFlagMutation.mutate({ uids: uidsArray(), flag: FLAG_SEEN, add: true });
  const handleBulkMarkUnread = () =>
    bulkFlagMutation.mutate({ uids: uidsArray(), flag: FLAG_SEEN, add: false });
  const handleBulkStar = () =>
    bulkFlagMutation.mutate({ uids: uidsArray(), flag: FLAG_STARRED, add: true });
  const handleBulkUnstar = () =>
    bulkFlagMutation.mutate({ uids: uidsArray(), flag: FLAG_STARRED, add: false });
  const handleBulkArchive = () =>
    bulkMoveMutation.mutate({ uids: uidsArray(), toFolder: ARCHIVE_FOLDER });
  const handleBulkDelete = () => {
    // Mirror the single-row delete UX: permanent expunge needs a confirm
    // gate, soft-delete (move to trash) is one-click recoverable.
    if (isPermanentDelete) {
      const ok = window.confirm(
        `Permanently delete ${selectedCount} email${
          selectedCount === 1 ? '' : 's'
        }? This cannot be undone.`,
      );
      if (!ok) return;
    }
    bulkDeleteMutation.mutate({ uids: uidsArray() });
  };
  const handleBulkMoveTo = (toFolder: string) =>
    bulkMoveMutation.mutate({ uids: uidsArray(), toFolder });
  const handleToggleSelectAll = () => {
    if (allVisibleSelected) {
      setSelectedUids(clearSelection());
      setSelectionAnchorUid(null);
    } else {
      setSelectedUids(selectAll(visibleUids));
    }
  };
  const handleClearSelection = () => {
    setSelectedUids(clearSelection());
    setSelectionAnchorUid(null);
  };

  // NOTE (TMAIL-326): the master checkbox visually shows a checked/unchecked
  // glyph but the surrounding label conveys partial selection ("N selected"
  // with N < total visible). Radix's @1.1.4 <Checkbox> doesn't expose an
  // `indeterminate` prop and the alt-UI is still on React 18, so we don't
  // wire a ref-based aria-checked="mixed" — the label + visible-count badge
  // already tell the user the state is partial.

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
            // TMAIL-326: drop multi-select state across folder boundaries so
            // a bulk action can't accidentally fire against the wrong inbox.
            setSelectedUids(clearSelection());
            setSelectionAnchorUid(null);
            setSidebarOpen(false);
          }}
          onCompose={() => {
            // Sidebar Compose button = blank compose. Drop any stale reply
            // context so the modal opens with empty fields.
            setReplyContext(null);
            setIsComposing(true);
            setSidebarOpen(false);
          }}
          // TMAIL-324: real folder CRUD wired through TanStack mutations.
          onAddFolder={(name) => createFolderMutation.mutate(name)}
          onDeleteFolder={(name) => deleteFolderMutation.mutate(name)}
          isAddingFolderPending={createFolderMutation.isPending}
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
          {/* TMAIL-326: when the user has selected one or more rows, swap
              the folder header for the bulk-action toolbar. Same height
              (h-14) so the list below doesn't reflow. The toolbar renders
              Mark Read/Unread, Star/Unstar, Archive, Move-to, and
              Delete — the labels flip based on the aggregate state of the
              visible selected envelopes so the verb describes what will
              happen next (mirrors Gmail's behaviour). */}
          {selectedCount > 0 ? (
            <div
              className="h-14 border-b border-zinc-200 dark:border-zinc-800 flex items-center justify-between px-3 shrink-0 bg-blue-50 dark:bg-blue-950/40"
              role="toolbar"
              aria-label={`Bulk actions for ${selectedCount} selected email${selectedCount === 1 ? '' : 's'}`}
            >
              <div className="flex items-center gap-2 min-w-0">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={handleClearSelection}
                  aria-label="Clear selection"
                  title="Clear selection"
                >
                  <X className="size-4" />
                </Button>
                <Checkbox
                  checked={allVisibleSelected}
                  onClick={(e) => {
                    e.stopPropagation();
                    handleToggleSelectAll();
                  }}
                  aria-label={
                    allVisibleSelected
                      ? 'Deselect all visible emails'
                      : 'Select all visible emails'
                  }
                  data-indeterminate={someVisibleSelected && !allVisibleSelected ? 'true' : undefined}
                />
                <span className="text-sm font-medium truncate">
                  {selectedCount} selected
                </span>
              </div>
              <div className="flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={allSelectedRead ? handleBulkMarkUnread : handleBulkMarkRead}
                  disabled={bulkFlagMutation.isPending}
                  aria-label={allSelectedRead ? 'Mark selected as unread' : 'Mark selected as read'}
                  title={allSelectedRead ? 'Mark as unread' : 'Mark as read'}
                >
                  {allSelectedRead ? (
                    <Mail className="size-4" />
                  ) : (
                    <MailOpen className="size-4" />
                  )}
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={allSelectedStarred ? handleBulkUnstar : handleBulkStar}
                  disabled={bulkFlagMutation.isPending}
                  aria-label={allSelectedStarred ? 'Unstar selected' : 'Star selected'}
                  aria-pressed={allSelectedStarred}
                  title={allSelectedStarred ? 'Unstar' : 'Star'}
                >
                  <StarIcon
                    className={`size-4 ${
                      allSelectedStarred ? 'fill-yellow-400 text-yellow-400' : ''
                    }`}
                  />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={handleBulkArchive}
                  disabled={bulkMoveMutation.isPending || activeFolder === ARCHIVE_FOLDER}
                  aria-label="Archive selected"
                  title="Archive"
                >
                  <ArchiveIcon className="size-4" />
                </Button>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      disabled={bulkMoveMutation.isPending || moveTargetFolders.length === 0}
                      aria-label="Move selected to folder"
                      title="Move to…"
                    >
                      <FolderInput className="size-4" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    {moveTargetFolders.map((f) => (
                      <DropdownMenuItem
                        key={f.id}
                        onSelect={() => handleBulkMoveTo(f.id)}
                      >
                        {f.name}
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuContent>
                </DropdownMenu>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={handleBulkDelete}
                  disabled={bulkDeleteMutation.isPending}
                  aria-label={
                    isPermanentDelete
                      ? `Permanently delete ${selectedCount} selected emails`
                      : `Delete ${selectedCount} selected emails`
                  }
                  title={isPermanentDelete ? 'Permanently delete' : 'Delete'}
                  className="text-red-600 hover:text-red-700"
                >
                  <Trash2 className="size-4" />
                </Button>
              </div>
            </div>
          ) : (
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
          )}
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
                // Added (TMAIL-325): infinite-scroll plumbing. EmailList owns
                // the IntersectionObserver-driven sentinel and calls
                // onLoadMore when it scrolls into view; hasNextPage and
                // isFetchingNextPage come straight from the useInfiniteQuery
                // above so the list knows when to stop fetching and when to
                // render the "Loading more…" indicator.
                hasNextPage={messagesQuery.hasNextPage}
                isFetchingNextPage={messagesQuery.isFetchingNextPage}
                onLoadMore={() => messagesQuery.fetchNextPage()}
                // Added (TMAIL-326): multi-select. The list renders per-row
                // checkboxes only when onToggleSelect is wired (we always
                // wire it here, but EmailList keeps the prop optional so
                // standalone usages stay valid). handleToggleSelect routes
                // shift-clicks through rangeSelect() against visibleUids and
                // single clicks through toggleSelection().
                selectedIds={selectedIds}
                onToggleSelect={handleToggleSelect}
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
