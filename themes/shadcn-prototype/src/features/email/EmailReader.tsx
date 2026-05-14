import { Reply, ReplyAll, Forward, Trash2, Archive, Star, Download } from 'lucide-react';
import { format } from 'date-fns';
import { Button } from '@/components/ui/button';
import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import type { Email } from '@/data/mockData';

interface EmailReaderProps {
  email: Email | null;
  onCompose: () => void;
}

export function EmailReader({ email, onCompose }: EmailReaderProps) {
  if (!email) {
    return (
      <div className="flex items-center justify-center h-full text-zinc-500">
        Select an email to read
      </div>
    );
  }

  const initials = email.from
    .split(' ')
    .map(n => n[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);

  return (
    <div className="flex flex-col h-full">
      {/* Email Header */}
      <div className="border-b border-zinc-200 dark:border-zinc-800 p-4">
        <div className="flex items-start justify-between mb-4">
          <h2 className="text-2xl font-semibold flex-1">{email.subject}</h2>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => {
              // Toggle star logic
            }}
          >
            <Star
              className={`size-5 ${
                email.starred
                  ? 'fill-yellow-400 text-yellow-400'
                  : 'text-zinc-400 hover:text-yellow-400'
              }`}
            />
          </Button>
        </div>

        <div className="flex items-center gap-3 mb-4">
          <Avatar>
            <AvatarFallback className="bg-gradient-to-br from-blue-500 to-purple-600 text-white">
              {initials}
            </AvatarFallback>
          </Avatar>
          <div className="flex-1 min-w-0">
            <div className="font-medium truncate">{email.from}</div>
            <div className="text-sm text-zinc-500 truncate">
              {email.fromEmail} → {email.to}
            </div>
          </div>
          <div className="text-xs sm:text-sm text-zinc-500 shrink-0 ml-2">
            <span className="hidden sm:inline">{format(email.timestamp, 'MMM d, yyyy • h:mm a')}</span>
            <span className="sm:hidden">{format(email.timestamp, 'MMM d')}</span>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center gap-1 sm:gap-2 flex-wrap">
          <Button variant="outline" size="sm" onClick={onCompose}>
            <Reply className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Reply</span>
          </Button>
          <Button variant="outline" size="sm" onClick={onCompose}>
            <ReplyAll className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Reply All</span>
          </Button>
          <Button variant="outline" size="sm" onClick={onCompose}>
            <Forward className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Forward</span>
          </Button>
          <div className="flex-1" />
          <Button variant="outline" size="sm">
            <Archive className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Archive</span>
          </Button>
          <Button variant="outline" size="sm">
            <Trash2 className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Delete</span>
          </Button>
        </div>
      </div>

      {/* Email Body */}
      <div className="flex-1 overflow-y-auto p-6">
        <div
          className="prose dark:prose-invert max-w-none"
          dangerouslySetInnerHTML={{ __html: email.body }}
        />

        {/* Attachments */}
        {email.attachments && email.attachments.length > 0 && (
          <div className="mt-8 space-y-2">
            <h3 className="text-sm font-medium text-zinc-700 dark:text-zinc-300 mb-3">
              Attachments ({email.attachments.length})
            </h3>
            {email.attachments.map((attachment, index) => (
              <div
                key={index}
                className="flex items-center justify-between p-3 border border-zinc-200 dark:border-zinc-800 rounded-lg bg-zinc-50 dark:bg-zinc-900"
              >
                <div className="flex items-center gap-3">
                  <div className="size-10 rounded bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center">
                    <Download className="size-5 text-blue-600 dark:text-blue-400" />
                  </div>
                  <div>
                    <div className="font-medium text-sm">{attachment.name}</div>
                    <div className="text-xs text-zinc-500">{attachment.size}</div>
                  </div>
                </div>
                <Button variant="outline" size="sm">
                  <Download className="size-4 mr-2" />
                  Download
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
