// TMAIL-218: hydrate the full message body from /api/folders/{folder}/messages/{uid}
// when a row is selected. Fall back to the envelope summary while the body is
// loading. HTML body is sanitized with DOMPurify (same contract the production
// SPA uses) before insertion via the React-escape hatch — required because the
// IMAP source HTML is the actual rendering target.
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import DOMPurify from 'dompurify';
import {
  Reply,
  ReplyAll,
  Forward,
  Trash2,
  Archive,
  Star,
  Download,
  FileDown,
  ShieldAlert,
  ShieldCheck,
} from 'lucide-react';
import { format } from 'date-fns';
import { Button } from '@/components/ui/button';
import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import { downloadAttachment, fetchMessage } from '@/api/messages';
// Added (TMAIL-349): per-message EML export — single click downloads the raw
// RFC822 bytes of the open message as `message_<uid>.eml`. Shares the same
// blob → object URL → anchor click lifecycle as the attachment download.
import { exportEml, triggerBlobDownload } from '@/api/eml';
// Added (TMAIL-347): phishing scan / report client. Same backend contract the
// classic SPA's MessageView consumes — see `themes/shadcn-prototype/src/api/phishing.ts`.
import {
  getPhishingReport,
  parseFromHeader,
  scanMessage,
  updatePhishingAction,
  type PhishingAction,
  type PhishingReport,
} from '@/api/phishing';
import type { Email } from '@/types/ui';
import type { Attachment, FullMessage } from '@/types/mail';
import type { ReplyKind } from './replyContext';
// Added (TMAIL-348): per-message internal comments thread rendered below the
// body + attachments. Mailbox-scoped server-side so the component doesn't
// need any current-user identity prop — see `CommentsThread.tsx` for the
// ownership note.
import { CommentsThread } from './CommentsThread';

interface EmailReaderProps {
  folder: string;
  uid: number | null;
  listItem: Email | null;
  // Changed: TMAIL-319 — onCompose now carries both the active intent
  // (Reply / Reply All / Forward) AND the loaded FullMessage when one is
  // available. EmailClient turns that into a ReplyContext via
  // buildReplyContext() and hands it to ComposeModal as the prefill.
  //
  // The legacy zero-arg shape ("user clicked the floating Compose button")
  // is preserved by passing both args as null/undefined — the reader's
  // existing call sites in TopBar / sidebar continue to work without
  // change.
  onCompose: (kind: ReplyKind | null, message: FullMessage | null) => void;
  // Added: TMAIL-316 — star button in the reader header toggles the IMAP
  // \Flagged keyword via the same /flag endpoint the EmailList star uses.
  // EmailClient owns the mutation + cache invalidation; the reader stays
  // presentational and just reports user intent (mirrors EmailList — TMAIL-315).
  onToggleStar?: (uid: number, currentlyStarred: boolean) => void;
  // Added: TMAIL-317 — Archive button moves the message to the IMAP "Archive"
  // folder via the same /move endpoint MessageView uses in the classic SPA.
  // EmailClient owns the mutation, optimistic update, and cache invalidation;
  // the reader stays presentational.
  onArchive?: (uid: number) => void;
  // Added: TMAIL-318 — Delete button. From any non-trash folder the backend
  // /delete handler soft-deletes by moving to the per-user trash folder
  // (Stalwart "Deleted Items", Dovecot "Trash", Gmail "[Gmail]/Trash" — see
  // imap_service.rs::trash_folder()). From the trash folder itself it is a
  // permanent EXPUNGE — EmailClient gates that path behind window.confirm()
  // before invoking the mutation. The reader stays presentational and only
  // reports user intent; `isPermanentDelete` lets the aria-label spell out
  // the consequence to screen-reader users.
  onDelete?: (uid: number) => void;
  isPermanentDelete?: boolean;
}

export function EmailReader({
  folder,
  uid,
  listItem,
  onCompose,
  onToggleStar,
  onArchive,
  onDelete,
  isPermanentDelete,
}: EmailReaderProps) {
  const messageQuery = useQuery({
    queryKey: ['message', folder, uid],
    queryFn: () => fetchMessage(folder, uid!),
    enabled: uid != null,
  });

  // Added (TMAIL-347): phishing report query. Returns null when the message
  // has never been scanned — the UI uses that null to surface the manual
  // "Scan for phishing" button. staleTime keeps the cache warm for a minute
  // so flipping between adjacent messages in a thread doesn't refetch.
  const queryClient = useQueryClient();
  const phishingQuery = useQuery({
    queryKey: ['phishing', folder, uid],
    queryFn: () => getPhishingReport(folder, uid!),
    enabled: uid != null,
    staleTime: 60_000,
  });

  // Added (TMAIL-347): one-shot scan trigger. Driven by the manual "Scan for
  // phishing" button — we deliberately do NOT auto-scan on open (parity with
  // classic SPA; avoids surprise writes to the phishing_reports table for
  // every message the user opens). Successful scan invalidates the query so
  // the banner renders without a second click.
  const scanMutation = useMutation({
    mutationFn: () => {
      if (uid == null || !messageQuery.data) {
        return Promise.reject(new Error('Message not yet loaded'));
      }
      const m = messageQuery.data;
      const { display, email } = parseFromHeader(m.from);
      return scanMessage(folder, uid, {
        html_body: m.html_body || m.text_body || '',
        sender_display_name: display,
        sender_email: email,
        attachments: (m.attachments ?? []).map((a) => ({
          filename: a.filename,
          content_type: a.content_type,
        })),
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['phishing', folder, uid] });
    },
  });

  // Added (TMAIL-347): "Mark safe" (confirmed_safe) / "Report" (reported) /
  // "Dismiss" (dismissed) actions. All three hide the banner — the action is
  // persisted server-side so re-opening the message doesn't resurface it.
  const phishingActionMutation = useMutation({
    mutationFn: ({ reportId, action }: { reportId: string; action: PhishingAction }) =>
      updatePhishingAction(reportId, action),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['phishing', folder, uid] });
    },
  });

  // Added (TMAIL-320): per-row download state so the user can see which
  // attachment is fetching when multiple are present. We store the part_id of
  // the active fetch (or 'error:<part_id>' on failure) — keeps the state
  // shape tiny without pulling in a full mutation per row.
  const [downloadingPartId, setDownloadingPartId] = useState<string | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);

  // Added (TMAIL-349): per-message EML export state. We surface a tiny error
  // string under the toolbar on failure so the user sees something specific
  // (the alt-UI has no global toast layer yet — same pattern as
  // downloadError above).
  const [isExportingEml, setIsExportingEml] = useState(false);
  const [emlExportError, setEmlExportError] = useState<string | null>(null);

  const handleExportEml = async () => {
    if (uid == null) return;
    setIsExportingEml(true);
    setEmlExportError(null);
    try {
      const { blob, filename } = await exportEml(folder, uid);
      triggerBlobDownload(blob, filename);
    } catch (err) {
      setEmlExportError(
        err instanceof Error ? err.message : 'Failed to export message as EML',
      );
    } finally {
      setIsExportingEml(false);
    }
  };

  const handleDownloadAttachment = async (attachment: Attachment) => {
    if (uid == null) return;
    setDownloadingPartId(attachment.part_id);
    setDownloadError(null);
    try {
      const { blob, filename } = await downloadAttachment(
        folder,
        uid,
        attachment.part_id,
        attachment.filename || 'attachment',
      );
      // Trigger a real browser download via a temporary object URL. Wrapped
      // in try/finally so we always revoke the URL even if the synthetic
      // click throws (some headless drivers reject programmatic clicks).
      const objectUrl = URL.createObjectURL(blob);
      try {
        const a = document.createElement('a');
        a.href = objectUrl;
        a.download = filename;
        a.rel = 'noopener';
        // Some browsers (notably Firefox) ignore the click on a detached
        // anchor — attach to the DOM for the click then remove.
        document.body.appendChild(a);
        a.click();
        a.remove();
      } finally {
        URL.revokeObjectURL(objectUrl);
      }
    } catch (err) {
      setDownloadError(
        err instanceof Error ? err.message : 'Failed to download attachment',
      );
    } finally {
      setDownloadingPartId(null);
    }
  };

  if (uid == null) {
    return (
      <div className="flex items-center justify-center h-full text-zinc-500">
        Select an email to read
      </div>
    );
  }

  const m = messageQuery.data;
  const subject = m?.subject ?? listItem?.subject ?? '(loading…)';
  const from = m?.from ?? listItem?.from ?? '(unknown)';
  const fromEmail = m?.from ?? listItem?.fromEmail ?? '';
  const to = m?.to ?? '';
  const dateRaw = m?.date ?? listItem?.timestamp ?? new Date();
  const timestamp = typeof dateRaw === 'string' ? new Date(dateRaw) : dateRaw;
  const isStarred = m?.flags?.some((f: string) => f.includes('Flagged')) ?? listItem?.starred ?? false;
  const htmlBody = m?.html_body ?? '';
  const textBody = m?.text_body ?? '';
  const attachments = m?.attachments ?? [];

  const initials = from
    .split(' ')
    .map((n: string) => n[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);

  // Sanitize once so the JSX below stays readable.
  const safeHtml = htmlBody ? DOMPurify.sanitize(htmlBody) : '';

  return (
    <div className="flex flex-col h-full">
      <div className="border-b border-zinc-200 dark:border-zinc-800 p-4">
        <div className="flex items-start justify-between mb-4">
          <h2 className="text-2xl font-semibold flex-1">{subject}</h2>
          {/* Added: TMAIL-316 — wired to /flag via EmailClient's mutation.
              Native <button> + aria-pressed so screen readers announce the
              toggle state, matching the EmailList pattern (TMAIL-315). */}
          <button
            type="button"
            aria-pressed={isStarred}
            aria-label={isStarred ? `Unstar email from ${from}` : `Star email from ${from}`}
            disabled={uid == null || !onToggleStar}
            onClick={() => {
              if (uid != null) onToggleStar?.(uid, isStarred);
            }}
            className="inline-flex items-center justify-center size-9 rounded-md hover:bg-zinc-100 dark:hover:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Star
              className={`size-5 ${isStarred ? 'fill-yellow-400 text-yellow-400' : 'text-zinc-400 hover:text-yellow-400'}`}
            />
          </button>
        </div>

        <div className="flex items-center gap-3 mb-4">
          <Avatar>
            <AvatarFallback className="bg-gradient-to-br from-blue-500 to-purple-600 text-white">{initials}</AvatarFallback>
          </Avatar>
          <div className="flex-1 min-w-0">
            <div className="font-medium truncate">{from}</div>
            <div className="text-sm text-zinc-500 truncate">
              {fromEmail}
              {to ? ` → ${to}` : ''}
            </div>
          </div>
          <div className="text-xs sm:text-sm text-zinc-500 shrink-0 ml-2">
            <span className="hidden sm:inline">{format(timestamp, 'MMM d, yyyy • h:mm a')}</span>
            <span className="sm:hidden">{format(timestamp, 'MMM d')}</span>
          </div>
        </div>

        <div className="flex items-center gap-1 sm:gap-2 flex-wrap">
          {/* Added: TMAIL-319 — Reply / Reply All / Forward each open the
              composer with the loaded FullMessage so EmailClient can build a
              ReplyContext prefill (recipients, Re: / Fwd: subject, quoted
              body, In-Reply-To + References headers). Disabled while the
              /api/folders/{folder}/messages/{uid} body is loading so the
              prefill always reflects the real message — clicking before the
              body arrives would prefill a "(loading…)" subject and an empty
              quote, and there's no good way to retro-fix that once the modal
              is open. Each button carries an aria-label so screen readers
              announce both the action and the source sender. */}
          <Button
            variant="outline"
            size="sm"
            disabled={m == null}
            aria-label={`Reply to ${from}`}
            onClick={() => onCompose('reply', m ?? null)}
          >
            <Reply className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Reply</span>
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={m == null}
            aria-label={`Reply all to ${from}`}
            onClick={() => onCompose('replyAll', m ?? null)}
          >
            <ReplyAll className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Reply All</span>
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={m == null}
            aria-label={`Forward email from ${from}`}
            onClick={() => onCompose('forward', m ?? null)}
          >
            <Forward className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Forward</span>
          </Button>
          {/* Added (TMAIL-349): Export the open message as a raw .eml file via
              GET /api/folders/{folder}/messages/{uid}/eml. Disabled until the
              message body has loaded so the export always corresponds to the
              currently-rendered message. */}
          <Button
            variant="outline"
            size="sm"
            disabled={uid == null || m == null || isExportingEml}
            aria-label={`Export email from ${from} as EML file`}
            onClick={handleExportEml}
            data-testid="modern-export-eml"
          >
            <FileDown className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">
              {isExportingEml ? 'Exporting…' : 'Export EML'}
            </span>
          </Button>
          <div className="flex-1" />
          {/* Added: TMAIL-317 — wires to /move via EmailClient's archiveMutation
              targeting the IMAP "Archive" folder (auto-created on first use). */}
          <Button
            variant="outline"
            size="sm"
            disabled={uid == null || !onArchive}
            aria-label={`Archive email from ${from}`}
            onClick={() => {
              if (uid != null) onArchive?.(uid);
            }}
          >
            <Archive className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Archive</span>
          </Button>
          {/* Added: TMAIL-318 — wires to /delete via EmailClient's deleteMutation.
              Backend resolves the per-user trash folder name and either moves
              the message there (soft delete) or EXPUNGEs it permanently if the
              active folder IS that trash folder. EmailClient gates permanent
              delete behind window.confirm() before firing the mutation. */}
          <Button
            variant="outline"
            size="sm"
            disabled={uid == null || !onDelete}
            aria-label={
              isPermanentDelete
                ? `Permanently delete email from ${from}`
                : `Delete email from ${from}`
            }
            onClick={() => {
              if (uid != null) onDelete?.(uid);
            }}
          >
            <Trash2 className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Delete</span>
          </Button>
        </div>
        {/* Added (TMAIL-349): inline error for EML export failures. Placed
            outside the action row so a long server message wraps without
            breaking the toolbar layout. */}
        {emlExportError && (
          <div
            role="alert"
            data-testid="modern-export-eml-error"
            className="mt-2 text-sm text-red-600 dark:text-red-400"
          >
            {emlExportError}
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto p-6">
        {messageQuery.isLoading && (
          <div className="text-zinc-500 text-sm">Loading message…</div>
        )}
        {messageQuery.isError && (
          <div className="text-red-600 text-sm">
            Couldn't load message: {String(messageQuery.error)}
          </div>
        )}
        {!messageQuery.isLoading && !messageQuery.isError && (
          <>
            {/* Added (TMAIL-347): phishing detection banner. Shown when the
                backend has a report with risk_score > 0 AND the user hasn't
                already acted on it. Action handlers are owned by the reader
                so EmailClient doesn't need to know about phishing state. */}
            <PhishingBanner
              report={phishingQuery.data ?? null}
              isLoading={phishingQuery.isLoading}
              isScanning={scanMutation.isPending}
              scanError={scanMutation.error}
              canScan={messageQuery.data != null && uid != null}
              isActing={phishingActionMutation.isPending}
              onScan={() => scanMutation.mutate()}
              onAction={(reportId, action) =>
                phishingActionMutation.mutate({ reportId, action })
              }
            />
            {safeHtml ? (
              // eslint-disable-next-line react/no-danger -- DOMPurify-sanitized above
              <div className="prose dark:prose-invert max-w-none" dangerouslySetInnerHTML={{ __html: safeHtml }} />
            ) : (
              <pre className="whitespace-pre-wrap font-sans text-sm">{textBody || '(empty body)'}</pre>
            )}

            {attachments.length > 0 && (
              <div className="mt-8 space-y-2">
                <h3 className="text-sm font-medium text-zinc-700 dark:text-zinc-300 mb-3">
                  Attachments ({attachments.length})
                </h3>
                {downloadError && (
                  <div
                    role="alert"
                    className="text-sm text-red-600 dark:text-red-400 mb-2"
                  >
                    {downloadError}
                  </div>
                )}
                {attachments.map((attachment, index) => {
                  // Added (TMAIL-320): typed alias so the click handler gets
                  // the same Attachment shape the API returns — keeps the
                  // download call site short.
                  const att = attachment as Attachment;
                  const isDownloading = downloadingPartId === att.part_id;
                  const filename = att.filename || '(unnamed)';
                  return (
                    <div
                      key={att.part_id || index}
                      className="flex items-center justify-between p-3 border border-zinc-200 dark:border-zinc-800 rounded-lg bg-zinc-50 dark:bg-zinc-900"
                    >
                      <div className="flex items-center gap-3">
                        <div className="size-10 rounded bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center">
                          <Download className="size-5 text-blue-600 dark:text-blue-400" />
                        </div>
                        <div>
                          <div className="font-medium text-sm">{filename}</div>
                          <div className="text-xs text-zinc-500">
                            {att.size != null ? `${Math.round(att.size / 1024)} KB` : ''}
                          </div>
                        </div>
                      </div>
                      {/* Added (TMAIL-320): wires to GET /api/folders/{folder}
                          /messages/{uid}/parts/{part_id} and triggers a real
                          browser download via a blob URL. Disabled while a
                          fetch is in flight so a double-click doesn't queue
                          two downloads of the same part. */}
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={isDownloading || uid == null}
                        aria-label={`Download attachment ${filename}`}
                        onClick={() => handleDownloadAttachment(att)}
                      >
                        <Download className="size-4 mr-2" />
                        {isDownloading ? 'Downloading…' : 'Download'}
                      </Button>
                    </div>
                  );
                })}
              </div>
            )}

            {/* Added (TMAIL-348): per-message internal comments thread.
                Visual stack is body → attachments → comments so the message
                reads naturally before the org-internal notes section. The
                thread mounts only when we have a real uid (guaranteed inside
                this branch by the early uid==null return above). */}
            <CommentsThread folder={folder} uid={uid} />
          </>
        )}
      </div>
    </div>
  );
}

// Added (TMAIL-347) — phishing banner subcomponent.
//
// PURPOSE: render the phishing-detection state above the message body:
//   - report=null + canScan=true → "Scan for phishing" prompt (manual trigger)
//   - report exists + risk_score > 0 + user_action='none' → severity banner
//   - report exists + user_action != 'none' → nothing (user already acted)
//
// Kept separate from EmailReader so the body of the reader stays small and
// the banner can be visually-regression-tested in isolation later.
//
// Severity tiers match the classic SPA (TMAIL-124, MessageView.tsx:111-121):
//   risk_score >= 71 → high   (red,   "appears to be a phishing attempt")
//   risk_score >= 41 → medium (amber, "contains suspicious links")
//   risk_score >   0 → low    (blue,  "some links may be suspicious")
interface PhishingBannerProps {
  report: PhishingReport | null;
  isLoading: boolean;
  isScanning: boolean;
  scanError: Error | null;
  canScan: boolean;
  isActing: boolean;
  onScan: () => void;
  onAction: (reportId: string, action: PhishingAction) => void;
}

function PhishingBanner({
  report,
  isLoading,
  isScanning,
  scanError,
  canScan,
  isActing,
  onScan,
  onAction,
}: PhishingBannerProps) {
  // Wait until the GET resolves so we don't flash the "Scan" button under a
  // report that's actually already on the server.
  if (isLoading) return null;

  if (report && report.risk_score > 0 && report.user_action === 'none') {
    const tier = phishingTier(report.risk_score);
    return (
      <div
        role="alert"
        data-testid="modern-phishing-banner"
        data-severity={tier.severity}
        className={`mb-6 rounded-lg border px-4 py-3 ${tier.classes}`}
      >
        <div className="flex items-start gap-3">
          <ShieldAlert className="size-5 shrink-0 mt-0.5" aria-hidden="true" />
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <strong className="text-sm font-semibold">{tier.title}</strong>
              <span className="text-xs opacity-80">
                Risk score: {report.risk_score}/100
              </span>
            </div>
            {report.suspicious_links.length > 0 && (
              <ul className="mt-2 space-y-1 text-xs">
                {report.suspicious_links.slice(0, 5).map((link, idx) => (
                  <li key={idx} className="break-all">
                    <code className="bg-black/5 dark:bg-white/10 rounded px-1 py-0.5">
                      {link.url}
                    </code>
                    {link.reasons.length > 0 && (
                      <span className="opacity-80">
                        {' '}
                        — {link.reasons.join(', ')}
                      </span>
                    )}
                  </li>
                ))}
                {report.suspicious_links.length > 5 && (
                  <li className="opacity-70">
                    …and {report.suspicious_links.length - 5} more
                  </li>
                )}
              </ul>
            )}
            {(report.dangerous_attachments ?? []).length > 0 && (
              <ul className="mt-2 space-y-1 text-xs">
                {(report.dangerous_attachments ?? []).map((att, idx) => (
                  <li key={idx}>
                    <strong>{att.filename}</strong>{' '}
                    <span className="opacity-80">— {att.reason}</span>
                  </li>
                ))}
              </ul>
            )}
            <div className="mt-3 flex flex-wrap gap-2">
              <Button
                size="sm"
                variant="outline"
                disabled={isActing}
                data-testid="modern-phishing-mark-safe"
                aria-label="Mark this email as safe"
                onClick={() => onAction(report.id, 'confirmed_safe')}
              >
                <ShieldCheck className="size-4 mr-2" aria-hidden="true" />
                Mark safe
              </Button>
              <Button
                size="sm"
                variant="destructive"
                disabled={isActing}
                data-testid="modern-phishing-report"
                aria-label="Report this email as phishing"
                onClick={() => onAction(report.id, 'reported')}
              >
                <ShieldAlert className="size-4 mr-2" aria-hidden="true" />
                Report phishing
              </Button>
              <Button
                size="sm"
                variant="ghost"
                disabled={isActing}
                data-testid="modern-phishing-dismiss"
                aria-label="Dismiss this phishing warning"
                onClick={() => onAction(report.id, 'dismissed')}
              >
                Dismiss
              </Button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // No report yet — surface the manual scan trigger.
  if (!report && canScan) {
    return (
      <div className="mb-6 flex items-center gap-3 text-xs text-zinc-500 dark:text-zinc-400">
        <Button
          size="sm"
          variant="outline"
          disabled={isScanning}
          data-testid="modern-phishing-scan"
          aria-label="Scan this email for phishing"
          onClick={onScan}
        >
          <ShieldAlert className="size-4 mr-2" aria-hidden="true" />
          {isScanning ? 'Scanning…' : 'Scan for phishing'}
        </Button>
        {scanError && (
          <span role="status" className="text-red-600 dark:text-red-400">
            Scan failed: {scanError.message}
          </span>
        )}
      </div>
    );
  }

  return null;
}

// Pure helper — severity tier metadata for a given risk score. Exported as a
// module-local function (not via the public API) so the banner stays
// self-contained but the tiers can still be unit-tested cheaply.
function phishingTier(riskScore: number): {
  severity: 'low' | 'medium' | 'high';
  title: string;
  classes: string;
} {
  if (riskScore >= 71) {
    return {
      severity: 'high',
      title: 'Warning: this email appears to be a phishing attempt',
      classes:
        'border-red-300 bg-red-50 text-red-900 dark:border-red-700 dark:bg-red-950/40 dark:text-red-100',
    };
  }
  if (riskScore >= 41) {
    return {
      severity: 'medium',
      title: 'This email contains suspicious links',
      classes:
        'border-amber-300 bg-amber-50 text-amber-900 dark:border-amber-700 dark:bg-amber-950/40 dark:text-amber-100',
    };
  }
  return {
    severity: 'low',
    title: 'Some links in this email may be suspicious',
    classes:
      'border-sky-300 bg-sky-50 text-sky-900 dark:border-sky-700 dark:bg-sky-950/40 dark:text-sky-100',
  };
}
