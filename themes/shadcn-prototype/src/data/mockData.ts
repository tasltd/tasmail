export interface Email {
  id: string;
  from: string;
  fromEmail: string;
  to: string;
  subject: string;
  preview: string;
  body: string;
  timestamp: Date;
  read: boolean;
  starred: boolean;
  folder: string;
  attachments?: Array<{ name: string; size: string }>;
}

export interface Folder {
  id: string;
  name: string;
  icon: string;
  count: number;
  isCustom?: boolean;
}

export interface Mailbox {
  id: string;
  user: string;
  domain: string;
  quotaUsed: number;
  quotaTotal: number;
  status: 'active' | 'suspended' | 'disabled';
}

export const mockEmails: Email[] = [
  {
    id: '1',
    from: 'Sarah Chen',
    fromEmail: 'sarah@privacy.mail',
    to: 'me@mydomain.com',
    subject: 'Welcome to your self-hosted email!',
    preview: 'Congratulations on setting up your privacy-first email system...',
    body: `<p>Hi there!</p>
    <p>Congratulations on setting up your privacy-first email system. You now have complete control over your communications.</p>
    <p>Here are some key features:</p>
    <ul>
      <li>End-to-end encryption support</li>
      <li>No tracking or data mining</li>
      <li>Custom domain support</li>
      <li>Unlimited storage (based on your server)</li>
    </ul>
    <p>Best regards,<br/>Sarah</p>`,
    timestamp: new Date('2026-03-07T10:30:00'),
    read: false,
    starred: true,
    folder: 'inbox',
    attachments: [{ name: 'setup-guide.pdf', size: '2.4 MB' }]
  },
  {
    id: '2',
    from: 'System Admin',
    fromEmail: 'admin@mydomain.com',
    to: 'me@mydomain.com',
    subject: 'Server backup completed successfully',
    preview: 'Your scheduled backup has completed. All mailboxes backed up...',
    body: `<p>Backup Report - March 7, 2026</p>
    <p>Your scheduled backup has completed successfully.</p>
    <p><strong>Backup Details:</strong></p>
    <ul>
      <li>Total mailboxes: 12</li>
      <li>Total size: 8.4 GB</li>
      <li>Duration: 14 minutes</li>
      <li>Status: ✓ Success</li>
    </ul>
    <p>Next backup scheduled for: March 8, 2026 at 2:00 AM</p>`,
    timestamp: new Date('2026-03-07T02:15:00'),
    read: true,
    starred: false,
    folder: 'inbox'
  },
  {
    id: '3',
    from: 'Marcus Rodriguez',
    fromEmail: 'marcus@techcorp.net',
    to: 'me@mydomain.com',
    subject: 'Re: Project proposal for Q2',
    preview: 'Thanks for sending over the proposal. I have reviewed it with...',
    body: `<p>Hi,</p>
    <p>Thanks for sending over the proposal. I have reviewed it with the team and we're impressed with the approach.</p>
    <p>Could we schedule a call next week to discuss the timeline and resources needed?</p>
    <p>Let me know your availability.</p>
    <p>Cheers,<br/>Marcus</p>`,
    timestamp: new Date('2026-03-06T16:45:00'),
    read: false,
    starred: false,
    folder: 'inbox'
  },
  {
    id: '4',
    from: 'Newsletter Weekly',
    fromEmail: 'news@techweekly.com',
    to: 'me@mydomain.com',
    subject: 'This week in privacy tech - March 2026',
    preview: 'Your weekly roundup of privacy and security news...',
    body: `<h2>This Week's Headlines</h2>
    <p>Your weekly roundup of privacy and security news.</p>
    <ol>
      <li>New EU regulations on data privacy go into effect</li>
      <li>Open-source email servers see 40% adoption increase</li>
      <li>Encryption standards updated for 2026</li>
    </ol>
    <p>Read more at our website...</p>`,
    timestamp: new Date('2026-03-06T08:00:00'),
    read: true,
    starred: false,
    folder: 'inbox'
  },
  {
    id: '5',
    from: 'Emma Thompson',
    fromEmail: 'emma@designstudio.io',
    to: 'client@designstudio.io',
    subject: 'Final designs attached',
    preview: 'Please find the final design mockups attached. Looking forward...',
    body: `<p>Hi Team,</p>
    <p>Please find the final design mockups attached. Looking forward to your feedback!</p>
    <p>All files are in the latest version.</p>
    <p>Best,<br/>Emma</p>`,
    timestamp: new Date('2026-03-05T14:20:00'),
    read: true,
    starred: false,
    folder: 'sent',
    attachments: [
      { name: 'mockup-v3.fig', size: '12.8 MB' },
      { name: 'assets.zip', size: '5.2 MB' }
    ]
  },
  {
    id: '6',
    from: 'Draft',
    fromEmail: 'me@mydomain.com',
    to: '',
    subject: 'Monthly newsletter ideas',
    preview: 'Topic ideas for the next newsletter: 1. Self-hosting benefits...',
    body: `<p>Topic ideas for the next newsletter:</p>
    <ol>
      <li>Self-hosting benefits</li>
      <li>Privacy tips for email</li>
      <li>Custom domain setup guide</li>
    </ol>`,
    timestamp: new Date('2026-03-04T11:00:00'),
    read: true,
    starred: false,
    folder: 'drafts'
  },
  {
    id: '7',
    from: 'Spam Bot',
    fromEmail: 'winner@lottery-scam.xyz',
    to: 'me@mydomain.com',
    subject: 'YOU WON $1,000,000!!!',
    preview: 'Claim your prize now by clicking this suspicious link...',
    body: `<p>CONGRATULATIONS!!!</p><p>You have won one million dollars. Click here to claim.</p>`,
    timestamp: new Date('2026-03-03T22:15:00'),
    read: false,
    starred: false,
    folder: 'spam'
  },
  {
    id: '8',
    from: 'Old Account',
    fromEmail: 'old@example.com',
    to: 'me@mydomain.com',
    subject: 'Account migration reminder',
    preview: 'This email is from an old service you no longer use...',
    body: `<p>This is a reminder about your old account.</p>`,
    timestamp: new Date('2026-02-28T09:00:00'),
    read: true,
    starred: false,
    folder: 'trash'
  }
];

export const mockFolders: Folder[] = [
  { id: 'inbox', name: 'Inbox', icon: 'Inbox', count: 3 },
  { id: 'sent', name: 'Sent', icon: 'Send', count: 0 },
  { id: 'drafts', name: 'Drafts', icon: 'FileText', count: 1 },
  { id: 'spam', name: 'Spam', icon: 'AlertOctagon', count: 1 },
  { id: 'trash', name: 'Trash', icon: 'Trash2', count: 1 },
  { id: 'work', name: 'Work', icon: 'Briefcase', count: 0, isCustom: true },
  { id: 'personal', name: 'Personal', icon: 'User', count: 0, isCustom: true }
];

export const mockMailboxes: Mailbox[] = [
  {
    id: '1',
    user: 'admin@mydomain.com',
    domain: 'mydomain.com',
    quotaUsed: 4.2,
    quotaTotal: 10,
    status: 'active'
  },
  {
    id: '2',
    user: 'sarah@mydomain.com',
    domain: 'mydomain.com',
    quotaUsed: 2.8,
    quotaTotal: 5,
    status: 'active'
  },
  {
    id: '3',
    user: 'team@mydomain.com',
    domain: 'mydomain.com',
    quotaUsed: 8.5,
    quotaTotal: 10,
    status: 'active'
  },
  {
    id: '4',
    user: 'support@mydomain.com',
    domain: 'mydomain.com',
    quotaUsed: 1.2,
    quotaTotal: 5,
    status: 'active'
  },
  {
    id: '5',
    user: 'info@seconddomain.com',
    domain: 'seconddomain.com',
    quotaUsed: 0.5,
    quotaTotal: 5,
    status: 'suspended'
  },
  {
    id: '6',
    user: 'contact@seconddomain.com',
    domain: 'seconddomain.com',
    quotaUsed: 3.1,
    quotaTotal: 5,
    status: 'active'
  }
];

export const mockStats = {
  totalUsers: 12,
  storageUsed: 28.4,
  storageTotal: 100,
  messagesToday: 47,
  messagesThisWeek: 312,
  activeDomains: 3
};
