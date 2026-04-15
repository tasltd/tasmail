// Added: Attachment storage management UI for TMAIL-59
import React, { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Upload, Trash2, Download, HardDrive } from 'lucide-react';
import { attachmentsApi } from '../../api/attachments';
import type { Attachment } from '../../api/attachments';
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
 * PURPOSE: Map scan_status to a display color for the badge
 */
function scanStatusColor(status: Attachment['scan_status']): string {
  switch (status) {
    case 'clean':
      return 'var(--color-success, #28a745)';
    case 'infected':
      return 'var(--color-danger, #dc3545)';
    case 'error':
      return 'var(--color-warning, #ffc107)';
    default:
      return 'var(--color-text-secondary)';
  }
}

/**
 * PURPOSE: Manage attachments — list, upload, download, delete with storage stats
 * EXTERNAL: Uses /api/attachments endpoints via TanStack Query
 */
export function AttachmentManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);

  // Added: File input state for upload form
  const [selectedFile, setSelectedFile] = useState<File | null>(null);

  const { data: attachments = [], isLoading: loadingAttachments } = useQuery({
    queryKey: ['attachments'],
    queryFn: attachmentsApi.list,
  });

  const { data: storageStats } = useQuery({
    queryKey: ['attachment-stats'],
    queryFn: attachmentsApi.stats,
  });

  const uploadMutation = useMutation({
    mutationFn: attachmentsApi.upload,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attachments'] });
      queryClient.invalidateQueries({ queryKey: ['attachment-stats'] });
      setSelectedFile(null);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: attachmentsApi.delete,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['attachments'] });
      queryClient.invalidateQueries({ queryKey: ['attachment-stats'] });
    },
  });

  // Added: Handle upload form submission
  const handleUpload = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!selectedFile) return;
    uploadMutation.mutate(selectedFile);
  };

  // Added: Trigger browser download for an attachment
  const handleDownload = async (attachment: Attachment) => {
    const blob = await attachmentsApi.download(attachment.id);
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = attachment.filename;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  if (loadingAttachments) return <LoadingSkeleton rows={4} />;

  return (
    <div className="attachment-manager" style={{ padding: '16px', maxWidth: '800px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Attachment Storage</h2>
      </div>

      {/* Added: Storage stats summary */}
      {storageStats && (
        <div
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
            background: 'var(--color-bg-secondary)',
            display: 'flex',
            gap: '24px',
            alignItems: 'center',
          }}
          data-testid="storage-stats"
        >
          <HardDrive size={24} />
          <div>
            <strong>{formatFileSize(storageStats.total_size_bytes)}</strong>
            <span style={{ color: 'var(--color-text-secondary)', marginLeft: '4px' }}>
              used
            </span>
          </div>
          <div>
            <strong>{storageStats.total_count}</strong>
            <span style={{ color: 'var(--color-text-secondary)', marginLeft: '4px' }}>
              file{storageStats.total_count !== 1 ? 's' : ''}
            </span>
          </div>
          {storageStats.pending_scans > 0 && (
            <div>
              <strong>{storageStats.pending_scans}</strong>
              <span style={{ color: 'var(--color-text-secondary)', marginLeft: '4px' }}>
                pending scans
              </span>
            </div>
          )}
          {storageStats.infected_count > 0 && (
            <div style={{ color: 'var(--color-danger, #dc3545)' }}>
              <strong>{storageStats.infected_count}</strong>
              <span style={{ marginLeft: '4px' }}>infected</span>
            </div>
          )}
        </div>
      )}

      {/* Added: Upload form */}
      <form
        onSubmit={handleUpload}
        style={{
          marginTop: '16px',
          padding: '16px',
          border: '1px solid var(--color-border)',
          borderRadius: '8px',
        }}
      >
        <h3 style={{ marginBottom: '12px' }}>Upload Attachment</h3>
        <div className="composer__field">
          <label>File</label>
          <input
            type="file"
            onChange={(e) => setSelectedFile(e.target.files?.[0] ?? null)}
            data-testid="file-input"
          />
        </div>
        <div className="composer__actions" style={{ marginTop: '12px' }}>
          <button
            type="submit"
            className="btn btn--primary"
            disabled={!selectedFile || uploadMutation.isPending}
          >
            <Upload size={16} />
            {uploadMutation.isPending ? 'Uploading...' : 'Upload'}
          </button>
        </div>
      </form>

      {/* Added: Attachment list with scan status and actions */}
      <div style={{ marginTop: '16px' }}>
        {attachments.length === 0 && (
          <p
            style={{
              color: 'var(--color-text-secondary)',
              textAlign: 'center',
              padding: '24px',
            }}
          >
            No attachments yet. Upload a file to get started.
          </p>
        )}
        {attachments.map((attachment: Attachment) => (
          <div
            key={attachment.id}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
          >
            <div style={{ flex: 1 }}>
              <strong>{attachment.filename}</strong>
              {/* Added: Scan status badge */}
              <span
                style={{
                  marginLeft: '8px',
                  fontSize: '11px',
                  background: scanStatusColor(attachment.scan_status),
                  color: 'white',
                  padding: '1px 6px',
                  borderRadius: '10px',
                }}
              >
                {attachment.scan_status}
              </span>
              <div
                style={{
                  fontSize: '12px',
                  color: 'var(--color-text-secondary)',
                  marginTop: '4px',
                }}
              >
                {formatFileSize(attachment.size_bytes)} · {attachment.content_type}
                {' · '}
                {new Date(attachment.created_at).toLocaleDateString()}
              </div>
            </div>
            <button
              className="btn btn--icon"
              onClick={() => handleDownload(attachment)}
              title="Download"
              data-testid={`download-${attachment.id}`}
            >
              <Download size={16} />
            </button>
            <button
              className="btn btn--icon btn--danger"
              onClick={() => deleteMutation.mutate(attachment.id)}
              title="Delete"
              data-testid={`delete-${attachment.id}`}
            >
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
