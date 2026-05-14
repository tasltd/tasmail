import { useState } from 'react';
import { Link } from 'react-router';
import { Settings, Menu, X, ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Sidebar } from '@/components/layout/Sidebar';
import { EmailList } from '@/features/email/EmailList';
import { EmailReader } from '@/features/email/EmailReader';
import { ComposeModal } from '@/features/email/ComposeModal';
import { mockEmails } from '@/data/mockData';

export function EmailClient() {
  const [activeFolder, setActiveFolder] = useState('inbox');
  const [selectedEmailId, setSelectedEmailId] = useState<string | null>(null);
  const [isComposing, setIsComposing] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(false);

  const filteredEmails = mockEmails.filter(email => email.folder === activeFolder);
  const selectedEmail = mockEmails.find(email => email.id === selectedEmailId) || null;

  // Mobile view states: 'sidebar' | 'list' | 'reader'
  const mobileView = selectedEmailId ? 'reader' : 'list';

  return (
    <div className="flex h-full relative">

      {/* Mobile sidebar overlay */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 bg-black/40 z-30 md:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      {/* Sidebar — hidden on mobile unless sidebarOpen */}
      <div className={`
        fixed inset-y-0 left-0 z-40 transition-transform duration-300
        md:static md:translate-x-0 md:z-auto
        ${sidebarOpen ? 'translate-x-0' : '-translate-x-full'}
      `}>
        <Sidebar
          activeFolder={activeFolder}
          onFolderChange={(folderId) => {
            setActiveFolder(folderId);
            setSelectedEmailId(null);
            setSidebarOpen(false);
          }}
          onCompose={() => { setIsComposing(true); setSidebarOpen(false); }}
        />
      </div>

      {/* Main content */}
      <div className="flex-1 flex overflow-hidden">

        {/* Email List Panel — full width on mobile when no email selected */}
        <div className={`
          border-r border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950
          flex flex-col
          w-full md:w-80 lg:w-96
          ${mobileView === 'reader' ? 'hidden md:flex' : 'flex'}
        `}>
          <div className="h-14 border-b border-zinc-200 dark:border-zinc-800 flex items-center justify-between px-4 shrink-0">
            <div className="flex items-center gap-2">
              {/* Mobile hamburger */}
              <Button
                variant="ghost"
                size="icon"
                className="md:hidden"
                onClick={() => setSidebarOpen(true)}
              >
                <Menu className="size-5" />
              </Button>
              <h2 className="font-semibold capitalize">{activeFolder}</h2>
            </div>
            <Link to="/admin">
              <Button variant="ghost" size="icon" title="Admin Dashboard">
                <Settings className="size-4" />
              </Button>
            </Link>
          </div>
          <div className="flex-1 overflow-y-auto">
            <EmailList
              emails={filteredEmails}
              selectedEmailId={selectedEmailId}
              onSelectEmail={setSelectedEmailId}
            />
          </div>
        </div>

        {/* Email Reader Panel — full width on mobile when email selected */}
        <div className={`
          flex-1 bg-white dark:bg-zinc-950 flex flex-col overflow-hidden
          ${mobileView === 'reader' ? 'flex' : 'hidden md:flex'}
        `}>
          {/* Mobile back button */}
          {selectedEmailId && (
            <div className="md:hidden h-11 border-b border-zinc-200 dark:border-zinc-800 flex items-center px-3">
              <Button
                variant="ghost"
                size="sm"
                className="gap-1 text-blue-600"
                onClick={() => setSelectedEmailId(null)}
              >
                <ArrowLeft className="size-4" />
                Back
              </Button>
            </div>
          )}
          <div className="flex-1 overflow-hidden">
            <EmailReader
              email={selectedEmail}
              onCompose={() => setIsComposing(true)}
            />
          </div>
        </div>
      </div>

      <ComposeModal
        isOpen={isComposing}
        onClose={() => setIsComposing(false)}
      />
    </div>
  );
}
