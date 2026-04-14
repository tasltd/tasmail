// Added: Shared file management UI for large file sharing via download links (TMAIL-138)
import { useState } from 'react';
import type { FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, FileUp, Trash2, Copy, Link } from 'lucide-react';
import {
  listSharedFiles,
  uploadSharedFile,
  deleteSharedFile,
  getDownloadUrl,
} from '../../api/shared-files';
import type { SharedFile } from '../../api/shared-files';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

/**
 * PURPOSE: Format file size in human-readable units (KB, MB, GB)
 * CONSTRAINTS: Input is in bytes; returns string with 1 decimal place
 */
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/**
 * PURPOSE: Determine if a shared file's link has expired
 * NOTE: Checks both time-based and download-count-based expiry
 */
function isExpired(file: SharedFile): boolean {
  if (file.expires_at && new Date(file.expires_at) < new Date()) {
    return true;
  }
  if (file.max_downloads !== null && file.download_count >= file.max_downloads) {
    return true;
  }
  return false;
}

export function SharedFileManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);

  // Added: Upload form state
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [maxDownloads, setMaxDownloads] = useState('');
  const [expiresAt, setExpiresAt] = useState('');
  const [password, setPassword] = useState('');
  const [copyFeedback, setCopyFeedback] = useState<string | null>(null);

  const { data: sharedFiles, isLoading } = useQuery({
    queryKey: ['shared-files'],
    queryFn: listSharedFiles,
  });

  // Added: Upload mutation with FormData construction
  const uploadMutation = useMutation({
    mutationFn: (formData: FormData) => uploadSharedFile(formData),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['shared-files'] });
      // NOTE: Reset form after successful upload
      setSelectedFile(null);
      setMaxDownloads('');
      setExpiresAt('');
      setPassword('');
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteSharedFile,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['shared-files'] }),
  });

  // Added: Build FormData from form state and submit
  const handleUpload = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!selectedFile) return;

    const formData = new FormData();
    formData.append('file', selectedFile);
    if (maxDownloads) formData.append('max_downloads', maxDownloads);
    if (expiresAt) formData.append('expires_at', new Date(expiresAt).toISOString());
    if (password) formData.append('password', password);

    uploadMutation.mutate(formData);
  };

  // Added: Copy download URL to clipboard with visual feedback
  const handleCopyLink = async (downloadToken: string) => {
    const downloadUrl = getDownloadUrl(downloadToken);
    await navigator.clipboard.writeText(downloadUrl);
    setCopyFeedback(downloadToken);
    setTimeout(() => setCopyFeedback(null), 2000);
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="shared-file-manager" style={{ padding: '16px', maxWidth: '800px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Shared Files</h2>
      </div>

      {/* Added: Upload form with optional expiry, download limit, and password */}
      <form
        onSubmit={handleUpload}
        style={{
          marginTop: '16px',
          padding: '16px',
          border: '1px solid var(--color-border)',
          borderRadius: '8px',
        }}
      >
        <h3 style={{ marginBottom: '12px' }}>Upload File</h3>
        <div className="composer__field">
          <label>File</label>
          <input
            type="file"
            onChange={(e) => setSelectedFile(e.target.files?.[0] ?? null)}
            data-testid="file-input"
          />
        </div>
        <div className="composer__field">
          <label>Expires</label>
          <input
            type="datetime-local"
            value={expiresAt}
            onChange={(e) => setExpiresAt(e.target.value)}
            data-testid="expiry-input"
          />
        </div>
        <div className="composer__field">
          <label>Max Downloads</label>
          <input
            type="number"
            min="1"
            placeholder="Unlimited"
            value={maxDownloads}
            onChange={(e) => setMaxDownloads(e.target.value)}
            data-testid="max-downloads-input"
          />
        </div>
        <div className="composer__field">
          <label>Password (optional)</label>
          <input
            type="password"
            placeholder="Leave empty for no password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </div>
        <div className="composer__actions" style={{ marginTop: '12px' }}>
          <button
            type="submit"
            className="btn btn--primary"
            disabled={!selectedFile || uploadMutation.isPending}
          >
            <FileUp size={16} />
            {uploadMutation.isPending ? 'Uploading...' : 'Upload & Share'}
          </button>
        </div>
      </form>

      {/* Added: File list with status badges and actions */}
      <div style={{ marginTop: '16px' }}>
        {(!sharedFiles || sharedFiles.length === 0) && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No shared files yet. Upload a file to generate a shareable link.
          </p>
        )}
        {sharedFiles?.map((file: SharedFile) => {
          const expired = isExpired(file);
          return (
            <div
              key={file.id}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '12px',
                padding: '12px',
                borderBottom: '1px solid var(--color-border)',
                opacity: expired ? 0.6 : 1,
              }}
            >
              <div style={{ flex: 1 }}>
                <strong>{file.filename}</strong>
                {/* Added: Status badge for expired/active state */}
                <span
                  style={{
                    marginLeft: '8px',
                    fontSize: '11px',
                    background: expired ? 'var(--color-danger, #dc3545)' : 'var(--color-success, #28a745)',
                    color: 'white',
                    padding: '1px 6px',
                    borderRadius: '10px',
                  }}
                >
                  {expired ? 'Expired' : 'Active'}
                </span>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '4px' }}>
                  {formatFileSize(file.file_size)} · {file.download_count} downloads
                  {file.max_downloads !== null && ` / ${file.max_downloads} max`}
                  {file.expires_at && ` · Expires ${new Date(file.expires_at).toLocaleDateString()}`}
                </div>
              </div>
              {/* Added: Copy link button with feedback */}
              <button
                className="btn btn--icon"
                onClick={() => handleCopyLink(file.download_token)}
                title="Copy download link"
                data-testid={`copy-link-${file.id}`}
              >
                {copyFeedback === file.download_token ? <Link size={16} /> : <Copy size={16} />}
              </button>
              <button
                className="btn btn--icon btn--danger"
                onClick={() => deleteMutation.mutate(file.id)}
                title="Delete"
                data-testid={`delete-${file.id}`}
              >
                <Trash2 size={16} />
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
