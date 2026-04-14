import { ArrowLeft, Reply, Forward, Trash2, FolderInput, Star, Download, ShieldAlert } from 'lucide-react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCurrentMessage } from '../../hooks/useMailbox';
import { useMailStore } from '../../stores/mailStore';
import { formatFullDate } from '../../utils/date';
import { sanitizeHtml } from '../../utils/sanitize';
import { deleteMessage, moveMessage, flagMessage } from '../../api/messages';
// Added: EML export for TMAIL-68
import { exportEml, downloadEml } from '../../api/eml';
// Added: Phishing scan API for TMAIL-124
import { getPhishingReport, scanMessage, updatePhishingAction } from '../../api/phishing';
import type { PhishingReport } from '../../api/phishing';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';
// Added: Internal comments component for TMAIL-128
import { CommentThread } from './CommentThread';
// Added: Smart reply suggestion bar for TMAIL-104
import { SmartReplyBar } from './SmartReplyBar';
// Added: Email summarization component for TMAIL-103
import { EmailSummary } from './EmailSummary';

export function MessageView() {
  const queryClient = useQueryClient();
  const { data: message, isLoading, error } = useCurrentMessage();
  const selectedFolder = useMailStore((s) => s.selectedFolder);
  const setSelectedUid = useMailStore((s) => s.setSelectedUid);
  const setViewMode = useMailStore((s) => s.setViewMode);

  // Added: Delete mutation
  const deleteMut = useMutation({
    mutationFn: () => deleteMessage(selectedFolder, message!.uid),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['messages'] });
      queryClient.invalidateQueries({ queryKey: ['folders'] });
      setSelectedUid(null);
    },
  });

  // Added: Move mutation
  const moveMut = useMutation({
    mutationFn: (toFolder: string) => moveMessage(selectedFolder, message!.uid, toFolder),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['messages'] });
      queryClient.invalidateQueries({ queryKey: ['folders'] });
      setSelectedUid(null);
    },
  });

  // Added: Flag mutation for starring
  const flagMut = useMutation({
    mutationFn: ({ flag, add }: { flag: string; add: boolean }) =>
      flagMessage(selectedFolder, message!.uid, flag, add),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['message'] });
    },
  });

  // Added: EML export mutation — downloads the raw email as a .eml file (TMAIL-68)
  const exportEmlMut = useMutation({
    mutationFn: () => exportEml(selectedFolder, message!.uid),
    onSuccess: (blob) => {
      downloadEml(blob, message!.uid);
    },
  });

  // Added: Fetch existing phishing report for the current message (TMAIL-124)
  const phishingQuery = useQuery({
    queryKey: ['phishing', selectedFolder, message?.uid],
    queryFn: () => getPhishingReport(selectedFolder, message!.uid),
    enabled: !!message,
    staleTime: 60_000,
  });

  // Added: Trigger phishing scan on message load if no existing report (TMAIL-124)
  const scanMut = useMutation({
    mutationFn: () => scanMessage(selectedFolder, message!.uid, {
      html_body: message!.html_body || message!.text_body || '',
      sender_display_name: (message!.from ?? '').split('<')[0].trim(),
      sender_email: (message!.from ?? '').match(/<(.+)>/)?.[1] || message!.from || '',
    }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['phishing', selectedFolder, message!.uid] });
    },
  });

  // Added: Dismiss or report phishing actions (TMAIL-124)
  const phishingActionMut = useMutation({
    mutationFn: ({ reportId, action }: { reportId: string; action: 'dismissed' | 'reported' | 'confirmed_safe' }) =>
      updatePhishingAction(reportId, { action }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['phishing', selectedFolder, message?.uid] });
    },
  });

  if (isLoading) return <LoadingSkeleton rows={8} />;
  if (error) return <div className="message-view__error">Failed to load message</div>;
  if (!message) return null;

  const handleReply = () => {
    setViewMode('compose');
  };

  const isFlagged = message.flags.some((f) => f.includes('Flagged'));

  // Added: Determine phishing report state for banner rendering (TMAIL-124)
  const phishingReport: PhishingReport | null | undefined = phishingQuery.data;
  const showPhishingBanner = phishingReport && phishingReport.risk_score > 0 && phishingReport.user_action === 'none';

  // Added: Risk level classification for banner styling (TMAIL-124)
  const getPhishingBannerClass = (riskScore: number): string => {
    if (riskScore >= 71) return 'phishing-banner phishing-banner--high';
    if (riskScore >= 41) return 'phishing-banner phishing-banner--medium';
    return 'phishing-banner phishing-banner--low';
  };

  const getPhishingBannerMessage = (riskScore: number): string => {
    if (riskScore >= 71) return 'Warning: This email appears to be a phishing attempt';
    if (riskScore >= 41) return 'This email contains suspicious links';
    return 'Some links in this email may be suspicious';
  };

  // Sanitize HTML body using DOMPurify to prevent XSS before rendering
  const sanitizedBody = message.html_body ? sanitizeHtml(message.html_body) : null;

  return (
    <div className="message-view">
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setSelectedUid(null)} title="Back to list">
          <ArrowLeft size={20} />
        </button>
        <button className="btn btn--icon" onClick={handleReply} title="Reply">
          <Reply size={20} />
        </button>
        <button className="btn btn--icon" title="Forward">
          <Forward size={20} />
        </button>
        <button
          className={`btn btn--icon ${isFlagged ? 'btn--active' : ''}`}
          onClick={() => flagMut.mutate({ flag: '\\Flagged', add: !isFlagged })}
          title={isFlagged ? 'Unstar' : 'Star'}
        >
          <Star size={20} />
        </button>
        {/* Added: Download .eml button for TMAIL-68 */}
        <button
          className="btn btn--icon"
          onClick={() => exportEmlMut.mutate()}
          disabled={exportEmlMut.isPending}
          title="Download .eml"
        >
          <Download size={20} />
        </button>
        <button
          className="btn btn--icon"
          onClick={() => {
            const folder = prompt('Move to folder:');
            if (folder) moveMut.mutate(folder);
          }}
          title="Move to folder"
        >
          <FolderInput size={20} />
        </button>
        <button
          className="btn btn--icon btn--danger"
          onClick={() => deleteMut.mutate()}
          title="Delete"
        >
          <Trash2 size={20} />
        </button>
      </div>

      <div className="message-view__header">
        <h2 className="message-view__subject">{message.subject || '(no subject)'}</h2>
        <div className="message-view__meta">
          <div className="message-view__from">
            <strong>From:</strong> {message.from}
          </div>
          <div className="message-view__to">
            <strong>To:</strong> {message.to.join(', ')}
          </div>
          {message.cc.length > 0 && (
            <div className="message-view__cc">
              <strong>Cc:</strong> {message.cc.join(', ')}
            </div>
          )}
          <div className="message-view__date">
            {formatFullDate(message.date)}
          </div>
        </div>
      </div>

      {/* Added: Phishing warning banner for TMAIL-124 */}
      {showPhishingBanner && (
        <div className={getPhishingBannerClass(phishingReport.risk_score)} data-testid="phishing-banner">
          <div className="phishing-banner__header">
            <ShieldAlert size={20} />
            <strong>{getPhishingBannerMessage(phishingReport.risk_score)}</strong>
            <span className="phishing-banner__score">Risk score: {phishingReport.risk_score}/100</span>
          </div>
          {phishingReport.suspicious_links.length > 0 && (
            <ul className="phishing-banner__links">
              {phishingReport.suspicious_links.map((link, index) => (
                <li key={index}>
                  <code>{link.url}</code>
                  {link.reasons.map((reason, reasonIndex) => (
                    <span key={reasonIndex} className="phishing-banner__reason">{reason}</span>
                  ))}
                </li>
              ))}
            </ul>
          )}
          <div className="phishing-banner__actions">
            <button
              className="btn btn--sm"
              onClick={() => phishingActionMut.mutate({ reportId: phishingReport.id, action: 'dismissed' })}
              disabled={phishingActionMut.isPending}
            >
              Dismiss
            </button>
            <button
              className="btn btn--sm btn--danger"
              onClick={() => phishingActionMut.mutate({ reportId: phishingReport.id, action: 'reported' })}
              disabled={phishingActionMut.isPending}
            >
              Report Phishing
            </button>
          </div>
        </div>
      )}

      {/* Added: Scan button when no phishing report exists yet (TMAIL-124) */}
      {!phishingReport && !phishingQuery.isLoading && (
        <button
          className="btn btn--sm btn--outline"
          onClick={() => scanMut.mutate()}
          disabled={scanMut.isPending}
          data-testid="scan-phishing-btn"
        >
          <ShieldAlert size={16} />
          {scanMut.isPending ? 'Scanning...' : 'Scan for phishing'}
        </button>
      )}

      {/* Added: AI email summarization above the message body (TMAIL-103) */}
      <EmailSummary
        folder={selectedFolder}
        uid={message.uid}
        emailText={message.text_body || message.html_body || ''}
      />

      {message.attachments.length > 0 && (
        <div className="message-view__attachments">
          <strong>Attachments:</strong>
          {message.attachments.map((att) => (
            <span key={att.part_id} className="attachment-chip">
              {att.filename} ({Math.round(att.size / 1024)}KB)
            </span>
          ))}
        </div>
      )}

      <div className="message-view__body">
        {sanitizedBody ? (
          <div
            className="message-view__html"
            dangerouslySetInnerHTML={{ __html: sanitizedBody }}
          />
        ) : (
          <pre className="message-view__text">{message.text_body}</pre>
        )}
      </div>

      {/* Added: Smart reply suggestion bar for TMAIL-104 */}
      <SmartReplyBar
        folder={selectedFolder}
        uid={message.uid}
        onUseReply={(replyText) => {
          // NOTE: Switches to compose mode — the reply text could be passed via store if needed
          void replyText;
          setViewMode('compose');
        }}
      />

      {/* Added: Internal comments thread for TMAIL-128 */}
      <CommentThread folder={selectedFolder} uid={message.uid} />
    </div>
  );
}
