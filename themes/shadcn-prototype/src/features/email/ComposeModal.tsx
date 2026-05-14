import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { X, Minimize2, Maximize2, Paperclip, Send, Save, Bold, Italic, Link as LinkIcon, List } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { scheduledApi } from '@/api/scheduled';

interface ComposeModalProps {
  isOpen: boolean;
  onClose: () => void;
}

// TMAIL-219: send via scheduledApi.scheduleSend so the modern UI uses the
// same code path the production SPA's composer does. delay_seconds=0 means
// "send immediately" for now — the modern UI doesn't have an undo banner
// yet; that can ship later if desired.
function splitAddrs(s: string): string[] {
  return s.split(/[,;]/).map((x) => x.trim()).filter(Boolean);
}

export function ComposeModal({ isOpen, onClose }: ComposeModalProps) {
  const queryClient = useQueryClient();
  const [minimized, setMinimized] = useState(false);
  const [showCc, setShowCc] = useState(false);
  const [showBcc, setShowBcc] = useState(false);
  const [to, setTo] = useState('');
  const [cc, setCc] = useState('');
  const [bcc, setBcc] = useState('');
  const [subject, setSubject] = useState('');
  const [body, setBody] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [attachments, setAttachments] = useState<File[]>([]);

  const sendMut = useMutation({
    mutationFn: () => scheduledApi.scheduleSend({
      to: splitAddrs(to),
      cc: cc.trim() ? splitAddrs(cc) : undefined,
      bcc: bcc.trim() ? splitAddrs(bcc) : undefined,
      subject,
      text_body: body || undefined,
      delay_seconds: 0,
    }),
    onSuccess: () => {
      // Sent message lands in the user's Sent folder; bump folder counts so
      // the sidebar's unread badges refresh.
      queryClient.invalidateQueries({ queryKey: ['folders'] });
      queryClient.invalidateQueries({ queryKey: ['messages'] });
      // Reset the form for the next compose.
      setTo(''); setCc(''); setBcc(''); setSubject(''); setBody('');
      setError(null);
      onClose();
    },
    onError: (err: Error) => setError(err.message || 'Send failed.'),
  });

  if (!isOpen) return null;

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files) {
      setAttachments([...attachments, ...Array.from(e.target.files)]);
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  };

  const totalSize = attachments.reduce((sum, file) => sum + file.size, 0);
  const maxSize = 25 * 1024 * 1024; // 25MB

  if (minimized) {
    return (
      <div className="fixed bottom-0 right-0 sm:right-4 w-full sm:w-80 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-t-lg shadow-2xl z-50">
        <div className="flex items-center justify-between p-3 border-b border-zinc-200 dark:border-zinc-800 cursor-pointer hover:bg-zinc-50 dark:hover:bg-zinc-800" onClick={() => setMinimized(false)}>
          <span className="font-medium">New Message</span>
          <div className="flex items-center gap-1">
            <Button variant="ghost" size="icon" className="size-8">
              <Maximize2 className="size-4" />
            </Button>
            <Button variant="ghost" size="icon" className="size-8" onClick={(e) => { e.stopPropagation(); onClose(); }}>
              <X className="size-4" />
            </Button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 sm:inset-auto sm:bottom-0 sm:right-4 sm:w-[560px] bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 sm:rounded-t-lg shadow-2xl flex flex-col sm:max-h-[600px] z-50">
      {/* Header */}
      <div className="flex items-center justify-between p-3 border-b border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-800">
        <span className="font-medium">New Message</span>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" className="size-8" onClick={() => setMinimized(true)}>
            <Minimize2 className="size-4" />
          </Button>
          <Button variant="ghost" size="icon" className="size-8" onClick={onClose}>
            <X className="size-4" />
          </Button>
        </div>
      </div>

      {/* Recipients */}
      <div className="border-b border-zinc-200 dark:border-zinc-800">
        <div className="flex items-center px-3 py-2 border-b border-zinc-200 dark:border-zinc-800">
          <span className="text-sm text-zinc-500 w-16">To</span>
          <Input
            type="text"
            value={to}
            onChange={(e) => setTo(e.target.value)}
            placeholder="alice@example.com, bob@example.com"
            className="flex-1 border-0 focus-visible:ring-0 focus-visible:ring-offset-0"
          />
          <div className="flex gap-2 text-sm">
            <button onClick={() => setShowCc(!showCc)} className="text-blue-600 hover:underline">Cc</button>
            <button onClick={() => setShowBcc(!showBcc)} className="text-blue-600 hover:underline">Bcc</button>
          </div>
        </div>

        {showCc && (
          <div className="flex items-center px-3 py-2 border-b border-zinc-200 dark:border-zinc-800">
            <span className="text-sm text-zinc-500 w-16">Cc</span>
            <Input
              type="text"
              value={cc}
              onChange={(e) => setCc(e.target.value)}
              placeholder="Carbon copy"
              className="flex-1 border-0 focus-visible:ring-0 focus-visible:ring-offset-0"
            />
          </div>
        )}

        {showBcc && (
          <div className="flex items-center px-3 py-2 border-b border-zinc-200 dark:border-zinc-800">
            <span className="text-sm text-zinc-500 w-16">Bcc</span>
            <Input
              type="text"
              value={bcc}
              onChange={(e) => setBcc(e.target.value)}
              placeholder="Blind carbon copy"
              className="flex-1 border-0 focus-visible:ring-0 focus-visible:ring-offset-0"
            />
          </div>
        )}

        <div className="flex items-center px-3 py-2">
          <span className="text-sm text-zinc-500 w-16">Subject</span>
          <Input
            type="text"
            value={subject}
            onChange={(e) => setSubject(e.target.value)}
            placeholder="Subject"
            className="flex-1 border-0 focus-visible:ring-0 focus-visible:ring-offset-0"
          />
        </div>
      </div>

      {/* Rich Text Toolbar */}
      <div className="flex items-center gap-1 px-3 py-2 border-b border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-800/50">
        <Button variant="ghost" size="icon" className="size-8" title="Bold">
          <Bold className="size-4" />
        </Button>
        <Button variant="ghost" size="icon" className="size-8" title="Italic">
          <Italic className="size-4" />
        </Button>
        <Button variant="ghost" size="icon" className="size-8" title="Insert link">
          <LinkIcon className="size-4" />
        </Button>
        <Button variant="ghost" size="icon" className="size-8" title="Bullet list">
          <List className="size-4" />
        </Button>
        <div className="flex-1" />
        <label htmlFor="file-upload">
          <Button variant="ghost" size="icon" className="size-8" title="Attach files" asChild>
            <span>
              <Paperclip className="size-4" />
            </span>
          </Button>
        </label>
        <input
          id="file-upload"
          type="file"
          multiple
          onChange={handleFileSelect}
          className="hidden"
        />
      </div>

      {/* Message Body */}
      <div className="flex-1 p-3 overflow-y-auto">
        <Textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder="Compose your message..."
          className="min-h-[150px] sm:min-h-[250px] h-full border-0 focus-visible:ring-0 focus-visible:ring-offset-0 resize-none"
        />
        {error && (
          <div role="alert" className="mt-2 text-sm text-red-600">{error}</div>
        )}

        {/* Attachments */}
        {attachments.length > 0 && (
          <div className="mt-4 space-y-2">
            <div className="flex items-center justify-between text-sm">
              <span className="font-medium">Attachments</span>
              <span className={totalSize > maxSize ? 'text-red-600' : 'text-zinc-500'}>
                {formatBytes(totalSize)} / 25 MB
              </span>
            </div>
            {attachments.map((file, index) => (
              <div
                key={index}
                className="flex items-center justify-between p-2 bg-zinc-50 dark:bg-zinc-800 rounded border border-zinc-200 dark:border-zinc-700"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <Paperclip className="size-4 text-zinc-500 flex-shrink-0" />
                  <span className="text-sm truncate">{file.name}</span>
                  <span className="text-xs text-zinc-500 flex-shrink-0">
                    {formatBytes(file.size)}
                  </span>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-6 flex-shrink-0"
                  onClick={() => setAttachments(attachments.filter((_, i) => i !== index))}
                >
                  <X className="size-3" />
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="flex items-center justify-between p-3 border-t border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-800 shrink-0">
        <div className="flex gap-2">
          <Button
            className="bg-blue-600 hover:bg-blue-700"
            onClick={() => sendMut.mutate()}
            disabled={sendMut.isPending || !to.trim() || !subject.trim()}
          >
            <Send className="size-4 mr-1 sm:mr-2" />
            {sendMut.isPending ? 'Sending…' : 'Send'}
          </Button>
          <Button variant="outline" disabled title="Drafts not yet wired in the modern UI">
            <Save className="size-4 mr-1 sm:mr-2" />
            <span className="hidden xs:inline sm:inline">Save Draft</span>
            <span className="xs:hidden sm:hidden">Save</span>
          </Button>
        </div>
        <Button variant="ghost" size="sm" onClick={onClose}>
          Discard
        </Button>
      </div>
    </div>
  );
}
