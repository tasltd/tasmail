// Added: PST import manager component for TMAIL-115 (Outlook PST file import)
import { useState, useRef, useCallback } from 'react';
import type { FormEvent, DragEvent, ChangeEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Upload, X, FileArchive, CheckCircle, AlertCircle, Clock, Loader } from 'lucide-react';
import { pstImportApi } from '../../api/pst-import';
import type { PstImport } from '../../types/pst-import';

/**
 * PURPOSE: UI component for uploading Outlook .pst files and tracking import progress
 * CONSTRAINTS: Accepts only .pst files; max file size enforced by backend
 * EXTERNAL: Calls /api/migration/pst/* endpoints via pstImportApi
 */
export function PstImportManager() {
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [targetFolder, setTargetFolder] = useState('INBOX');
  const [isDragging, setIsDragging] = useState(false);

  // Added: Fetch PST import history with polling for progress updates
  const { data: imports = [], isLoading } = useQuery({
    queryKey: ['pst-imports'],
    queryFn: pstImportApi.list,
    refetchInterval: 5000,
  });

  // Added: Upload mutation with optimistic invalidation
  const uploadMutation = useMutation({
    mutationFn: ({ file, folder }: { file: File; folder: string }) =>
      pstImportApi.upload(file, folder),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['pst-imports'] });
      setSelectedFile(null);
      setTargetFolder('INBOX');
      if (fileInputRef.current) {
        fileInputRef.current.value = '';
      }
    },
  });

  // Added: Delete mutation for cancelling/removing imports
  const deleteMutation = useMutation({
    mutationFn: pstImportApi.delete,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['pst-imports'] }),
  });

  // Added: Handle file selection from input or drop
  const handleFileSelect = useCallback((file: File) => {
    if (file.name.toLowerCase().endsWith('.pst')) {
      setSelectedFile(file);
    }
  }, []);

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  }, []);

  const handleDrop = useCallback((e: DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    const droppedFile = e.dataTransfer.files[0];
    if (droppedFile) {
      handleFileSelect(droppedFile);
    }
  }, [handleFileSelect]);

  const handleInputChange = useCallback((e: ChangeEvent<HTMLInputElement>) => {
    const inputFile = e.target.files?.[0];
    if (inputFile) {
      handleFileSelect(inputFile);
    }
  }, [handleFileSelect]);

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!selectedFile) return;
    uploadMutation.mutate({ file: selectedFile, folder: targetFolder });
  };

  // Added: Format file size for display (bytes to human-readable)
  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  };

  return (
    <div style={{ marginTop: '24px' }}>
      <h3 style={{ marginBottom: '12px' }}>
        <FileArchive size={16} style={{ marginRight: '6px', verticalAlign: 'middle' }} />
        PST Import (Outlook)
      </h3>

      <p style={{ fontSize: '13px', color: 'var(--color-text-secondary)', marginBottom: '12px' }}>
        Import emails from an Outlook .pst file. The file will be processed in the background using readpst.
      </p>

      <form className="settings-form" onSubmit={handleSubmit}>
        {/* Added: Drag-and-drop upload area */}
        <div
          className="pst-upload-area"
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          onClick={() => fileInputRef.current?.click()}
          style={{
            border: `2px dashed ${isDragging ? 'var(--color-primary)' : 'var(--color-border)'}`,
            borderRadius: '8px',
            padding: '24px',
            textAlign: 'center',
            cursor: 'pointer',
            backgroundColor: isDragging ? 'var(--color-primary-bg, rgba(0,100,200,0.05))' : 'transparent',
            transition: 'border-color 0.2s, background-color 0.2s',
            marginBottom: '12px',
          }}
          role="button"
          aria-label="Upload PST file"
        >
          <Upload size={24} style={{ marginBottom: '8px', color: 'var(--color-text-secondary)' }} />
          <p style={{ margin: 0, fontWeight: 500 }}>
            {selectedFile
              ? `${selectedFile.name} (${formatSize(selectedFile.size)})`
              : 'Drag & drop a .pst file here, or click to select'}
          </p>
          <input
            ref={fileInputRef}
            type="file"
            accept=".pst"
            onChange={handleInputChange}
            style={{ display: 'none' }}
            data-testid="pst-file-input"
          />
        </div>

        {/* Added: Target folder selector */}
        <div className="form-group">
          <label htmlFor="pst-target-folder">Target Folder</label>
          <select
            id="pst-target-folder"
            value={targetFolder}
            onChange={(e) => setTargetFolder(e.target.value)}
            data-testid="pst-target-folder"
          >
            <option value="INBOX">INBOX</option>
            <option value="Archive">Archive</option>
            <option value="Imported">Imported</option>
          </select>
        </div>

        <button
          type="submit"
          className="btn btn--primary"
          disabled={!selectedFile || uploadMutation.isPending}
        >
          {uploadMutation.isPending ? 'Uploading...' : 'Upload & Import'}
        </button>

        {uploadMutation.isError && (
          <p style={{ color: 'var(--color-danger)', fontSize: '13px', marginTop: '8px' }}>
            Upload failed: {(uploadMutation.error as Error).message}
          </p>
        )}
      </form>

      {/* Added: Import history table */}
      {imports.length > 0 && (
        <div style={{ marginTop: '16px' }}>
          <h4 style={{ marginBottom: '8px' }}>PST Import History</h4>
          <div className="pst-import-list">
            {imports.map((pstImport) => (
              <PstImportCard
                key={pstImport.id}
                pstImport={pstImport}
                onDelete={() => deleteMutation.mutate(pstImport.id)}
              />
            ))}
          </div>
        </div>
      )}

      {isLoading && <p className="loading">Loading import history...</p>}
    </div>
  );
}

/**
 * PURPOSE: Display a single PST import record with status badge and progress
 */
function PstImportCard({ pstImport, onDelete }: { pstImport: PstImport; onDelete: () => void }) {
  const canDelete = pstImport.status === 'pending' || pstImport.status === 'failed';
  const progress =
    pstImport.messages_found && pstImport.messages_found > 0
      ? Math.round(((pstImport.messages_imported ?? 0) / pstImport.messages_found) * 100)
      : 0;

  // Added: Map status to icon and color
  const statusConfig: Record<string, { icon: typeof CheckCircle; color: string }> = {
    pending: { icon: Clock, color: 'var(--color-warning, #f59e0b)' },
    processing: { icon: Loader, color: 'var(--color-primary, #3b82f6)' },
    completed: { icon: CheckCircle, color: 'var(--color-success, #22c55e)' },
    failed: { icon: AlertCircle, color: 'var(--color-danger, #ef4444)' },
  };

  const StatusIcon = statusConfig[pstImport.status]?.icon ?? Clock;
  const statusColor = statusConfig[pstImport.status]?.color ?? 'var(--color-text-secondary)';

  // Added: Format file size for display
  const formatSize = (bytes: number): string => {
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div
      className="pst-import-card"
      style={{
        padding: '12px',
        border: '1px solid var(--color-border)',
        borderRadius: '6px',
        marginBottom: '8px',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <strong>{pstImport.filename}</strong>
          <span style={{ color: 'var(--color-text-secondary)', marginLeft: '8px', fontSize: '12px' }}>
            {formatSize(pstImport.file_size)}
          </span>
          <span
            className={`badge badge--${pstImport.status}`}
            style={{ marginLeft: '8px', color: statusColor }}
          >
            <StatusIcon size={12} style={{ marginRight: '4px', verticalAlign: 'middle' }} />
            {pstImport.status}
          </span>
        </div>
        {canDelete && (
          <button className="btn btn--icon" onClick={onDelete} title="Cancel import">
            <X size={16} />
          </button>
        )}
      </div>

      {/* Added: Show target folder */}
      <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '4px' }}>
        Target: {pstImport.target_folder}
      </div>

      {/* Added: Progress bar for processing imports */}
      {pstImport.status === 'processing' && pstImport.messages_found != null && (
        <div style={{ marginTop: '8px' }}>
          <div style={{ background: 'var(--color-border)', borderRadius: '4px', height: '6px' }}>
            <div
              style={{
                width: `${progress}%`,
                background: 'var(--color-primary)',
                borderRadius: '4px',
                height: '100%',
                transition: 'width 0.3s',
              }}
            />
          </div>
          <span style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>
            {pstImport.messages_imported ?? 0}/{pstImport.messages_found} messages ({progress}%)
          </span>
        </div>
      )}

      {/* Added: Show import counts for completed imports */}
      {pstImport.status === 'completed' && pstImport.messages_imported != null && (
        <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '4px' }}>
          {pstImport.messages_imported} messages imported
        </div>
      )}

      {/* Added: Show error message for failed imports */}
      {pstImport.error_message && (
        <p style={{ color: 'var(--color-danger)', fontSize: '12px', marginTop: '4px' }}>
          {pstImport.error_message}
        </p>
      )}
    </div>
  );
}
