// Added: TMAIL-138 — Pure helpers for LargeFileAttacher. Split out of the
// component file so react-refresh stays happy (component files may only
// export components).
import { getDownloadUrl, type SharedFile } from '../../api/shared-files';
import { LARGE_FILE_THRESHOLD_BYTES } from '../../utils/constants';

/// PURPOSE: Format file size in human-readable units (B/KB/MB/GB).
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/// PURPOSE: Decide whether a file is "large" and should be cloud-uploaded
/// instead of inline-attached. Threshold is configurable per plan.
export function isLargeFile(sizeBytes: number, thresholdBytes = LARGE_FILE_THRESHOLD_BYTES): boolean {
  return sizeBytes > thresholdBytes;
}

/// PURPOSE: Build the HTML snippet inserted into the editor body when an
/// attachment is replaced with a shared-files download link.
/// CONSTRAINTS: Output is plain HTML; no script tags; opens in new tab.
export function buildSharedFileLinkHtml(file: SharedFile): string {
  const url = getDownloadUrl(file.download_token);
  const sizeLabel = formatBytes(file.file_size);
  const expiryNote = file.expires_at
    ? ` (expires ${new Date(file.expires_at).toLocaleDateString()})`
    : '';
  // NOTE: Wrap in a <p> so TipTap accepts it as a paragraph-level insertion.
  return (
    `<p>📎 Large attachment: ` +
    `<a href="${url}" target="_blank" rel="noopener">${file.filename}</a> ` +
    `(${sizeLabel})${expiryNote}</p>`
  );
}

/// PURPOSE: Compute an ISO 8601 expiry timestamp from a relative "days" option.
/// Returns null when the user picks the "Never" option.
export function computeExpiresAt(days: number | null, now: Date = new Date()): string | null {
  if (days === null) return null;
  const expiry = new Date(now.getTime() + days * 24 * 60 * 60 * 1000);
  return expiry.toISOString();
}
