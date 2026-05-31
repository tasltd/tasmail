// Added (TMAIL-346): Shared wizard types. Step is the discriminator that drives
// the OnboardingWizard's render tree; ServerForm is the controlled-input shape
// for the IMAP and SMTP forms.
import type { Encryption } from '@/api/byok';

export type Step = 'provider' | 'imap' | 'smtp' | 'done';

export interface ServerForm {
  host: string;
  port: number;
  username: string;
  password: string;
  encryption: Encryption;
}

export const BLANK_IMAP: ServerForm = {
  host: '',
  port: 993,
  username: '',
  password: '',
  encryption: 'ssl',
};

export const BLANK_SMTP: ServerForm = {
  host: '',
  port: 587,
  username: '',
  password: '',
  encryption: 'starttls',
};
