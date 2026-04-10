import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Upload, X, Server, FileArchive } from 'lucide-react';
import { migrationApi } from '../../api/migration';
import type { MigrationJob } from '../../types/migration';

export function MigrationManager() {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<'imap' | 'mbox'>('imap');
  const [imapForm, setImapForm] = useState({
    source_host: '', source_port: '993', source_user: '', source_password: '', source_use_ssl: true,
  });
  const [mboxPath, setMboxPath] = useState('');

  const { data: jobs = [], isLoading } = useQuery({
    queryKey: ['migration-jobs'],
    queryFn: migrationApi.list,
    refetchInterval: 5000, // Poll for progress updates
  });

  const imapMutation = useMutation({
    mutationFn: migrationApi.startImap,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['migration-jobs'] });
      setImapForm({ source_host: '', source_port: '993', source_user: '', source_password: '', source_use_ssl: true });
    },
  });

  const mboxMutation = useMutation({
    mutationFn: migrationApi.startMbox,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['migration-jobs'] });
      setMboxPath('');
    },
  });

  const cancelMutation = useMutation({
    mutationFn: migrationApi.cancel,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['migration-jobs'] }),
  });

  const handleImapSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    imapMutation.mutate({
      source_host: imapForm.source_host,
      source_port: parseInt(imapForm.source_port) || 993,
      source_user: imapForm.source_user,
      source_password: imapForm.source_password,
      source_use_ssl: imapForm.source_use_ssl,
    });
  };

  const handleMboxSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mboxMutation.mutate({ mbox_file_path: mboxPath });
  };

  return (
    <div className="settings-panel">
      <div className="settings-panel__header">
        <h2><Upload size={20} /> Email Migration</h2>
      </div>

      <div className="tabs" style={{ display: 'flex', gap: '8px', marginBottom: '16px' }}>
        <button className={`btn ${tab === 'imap' ? 'btn--primary' : ''}`} onClick={() => setTab('imap')}>
          <Server size={16} /> IMAP Migration
        </button>
        <button className={`btn ${tab === 'mbox' ? 'btn--primary' : ''}`} onClick={() => setTab('mbox')}>
          <FileArchive size={16} /> MBOX Import
        </button>
      </div>

      {tab === 'imap' && (
        <form className="settings-form" onSubmit={handleImapSubmit}>
          <p style={{ fontSize: '13px', color: 'var(--color-text-secondary)', marginBottom: '12px' }}>
            Migrate emails from another IMAP server (Gmail, Outlook, etc.)
          </p>
          <div className="form-group">
            <label>IMAP Server</label>
            <input type="text" value={imapForm.source_host} onChange={(e) => setImapForm({ ...imapForm, source_host: e.target.value })}
              placeholder="imap.gmail.com" required />
          </div>
          <div className="form-group">
            <label>Port</label>
            <input type="number" value={imapForm.source_port} onChange={(e) => setImapForm({ ...imapForm, source_port: e.target.value })}
              placeholder="993" />
          </div>
          <div className="form-group">
            <label>Username</label>
            <input type="text" value={imapForm.source_user} onChange={(e) => setImapForm({ ...imapForm, source_user: e.target.value })}
              placeholder="user@gmail.com" required />
          </div>
          <div className="form-group">
            <label>Password / App Password</label>
            <input type="password" value={imapForm.source_password} onChange={(e) => setImapForm({ ...imapForm, source_password: e.target.value })}
              placeholder="App-specific password" required />
          </div>
          <div className="form-group" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <input type="checkbox" checked={imapForm.source_use_ssl} onChange={(e) => setImapForm({ ...imapForm, source_use_ssl: e.target.checked })} id="ssl" />
            <label htmlFor="ssl">Use SSL/TLS</label>
          </div>
          <button type="submit" className="btn btn--primary" disabled={imapMutation.isPending}>
            {imapMutation.isPending ? 'Starting...' : 'Start Migration'}
          </button>
        </form>
      )}

      {tab === 'mbox' && (
        <form className="settings-form" onSubmit={handleMboxSubmit}>
          <p style={{ fontSize: '13px', color: 'var(--color-text-secondary)', marginBottom: '12px' }}>
            Import emails from a Google Takeout MBOX file
          </p>
          <div className="form-group">
            <label>MBOX File Path</label>
            <input type="text" value={mboxPath} onChange={(e) => setMboxPath(e.target.value)}
              placeholder="/path/to/takeout.mbox" required />
          </div>
          <button type="submit" className="btn btn--primary" disabled={mboxMutation.isPending}>
            {mboxMutation.isPending ? 'Starting...' : 'Start Import'}
          </button>
        </form>
      )}

      {/* Job history */}
      {jobs.length > 0 && (
        <div style={{ marginTop: '24px' }}>
          <h3 style={{ marginBottom: '8px' }}>Migration History</h3>
          <div className="migration-jobs">
            {jobs.map((job) => (
              <JobCard key={job.id} job={job} onCancel={() => cancelMutation.mutate(job.id)} />
            ))}
          </div>
        </div>
      )}

      {isLoading && <p className="loading">Loading migration history...</p>}
    </div>
  );
}

function JobCard({ job, onCancel }: { job: MigrationJob; onCancel: () => void }) {
  const isActive = job.status === 'pending' || job.status === 'running';
  const progress = job.messages_total && job.messages_total > 0
    ? Math.round((job.messages_done ?? 0) / job.messages_total * 100)
    : 0;

  return (
    <div className="migration-job" style={{ padding: '12px', border: '1px solid var(--color-border)', borderRadius: '6px', marginBottom: '8px' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <strong>{job.job_type === 'imap' ? 'IMAP' : 'MBOX'}</strong>
          {job.source_host && <span style={{ color: 'var(--color-text-secondary)', marginLeft: '8px' }}>{job.source_host}</span>}
          <span className={`badge badge--${job.status}`} style={{ marginLeft: '8px' }}>{job.status}</span>
        </div>
        {isActive && (
          <button className="btn btn--icon" onClick={onCancel} title="Cancel">
            <X size={16} />
          </button>
        )}
      </div>
      {job.status === 'running' && job.messages_total != null && (
        <div style={{ marginTop: '8px' }}>
          <div style={{ background: 'var(--color-border)', borderRadius: '4px', height: '6px' }}>
            <div style={{ width: `${progress}%`, background: 'var(--color-primary)', borderRadius: '4px', height: '100%', transition: 'width 0.3s' }} />
          </div>
          <span style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>
            {job.messages_done ?? 0}/{job.messages_total} messages ({progress}%)
          </span>
        </div>
      )}
      {job.error_message && (
        <p style={{ color: 'var(--color-danger)', fontSize: '12px', marginTop: '4px' }}>{job.error_message}</p>
      )}
    </div>
  );
}
