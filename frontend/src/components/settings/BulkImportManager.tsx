// Added: Bulk user import management component for TMAIL-136
import { useState, useCallback } from 'react';
import type { DragEvent, ChangeEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Upload, Download, ChevronDown, ChevronRight, UserPlus } from 'lucide-react';
import { bulkImportApi } from '../../api/bulk-import';
import type { BulkUserImport, BulkImportError } from '../../api/bulk-import';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

/**
 * PURPOSE: Badge component showing import status with appropriate color
 * CONSTRAINTS: Only renders known bulk import statuses
 */
function StatusBadge({ status }: { status: BulkUserImport['status'] }) {
  // Added: Color mapping for each import status
  const statusColors: Record<string, string> = {
    pending: '#2196f3',
    processing: '#ff9800',
    completed: '#4caf50',
    failed: '#f44336',
  };

  const displayLabel = status.charAt(0).toUpperCase() + status.slice(1);

  return (
    <span
      style={{
        fontSize: '11px',
        background: statusColors[status] || '#888',
        color: 'white',
        padding: '2px 8px',
        borderRadius: '10px',
        whiteSpace: 'nowrap',
      }}
    >
      {displayLabel}
    </span>
  );
}

/**
 * PURPOSE: Expandable error detail list for a single import record
 */
function ErrorDetails({ errors }: { errors: BulkImportError[] }) {
  if (!errors || errors.length === 0) return null;

  return (
    <div style={{ marginTop: '8px', padding: '8px', background: 'var(--color-surface)', borderRadius: '4px' }}>
      <table style={{ width: '100%', fontSize: '12px', borderCollapse: 'collapse' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid var(--color-border)' }}>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Row</th>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Field</th>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Error</th>
          </tr>
        </thead>
        <tbody>
          {errors.map((importError, errorIndex) => (
            <tr key={errorIndex} style={{ borderBottom: '1px solid var(--color-border)' }}>
              <td style={{ padding: '4px 8px' }}>{importError.row || '-'}</td>
              <td style={{ padding: '4px 8px' }}>{importError.field}</td>
              <td style={{ padding: '4px 8px', color: '#f44336' }}>{importError.message}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/**
 * PURPOSE: Admin bulk user import UI with CSV upload, template download, and import history
 * CONSTRAINTS: Requires admin role; CSV must match expected template format
 * EXTERNAL: Uses bulkImportApi for all backend communication
 */
export function BulkImportManager() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const queryClient = useQueryClient();
  const [isDragOver, setIsDragOver] = useState(false);
  const [expandedImportId, setExpandedImportId] = useState<string | null>(null);

  // Added: Fetch import history
  const { data: imports, isLoading } = useQuery({
    queryKey: ['bulk-imports'],
    queryFn: bulkImportApi.list,
  });

  // Added: Upload mutation with optimistic cache invalidation
  const uploadMutation = useMutation({
    mutationFn: (file: File) => bulkImportApi.upload(file),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bulk-imports'] });
    },
  });

  // Added: Handle file selection from input or drag-and-drop
  const handleFileUpload = useCallback(
    (file: File) => {
      if (!file.name.toLowerCase().endsWith('.csv')) {
        alert('Please select a CSV file.');
        return;
      }
      uploadMutation.mutate(file);
    },
    [uploadMutation],
  );

  // Added: Drag-and-drop event handlers
  const handleDragOver = useCallback((dragEvent: DragEvent<HTMLDivElement>) => {
    dragEvent.preventDefault();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback(() => {
    setIsDragOver(false);
  }, []);

  const handleDrop = useCallback(
    (dropEvent: DragEvent<HTMLDivElement>) => {
      dropEvent.preventDefault();
      setIsDragOver(false);
      const droppedFile = dropEvent.dataTransfer.files[0];
      if (droppedFile) {
        handleFileUpload(droppedFile);
      }
    },
    [handleFileUpload],
  );

  const handleInputChange = useCallback(
    (inputEvent: ChangeEvent<HTMLInputElement>) => {
      const selectedFile = inputEvent.target.files?.[0];
      if (selectedFile) {
        handleFileUpload(selectedFile);
      }
    },
    [handleFileUpload],
  );

  // Added: Toggle expand/collapse of import error details
  const toggleExpand = useCallback((importId: string) => {
    setExpandedImportId((previous) => (previous === importId ? null : importId));
  }, []);

  if (isLoading) return <LoadingSkeleton />;

  return (
    <div style={{ padding: '16px', maxWidth: '800px' }}>
      {/* Added: Header with back button */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '20px' }}>
        <button className="btn btn--icon" title="Back" onClick={() => setViewMode('list')}>
          <ArrowLeft size={18} />
        </button>
        <UserPlus size={22} />
        <h2 style={{ margin: 0 }}>Bulk User Import</h2>
      </div>

      {/* Added: Download template + Export users buttons (TMAIL-136) */}
      <div style={{ marginBottom: '16px', display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
        <button
          className="btn btn--secondary"
          onClick={() => bulkImportApi.downloadTemplate()}
          style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
        >
          <Download size={16} />
          Download CSV Template
        </button>
        <button
          className="btn btn--secondary"
          onClick={() => {
            bulkImportApi.exportUsers().catch((exportError: Error) => {
              alert(`Export failed: ${exportError.message}`);
            });
          }}
          style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
          data-testid="export-users-button"
        >
          <Download size={16} />
          Export Users (CSV)
        </button>
      </div>

      {/* Added: CSV upload area with drag-and-drop support */}
      <div
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        style={{
          border: `2px dashed ${isDragOver ? '#2196f3' : 'var(--color-border)'}`,
          borderRadius: '8px',
          padding: '32px',
          textAlign: 'center',
          background: isDragOver ? 'rgba(33, 150, 243, 0.05)' : 'transparent',
          marginBottom: '24px',
          transition: 'all 0.2s',
        }}
      >
        <Upload size={32} style={{ color: 'var(--color-text-secondary)', marginBottom: '8px' }} />
        <p style={{ margin: '0 0 8px' }}>
          {uploadMutation.isPending ? 'Uploading...' : 'Drag and drop a CSV file here, or click to browse'}
        </p>
        <input
          type="file"
          accept=".csv"
          onChange={handleInputChange}
          disabled={uploadMutation.isPending}
          style={{ display: 'inline-block' }}
          data-testid="csv-file-input"
        />
        {uploadMutation.isError && (
          <p style={{ color: '#f44336', marginTop: '8px' }}>
            Error: {uploadMutation.error?.message || 'Upload failed'}
          </p>
        )}
      </div>

      {/* Added: Import history list */}
      <h3>Import History</h3>
      {!imports || imports.length === 0 ? (
        <p style={{ color: 'var(--color-text-secondary)' }}>No bulk imports yet.</p>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
          {imports.map((importRecord) => (
            <div
              key={importRecord.id}
              style={{
                border: '1px solid var(--color-border)',
                borderRadius: '6px',
                padding: '12px',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px', justifyContent: 'space-between' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flex: 1 }}>
                  {/* Added: Expand/collapse toggle for imports with errors */}
                  {importRecord.error_count > 0 ? (
                    <button
                      className="btn btn--icon"
                      onClick={() => toggleExpand(importRecord.id)}
                      title="Toggle errors"
                      style={{ padding: '2px' }}
                    >
                      {expandedImportId === importRecord.id ? (
                        <ChevronDown size={16} />
                      ) : (
                        <ChevronRight size={16} />
                      )}
                    </button>
                  ) : (
                    <span style={{ width: '20px' }} />
                  )}
                  <span style={{ fontWeight: 500 }}>{importRecord.filename}</span>
                  <StatusBadge status={importRecord.status} />
                </div>
                <div style={{ display: 'flex', gap: '16px', fontSize: '13px', color: 'var(--color-text-secondary)' }}>
                  <span>Total: {importRecord.total_rows}</span>
                  <span style={{ color: '#4caf50' }}>Success: {importRecord.success_count}</span>
                  <span style={{ color: importRecord.error_count > 0 ? '#f44336' : 'inherit' }}>
                    Errors: {importRecord.error_count}
                  </span>
                  <span>{new Date(importRecord.created_at).toLocaleDateString()}</span>
                </div>
              </div>
              {/* Added: Expandable error details section */}
              {expandedImportId === importRecord.id && importRecord.errors && (
                <ErrorDetails errors={importRecord.errors} />
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
