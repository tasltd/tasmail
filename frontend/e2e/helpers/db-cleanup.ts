// Added: TMAIL-194 — direct DB cleanup so the noreply BYOK E2E can re-run idempotently.
//
// Reaches into the local Postgres that backs the live mail.techatscale.io
// deployment (via the SSH reverse tunnel — same DB the live backend talks to).
// We use psql via child_process because the frontend has no pg client and we
// don't want to add one just for this.
//
// Override via TASMAIL_DB_URL=postgres://user:pass@host:port/db if the local
// dev DB credentials rotate.

import { execFileSync } from 'node:child_process';

const DEFAULT_DB_URL = 'postgresql://alleina:alleina_dev_2026@127.0.0.1:5432/tasmail';

function dbUrl(): string {
  return process.env.TASMAIL_DB_URL ?? DEFAULT_DB_URL;
}

/**
 * Hard-deletes the TASMail mailbox with this username so a follow-up signup
 * starts clean. Returns the number of rows deleted (0 if it didn't exist).
 *
 * All FKs that point at mailboxes use ON DELETE CASCADE or SET NULL, so this
 * is safe — no need to scrub child tables first.
 */
export function deleteMailboxByUsername(username: string): number {
  const sql = `DELETE FROM mailboxes WHERE username = $$${username}$$ RETURNING id;`;
  try {
    const out = execFileSync('psql', [dbUrl(), '-At', '-c', sql], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const rows = out.trim().split('\n').filter((line) => line && !line.startsWith('DELETE '));
    return rows.length;
  } catch (err) {
    throw new Error(
      `deleteMailboxByUsername(${username}) failed — is psql installed and TASMAIL_DB_URL reachable? ` +
        `Original error: ${(err as Error).message}`,
    );
  }
}

/**
 * Flips a mailbox's `is_admin` flag so the user picks up the `is_admin: true`
 * claim on their next login. Used by TMAIL-402 to prove the Admin sidebar
 * entry only renders for admins.
 *
 * Returns the number of rows updated (0 if no mailbox with that username
 * exists). The caller must re-login (or re-issue the JWT) to refresh the
 * decoded claim — existing tokens are not retroactively elevated.
 */
export function setMailboxAdmin(username: string, isAdmin: boolean): number {
  const sql = `UPDATE mailboxes SET is_admin = ${isAdmin ? 'true' : 'false'} WHERE username = $$${username}$$ RETURNING id;`;
  try {
    const out = execFileSync('psql', [dbUrl(), '-At', '-c', sql], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const rows = out.trim().split('\n').filter((line) => line && !line.startsWith('UPDATE '));
    return rows.length;
  } catch (err) {
    throw new Error(
      `setMailboxAdmin(${username}, ${isAdmin}) failed — is psql installed and TASMAIL_DB_URL reachable? ` +
        `Original error: ${(err as Error).message}`,
    );
  }
}
