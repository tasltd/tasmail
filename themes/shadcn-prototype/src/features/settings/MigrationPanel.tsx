// TMAIL-345: Modern UI Settings → Import pane. IMAP/MBOX/PST migration
// wizard with progress tracking + cancel.
//
// Three sub-tabs (IMAP, MBOX, PST) wired to the same /api/migration/*
// endpoints the classic SPA's MigrationManager + PstImportManager use. The
// pane is mounted by SettingsPage when the `import` tab is active — see
// tabs.ts → SettingsTab.component. Adding the pane was a one-line registry
// edit, no route changes.
//
// Progress polling: both migration jobs and PST imports refetch every 5s
// while the pane is open so users see live progress. Cancel + Delete close
// the loop so users can recover from a botched migration without leaving
// the page.
//
// Why no MBOX file upload here: the backend /api/migration/mbox endpoint
// takes a server-side file path, not a multipart file. The classic SPA has
// the same constraint. Users who want to upload an mbox file from their
// browser should use the PST tab (which DOES accept multipart) — supporting
// browser-uploaded mbox would need a new backend endpoint and is out of
// scope for TMAIL-345.
import {
  useCallback,
  useRef,
  useState,
  type ChangeEvent,
  type DragEvent,
} from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AlertCircle,
  CheckCircle,
  Clock,
  Download,
  FileArchive,
  Loader,
  Server,
  Upload,
  X,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/components/ui/utils';
import {
  cancelMigration,
  deletePstImport,
  listMigrations,
  listPstImports,
  startImapMigration,
  startMboxImport,
  uploadPst,
  type MigrationJob,
  type MigrationJobStatus,
  type PstImport,
  type PstImportStatus,
} from '@/api/migration';

export const MIGRATION_JOBS_QUERY_KEY = ['migration-jobs'] as const;
export const PST_IMPORTS_QUERY_KEY = ['pst-imports'] as const;

type SubTab = 'imap' | 'mbox' | 'pst';

const SUB_TABS: Array<{ id: SubTab; label: string; icon: typeof Server }> = [
  { id: 'imap', label: 'IMAP', icon: Server },
  { id: 'mbox', label: 'MBOX', icon: FileArchive },
  { id: 'pst', label: 'PST', icon: Download },
];

export function MigrationPanel() {
  const [subTab, setSubTab] = useState<SubTab>('imap');

  return (
    <div
      data-testid="settings-tab-import-pane"
      className="h-full w-full p-6 sm:p-8 overflow-y-auto"
    >
      <header className="flex items-center gap-3 mb-2">
        <Upload
          className="size-6 text-blue-600 dark:text-blue-400"
          aria-hidden="true"
        />
        <h2 className="text-xl sm:text-2xl font-semibold">Email migration</h2>
      </header>
      <p className="text-sm text-zinc-600 dark:text-zinc-400 max-w-2xl mb-6">
        Import existing email from another mailbox. Choose IMAP for a live
        server-to-server migration, MBOX for a Google Takeout / Thunderbird
        export, or PST for an Outlook archive file.
      </p>

      <nav
        role="tablist"
        aria-label="Migration source"
        className="flex gap-1 mb-6 border-b border-zinc-200 dark:border-zinc-800"
      >
        {SUB_TABS.map((t) => {
          const Icon = t.icon;
          const isActive = subTab === t.id;
          return (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              data-testid={`migration-subtab-${t.id}`}
              onClick={() => setSubTab(t.id)}
              className={cn(
                'flex items-center gap-2 px-3 py-2 text-sm rounded-t-md border-b-2 -mb-px transition-colors',
                isActive
                  ? 'border-blue-600 text-blue-700 dark:text-blue-300'
                  : 'border-transparent text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100',
              )}
            >
              <Icon className="size-4" aria-hidden="true" />
              {t.label}
            </button>
          );
        })}
      </nav>

      {subTab === 'imap' && <ImapMigrationForm />}
      {subTab === 'mbox' && <MboxImportForm />}
      {subTab === 'pst' && <PstUploadForm />}

      <MigrationJobsHistory />
      <PstImportsHistory />
    </div>
  );
}

// ── IMAP wizard ───────────────────────────────────────────────────────────

function ImapMigrationForm() {
  const queryClient = useQueryClient();
  const [host, setHost] = useState('');
  const [port, setPort] = useState('993');
  const [user, setUser] = useState('');
  const [password, setPassword] = useState('');
  const [ssl, setSsl] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: startImapMigration,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: MIGRATION_JOBS_QUERY_KEY });
      setHost('');
      setUser('');
      setPassword('');
      setPort('993');
      setSsl(true);
      setError(null);
    },
    onError: (e: Error) => setError(e.message || 'Failed to start IMAP migration.'),
  });

  const handleSubmit = (e: React.SyntheticEvent) => {
    e.preventDefault();
    if (!host.trim() || !user.trim() || !password) return;
    const portNum = Number.parseInt(port, 10);
    mutation.mutate({
      source_host: host.trim(),
      source_port: Number.isFinite(portNum) ? portNum : 993,
      source_user: user.trim(),
      source_password: password,
      source_use_ssl: ssl,
    });
  };

  return (
    <form
      data-testid="migration-imap-form"
      onSubmit={handleSubmit}
      className="space-y-4 max-w-xl"
    >
      <p className="text-sm text-zinc-600 dark:text-zinc-400">
        Migrate every folder from another IMAP server (Gmail, Outlook, Zoho,
        FastMail, an existing Dovecot, etc.). For Gmail / Outlook you almost
        always need an app-specific password, not your account password.
      </p>

      <Field label="IMAP server" htmlFor="migration-imap-host">
        <Input
          id="migration-imap-host"
          data-testid="migration-imap-host"
          value={host}
          onChange={(e) => setHost(e.target.value)}
          placeholder="imap.gmail.com"
          required
          autoComplete="off"
        />
      </Field>

      <Field label="Port" htmlFor="migration-imap-port">
        <Input
          id="migration-imap-port"
          data-testid="migration-imap-port"
          type="number"
          value={port}
          onChange={(e) => setPort(e.target.value)}
          placeholder="993"
          min={1}
          max={65535}
        />
      </Field>

      <Field label="Username" htmlFor="migration-imap-user">
        <Input
          id="migration-imap-user"
          data-testid="migration-imap-user"
          value={user}
          onChange={(e) => setUser(e.target.value)}
          placeholder="you@gmail.com"
          required
          autoComplete="off"
        />
      </Field>

      <Field
        label="Password / app-specific password"
        htmlFor="migration-imap-password"
      >
        <Input
          id="migration-imap-password"
          data-testid="migration-imap-password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="App password"
          required
          autoComplete="off"
        />
      </Field>

      <label className="inline-flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          data-testid="migration-imap-ssl"
          checked={ssl}
          onChange={(e) => setSsl(e.target.checked)}
          className="size-4"
        />
        Use SSL / TLS
      </label>

      {error && (
        <FormError testId="migration-imap-error">{error}</FormError>
      )}

      <Button
        type="submit"
        data-testid="migration-imap-submit"
        disabled={
          mutation.isPending ||
          !host.trim() ||
          !user.trim() ||
          !password
        }
      >
        <Upload className="size-4" />
        {mutation.isPending ? 'Starting…' : 'Start migration'}
      </Button>
    </form>
  );
}

// ── MBOX import ────────────────────────────────────────────────────────────

function MboxImportForm() {
  const queryClient = useQueryClient();
  const [path, setPath] = useState('');
  const [error, setError] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: startMboxImport,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: MIGRATION_JOBS_QUERY_KEY });
      setPath('');
      setError(null);
    },
    onError: (e: Error) => setError(e.message || 'Failed to start MBOX import.'),
  });

  const handleSubmit = (e: React.SyntheticEvent) => {
    e.preventDefault();
    if (!path.trim()) return;
    mutation.mutate({ mbox_file_path: path.trim() });
  };

  return (
    <form
      data-testid="migration-mbox-form"
      onSubmit={handleSubmit}
      className="space-y-4 max-w-xl"
    >
      <p className="text-sm text-zinc-600 dark:text-zinc-400">
        Import a Google Takeout / Thunderbird MBOX file that already lives on
        the server. Provide an absolute path the TASMail backend can read
        (e.g. <code>/srv/uploads/takeout.mbox</code>). To upload an MBOX
        directly from your browser, use the PST tab — the upload endpoint
        currently accepts <code>.pst</code> only.
      </p>

      <Field label="MBOX file path" htmlFor="migration-mbox-path">
        <Input
          id="migration-mbox-path"
          data-testid="migration-mbox-path"
          value={path}
          onChange={(e) => setPath(e.target.value)}
          placeholder="/srv/uploads/takeout.mbox"
          required
          autoComplete="off"
        />
      </Field>

      {error && (
        <FormError testId="migration-mbox-error">{error}</FormError>
      )}

      <Button
        type="submit"
        data-testid="migration-mbox-submit"
        disabled={mutation.isPending || !path.trim()}
      >
        <Upload className="size-4" />
        {mutation.isPending ? 'Starting…' : 'Start import'}
      </Button>
    </form>
  );
}

// ── PST upload ─────────────────────────────────────────────────────────────

function PstUploadForm() {
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [file, setFile] = useState<File | null>(null);
  const [folder, setFolder] = useState('INBOX');
  const [isDragging, setIsDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: ({ file, folder }: { file: File; folder: string }) =>
      uploadPst(file, folder),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: PST_IMPORTS_QUERY_KEY });
      setFile(null);
      setFolder('INBOX');
      if (fileInputRef.current) fileInputRef.current.value = '';
      setError(null);
    },
    onError: (e: Error) => setError(e.message || 'PST upload failed.'),
  });

  const handleFileSelect = useCallback((picked: File | undefined | null) => {
    if (!picked) return;
    if (!picked.name.toLowerCase().endsWith('.pst')) {
      setError(`File "${picked.name}" is not a .pst file.`);
      return;
    }
    setError(null);
    setFile(picked);
  }, []);

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);
  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  }, []);
  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      setIsDragging(false);
      handleFileSelect(e.dataTransfer.files[0]);
    },
    [handleFileSelect],
  );
  const handleInputChange = (e: ChangeEvent<HTMLInputElement>) => {
    handleFileSelect(e.target.files?.[0]);
  };

  const handleSubmit = (e: React.SyntheticEvent) => {
    e.preventDefault();
    if (!file) return;
    mutation.mutate({ file, folder });
  };

  return (
    <form
      data-testid="migration-pst-form"
      onSubmit={handleSubmit}
      className="space-y-4 max-w-xl"
    >
      <p className="text-sm text-zinc-600 dark:text-zinc-400">
        Upload an Outlook <code>.pst</code> archive. The backend extracts it
        with <code>readpst</code> and APPENDs every message into the IMAP
        folder you pick below. Large files are processed asynchronously —
        watch the history table for progress.
      </p>

      <div
        data-testid="migration-pst-dropzone"
        role="button"
        tabIndex={0}
        aria-label="Upload PST file"
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        onClick={() => fileInputRef.current?.click()}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            fileInputRef.current?.click();
          }
        }}
        className={cn(
          'rounded-lg border-2 border-dashed p-6 text-center cursor-pointer transition-colors',
          isDragging
            ? 'border-blue-500 bg-blue-50/60 dark:bg-blue-950/30'
            : 'border-zinc-300 dark:border-zinc-700 hover:border-blue-400',
        )}
      >
        <Upload
          className="mx-auto size-6 text-zinc-500 dark:text-zinc-400 mb-2"
          aria-hidden="true"
        />
        <p className="font-medium text-sm">
          {file
            ? `${file.name} (${formatBytes(file.size)})`
            : 'Drag & drop a .pst file here, or click to pick one'}
        </p>
        <input
          ref={fileInputRef}
          type="file"
          accept=".pst"
          onChange={handleInputChange}
          className="hidden"
          data-testid="migration-pst-file-input"
        />
      </div>

      <Field label="Target folder" htmlFor="migration-pst-folder">
        <select
          id="migration-pst-folder"
          data-testid="migration-pst-folder"
          value={folder}
          onChange={(e) => setFolder(e.target.value)}
          className="block w-full rounded-md border border-zinc-300 dark:border-zinc-700 bg-white dark:bg-zinc-950 px-3 py-2 text-sm"
        >
          <option value="INBOX">INBOX</option>
          <option value="Archive">Archive</option>
          <option value="Imported">Imported</option>
        </select>
      </Field>

      {error && (
        <FormError testId="migration-pst-error">{error}</FormError>
      )}

      <Button
        type="submit"
        data-testid="migration-pst-submit"
        disabled={!file || mutation.isPending}
      >
        <Upload className="size-4" />
        {mutation.isPending ? 'Uploading…' : 'Upload & import'}
      </Button>
    </form>
  );
}

// ── History (migration jobs) ───────────────────────────────────────────────

function MigrationJobsHistory() {
  const queryClient = useQueryClient();
  const { data: jobs, isLoading } = useQuery({
    queryKey: MIGRATION_JOBS_QUERY_KEY,
    queryFn: listMigrations,
    refetchInterval: 5000,
  });

  const cancelMut = useMutation({
    mutationFn: cancelMigration,
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: MIGRATION_JOBS_QUERY_KEY }),
  });

  if (isLoading) {
    return (
      <section
        data-testid="migration-jobs-loading"
        className="mt-8 text-sm text-zinc-500"
      >
        Loading migration history…
      </section>
    );
  }

  if (!jobs || jobs.length === 0) {
    return (
      <section
        data-testid="migration-jobs-empty"
        className="mt-8 rounded-lg border border-dashed border-zinc-300 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-900/40 p-4 text-sm text-zinc-500"
      >
        No IMAP or MBOX migrations yet. Start one above and progress will
        appear here.
      </section>
    );
  }

  return (
    <section
      data-testid="migration-jobs-list"
      aria-label="IMAP and MBOX migration history"
      className="mt-8"
    >
      <h3 className="text-base font-semibold mb-2">IMAP / MBOX history</h3>
      <div className="space-y-2">
        {jobs.map((job) => (
          <MigrationJobRow
            key={job.id}
            job={job}
            disabled={cancelMut.isPending}
            onCancel={() => cancelMut.mutate(job.id)}
          />
        ))}
      </div>
    </section>
  );
}

function MigrationJobRow({
  job,
  disabled,
  onCancel,
}: {
  job: MigrationJob;
  disabled: boolean;
  onCancel: () => void;
}) {
  const isActive = job.status === 'pending' || job.status === 'running';
  const progress =
    job.messages_total && job.messages_total > 0
      ? Math.round(((job.messages_done ?? 0) / job.messages_total) * 100)
      : 0;
  const { Icon, color } = statusVisual(job.status);

  return (
    <article
      data-testid={`migration-job-${job.id}`}
      data-status={job.status}
      className="rounded-lg border border-zinc-200 dark:border-zinc-800 p-3"
    >
      <header className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="text-sm font-medium truncate">
            {job.job_type === 'imap'
              ? `IMAP · ${job.source_host ?? 'unknown host'}`
              : `MBOX · ${job.mbox_file_path ?? 'unknown path'}`}
          </p>
          <p className="text-xs text-zinc-500 mt-0.5 flex items-center gap-1">
            <Icon
              className="size-3.5 shrink-0"
              aria-hidden="true"
              style={{ color }}
            />
            <span>{job.status}</span>
            {job.source_user && (
              <span className="truncate">· {job.source_user}</span>
            )}
          </p>
        </div>
        {isActive && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="size-7 shrink-0"
            title="Cancel migration"
            aria-label="Cancel migration"
            data-testid={`migration-job-cancel-${job.id}`}
            disabled={disabled}
            onClick={onCancel}
          >
            <X className="size-4" />
          </Button>
        )}
      </header>
      {job.status === 'running' && job.messages_total != null && (
        <div className="mt-2">
          <div className="h-1.5 rounded-full bg-zinc-200 dark:bg-zinc-800 overflow-hidden">
            <div
              data-testid={`migration-job-progress-${job.id}`}
              className="h-full bg-blue-600 transition-[width] duration-300"
              style={{ width: `${progress}%` }}
            />
          </div>
          <p className="text-xs text-zinc-500 mt-1">
            {(job.messages_done ?? 0).toLocaleString()} /{' '}
            {job.messages_total.toLocaleString()} messages ({progress}%)
          </p>
        </div>
      )}
      {job.error_message && (
        <p
          data-testid={`migration-job-error-${job.id}`}
          className="text-xs text-red-600 dark:text-red-400 mt-1"
        >
          {job.error_message}
        </p>
      )}
    </article>
  );
}

// ── History (PST imports) ──────────────────────────────────────────────────

function PstImportsHistory() {
  const queryClient = useQueryClient();
  const { data: imports, isLoading } = useQuery({
    queryKey: PST_IMPORTS_QUERY_KEY,
    queryFn: listPstImports,
    refetchInterval: 5000,
  });

  const deleteMut = useMutation({
    mutationFn: deletePstImport,
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: PST_IMPORTS_QUERY_KEY }),
  });

  if (isLoading) {
    return (
      <section
        data-testid="pst-imports-loading"
        className="mt-6 text-sm text-zinc-500"
      >
        Loading PST imports…
      </section>
    );
  }

  if (!imports || imports.length === 0) {
    return (
      <section
        data-testid="pst-imports-empty"
        className="mt-6 rounded-lg border border-dashed border-zinc-300 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-900/40 p-4 text-sm text-zinc-500"
      >
        No Outlook PST imports yet. Upload a <code>.pst</code> file above to
        see it here.
      </section>
    );
  }

  return (
    <section
      data-testid="pst-imports-list"
      aria-label="PST import history"
      className="mt-6"
    >
      <h3 className="text-base font-semibold mb-2">PST import history</h3>
      <div className="space-y-2">
        {imports.map((row) => (
          <PstImportRow
            key={row.id}
            pstImport={row}
            disabled={deleteMut.isPending}
            onDelete={() => deleteMut.mutate(row.id)}
          />
        ))}
      </div>
    </section>
  );
}

function PstImportRow({
  pstImport,
  disabled,
  onDelete,
}: {
  pstImport: PstImport;
  disabled: boolean;
  onDelete: () => void;
}) {
  const canDelete =
    pstImport.status === 'pending' || pstImport.status === 'failed';
  const progress =
    pstImport.messages_found && pstImport.messages_found > 0
      ? Math.round(
          ((pstImport.messages_imported ?? 0) / pstImport.messages_found) * 100,
        )
      : 0;
  const { Icon, color } = pstStatusVisual(pstImport.status);

  return (
    <article
      data-testid={`pst-import-${pstImport.id}`}
      data-status={pstImport.status}
      className="rounded-lg border border-zinc-200 dark:border-zinc-800 p-3"
    >
      <header className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="text-sm font-medium truncate">{pstImport.filename}</p>
          <p className="text-xs text-zinc-500 mt-0.5 flex items-center gap-1">
            <Icon
              className="size-3.5 shrink-0"
              aria-hidden="true"
              style={{ color }}
            />
            <span>{pstImport.status}</span>
            <span>· {formatBytes(pstImport.file_size)}</span>
            <span>· → {pstImport.target_folder}</span>
          </p>
        </div>
        {canDelete && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="size-7 shrink-0"
            title="Cancel PST import"
            aria-label="Cancel PST import"
            data-testid={`pst-import-cancel-${pstImport.id}`}
            disabled={disabled}
            onClick={onDelete}
          >
            <X className="size-4" />
          </Button>
        )}
      </header>
      {pstImport.status === 'processing' &&
        pstImport.messages_found != null && (
          <div className="mt-2">
            <div className="h-1.5 rounded-full bg-zinc-200 dark:bg-zinc-800 overflow-hidden">
              <div
                data-testid={`pst-import-progress-${pstImport.id}`}
                className="h-full bg-blue-600 transition-[width] duration-300"
                style={{ width: `${progress}%` }}
              />
            </div>
            <p className="text-xs text-zinc-500 mt-1">
              {(pstImport.messages_imported ?? 0).toLocaleString()} /{' '}
              {pstImport.messages_found.toLocaleString()} messages ({progress}%)
            </p>
          </div>
        )}
      {pstImport.status === 'completed' &&
        pstImport.messages_imported != null && (
          <p className="text-xs text-zinc-500 mt-1">
            {pstImport.messages_imported.toLocaleString()} messages imported.
          </p>
        )}
      {pstImport.error_message && (
        <p
          data-testid={`pst-import-error-${pstImport.id}`}
          className="text-xs text-red-600 dark:text-red-400 mt-1"
        >
          {pstImport.error_message}
        </p>
      )}
    </article>
  );
}

// ── Tiny shared bits ───────────────────────────────────────────────────────

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1">
      <label
        htmlFor={htmlFor}
        className="text-sm font-medium text-zinc-700 dark:text-zinc-300"
      >
        {label}
      </label>
      {children}
    </div>
  );
}

function FormError({
  children,
  testId,
}: {
  children: React.ReactNode;
  testId: string;
}) {
  return (
    <div
      role="alert"
      data-testid={testId}
      className="rounded-md border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-800 dark:bg-red-950 dark:text-red-200"
    >
      {children}
    </div>
  );
}

function statusVisual(status: MigrationJobStatus): {
  Icon: typeof Clock;
  color: string;
} {
  switch (status) {
    case 'pending':
      return { Icon: Clock, color: '#f59e0b' };
    case 'running':
      return { Icon: Loader, color: '#3b82f6' };
    case 'completed':
      return { Icon: CheckCircle, color: '#22c55e' };
    case 'failed':
      return { Icon: AlertCircle, color: '#ef4444' };
    case 'cancelled':
      return { Icon: X, color: '#71717a' };
  }
}

function pstStatusVisual(status: PstImportStatus): {
  Icon: typeof Clock;
  color: string;
} {
  switch (status) {
    case 'pending':
      return { Icon: Clock, color: '#f59e0b' };
    case 'processing':
      return { Icon: Loader, color: '#3b82f6' };
    case 'completed':
      return { Icon: CheckCircle, color: '#22c55e' };
    case 'failed':
      return { Icon: AlertCircle, color: '#ef4444' };
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}
