// Added: TMAIL-138 — Large file auto-upload widget for the Composer.
// When the user picks a file larger than LARGE_FILE_THRESHOLD_BYTES, it is
// uploaded to the shared-files API and a download link is inserted into the
// message body in place of attaching the bytes inline.
import { useState, useCallback } from 'react';
import { Paperclip } from 'lucide-react';
import {
  uploadSharedFileWithProgress,
  type SharedFile,
} from '../../api/shared-files';
import {
  LARGE_FILE_THRESHOLD_BYTES,
  MAX_SHARED_FILE_BYTES,
  SHARED_LINK_EXPIRY_OPTIONS,
  DEFAULT_SHARED_LINK_EXPIRY_DAYS,
} from '../../utils/constants';
import {
  formatBytes,
  isLargeFile,
  buildSharedFileLinkHtml,
  computeExpiresAt,
} from './large-file-attacher-utils';

interface LargeFileAttacherProps {
  onLinkReady: (html: string, file: SharedFile) => void;
  onError?: (message: string) => void;
}

export function LargeFileAttacher({ onLinkReady, onError }: LargeFileAttacherProps) {
  const [picked, setPicked] = useState<File | null>(null);
  const [expiryDays, setExpiryDays] = useState<number | null>(DEFAULT_SHARED_LINK_EXPIRY_DAYS);
  const [password, setPassword] = useState('');
  const [progressPercent, setProgressPercent] = useState<number | null>(null);
  const [uploading, setUploading] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  const reset = useCallback(() => {
    setPicked(null);
    setExpiryDays(DEFAULT_SHARED_LINK_EXPIRY_DAYS);
    setPassword('');
    setProgressPercent(null);
    setUploading(false);
  }, []);

  const reportError = useCallback(
    (msg: string) => {
      setLocalError(msg);
      if (onError) onError(msg);
    },
    [onError],
  );

  const handlePicked = (file: File | null) => {
    setLocalError(null);
    if (!file) {
      setPicked(null);
      return;
    }
    if (file.size > MAX_SHARED_FILE_BYTES) {
      reportError(
        `File is ${formatBytes(file.size)} — max allowed is ${formatBytes(MAX_SHARED_FILE_BYTES)}.`,
      );
      setPicked(null);
      return;
    }
    setPicked(file);
  };

  const handleUpload = async () => {
    if (!picked) return;
    setUploading(true);
    setLocalError(null);
    setProgressPercent(0);

    try {
      const formData = new FormData();
      formData.append('file', picked);
      const expiresAt = computeExpiresAt(expiryDays);
      if (expiresAt) formData.append('expires_at', expiresAt);
      if (password) formData.append('password', password);

      const result = await uploadSharedFileWithProgress(formData, ({ loaded, total }) => {
        setProgressPercent(Math.round((loaded / total) * 100));
      });

      onLinkReady(buildSharedFileLinkHtml(result), result);
      reset();
    } catch (err) {
      reportError(err instanceof Error ? err.message : 'Upload failed');
      setUploading(false);
      setProgressPercent(null);
    }
  };

  const tooSmall = picked !== null && !isLargeFile(picked.size);

  return (
    <div
      className="large-file-attacher"
      style={{
        padding: '12px',
        border: '1px solid var(--color-border)',
        borderRadius: '6px',
        marginTop: '8px',
      }}
      data-testid="large-file-attacher"
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
        <Paperclip size={16} />
        <strong style={{ fontSize: '13px' }}>Attach large file (auto-uploads to cloud)</strong>
      </div>
      <input
        type="file"
        onChange={(e) => handlePicked(e.target.files?.[0] ?? null)}
        disabled={uploading}
        data-testid="large-file-input"
      />
      {picked && (
        <div style={{ fontSize: '12px', marginTop: '6px', color: 'var(--color-text-secondary)' }}>
          {picked.name} — {formatBytes(picked.size)}
          {tooSmall && (
            <span style={{ color: 'var(--color-warning, #b58900)', marginLeft: '6px' }}>
              (under {formatBytes(LARGE_FILE_THRESHOLD_BYTES)} — will still upload to cloud)
            </span>
          )}
        </div>
      )}
      <div style={{ display: 'flex', gap: '12px', marginTop: '8px', flexWrap: 'wrap' }}>
        <label style={{ fontSize: '12px' }}>
          Expires:&nbsp;
          <select
            value={expiryDays === null ? 'never' : String(expiryDays)}
            onChange={(e) =>
              setExpiryDays(e.target.value === 'never' ? null : Number(e.target.value))
            }
            disabled={uploading}
            data-testid="expiry-select"
          >
            {SHARED_LINK_EXPIRY_OPTIONS.map((opt) => (
              <option key={opt.label} value={opt.days === null ? 'never' : String(opt.days)}>
                {opt.label}
              </option>
            ))}
          </select>
        </label>
        <label style={{ fontSize: '12px' }}>
          Password:&nbsp;
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="optional"
            disabled={uploading}
            data-testid="password-input"
            style={{ fontSize: '12px', padding: '2px 6px' }}
          />
        </label>
      </div>
      {progressPercent !== null && (
        <div
          style={{ marginTop: '8px' }}
          data-testid="upload-progress"
          aria-label="upload-progress"
        >
          <div
            style={{
              height: '6px',
              background: 'var(--color-border)',
              borderRadius: '3px',
              overflow: 'hidden',
            }}
          >
            <div
              style={{
                width: `${progressPercent}%`,
                height: '100%',
                background: 'var(--color-primary, #4a90d9)',
                transition: 'width 0.2s',
              }}
            />
          </div>
          <div style={{ fontSize: '11px', marginTop: '2px' }}>Uploading… {progressPercent}%</div>
        </div>
      )}
      {localError && (
        <div
          style={{ fontSize: '12px', color: 'var(--color-danger, #dc3545)', marginTop: '6px' }}
          data-testid="large-file-error"
        >
          {localError}
        </div>
      )}
      <div style={{ marginTop: '8px' }}>
        <button
          type="button"
          className="btn btn--primary btn--sm"
          onClick={handleUpload}
          disabled={!picked || uploading}
          data-testid="upload-and-insert-button"
        >
          {uploading ? 'Uploading…' : 'Upload & insert link'}
        </button>
      </div>
    </div>
  );
}
