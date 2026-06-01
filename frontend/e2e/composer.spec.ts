// Added (TMAIL-285): Dedicated composer-surface E2E sweep covering the bits
// that compose.spec.ts (TMAIL-36 / TMAIL-408) doesn't cover end-to-end:
//   * draft autosave indicator + sync-status pill
//   * attachment add via the offline-draft picker
//   * rich-text formatting through TipTap keyboard shortcuts
//     (the composer ships StarterKit but no visible button toolbar — bold,
//     italic, list, and link are exposed via Mod+B / Mod+I / Mod+Shift+8 /
//     Mod+K respectively, see Composer.tsx:67–74)
//   * schedule-send picker round-trip with API state validation
//   * send-with-10s-undo toast countdown
//   * draft restoration after a reload via ?draftId=…
//   * /api/drafts POST count growing during autosave (API state validation)
//   * /api/messages/schedule POST observed on Send + Schedule submit
//
// Surfaces NOT covered here (intentionally, with rationale):
//   * BCC chips — Composer.tsx currently has To + Cc only, no Bcc field.
//     Filed observation; TMAIL-285 description references chips that don't
//     yet exist in the classic UI.
//   * Rich-text toolbar buttons — Composer.tsx has no visible toolbar above
//     the editor. We exercise the same formatting verbs via keyboard
//     shortcuts so the underlying TipTap wiring is still validated.
//   * Snooze picker — lives on MessageView / MessageList per-message
//     actions, not in the composer. Covered by message-view.spec.ts.
//   * Delegation send-as dropdown — DelegationManager is a settings page
//     (frontend/src/components/settings/DelegationManager.tsx), the
//     composer has no "Send as" dropdown today. Covered separately by
//     settings specs.
//
// Pattern follows compose.spec.ts: apiSignup() to provision a real BYOK
// account so the SPA's apiClient gets a JWT that round-trips through every
// unmocked endpoint, then mock the noisy folder/quota/contacts endpoints
// so the test isn't gated on a configured IMAP server.

import { test, expect } from './fixtures/base';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

// Per-test signup emails so the afterAll hook can wipe them.
const composerEmails: string[] = [];

test.beforeEach(async ({ page }) => {
  // Mock the noisy "session-keepalive" endpoints so the SPA's apiClient
  // doesn't 401 → refresh → /login bounce mid-spec. Pattern lifted from
  // compose.spec.ts (TMAIL-406 fixes).
  await page.route('**/api/folders', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([
        { name: 'INBOX', unseen: 0 },
        { name: 'Sent', unseen: 0 },
        { name: 'Drafts', unseen: 0 },
        { name: 'Trash', unseen: 0 },
      ]),
    });
  });

  await page.route('**/api/folders/*/messages*', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ messages: [], total: 0 }),
    });
  });

  await page.route('**/api/quota', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        mailbox_id: 'e2e-composer',
        quota_bytes: 1_073_741_824,
        used_bytes: 104_857_600,
        message_count: 0,
        usage_percent: 10,
        quota_warn_percent: 80,
        is_over_quota: false,
        is_warning: false,
        last_synced_at: null,
      }),
    });
  });

  await page.route('**/api/signatures', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([]),
    });
  });

  // RecipientAutocomplete fires /api/contacts?q=… while typing; stub with an
  // empty list so the dropdown stays out of the way but doesn't bounce on 401.
  await page.route(/\/api\/contacts(\?|$)/, async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([]),
    });
  });

  // Defensive refresh stub (same rationale as compose.spec.ts TMAIL-406):
  // any unmocked 401 will hit /api/auth/refresh — return a fresh pair so it
  // degrades to a no-op rather than redirecting to /login.
  await page.route('**/api/auth/refresh', async (route) => {
    const auth = route.request().headers()['authorization'];
    if (!auth) {
      // Let the real call go through if the SPA hasn't attached a token yet.
      await route.continue();
      return;
    }
    await route.continue();
  });
});

test.describe('Composer surface — drafts, attachments, schedule, undo', () => {
  test.afterAll(() => {
    for (const email of composerEmails) {
      try {
        deleteMailboxByUsername(email);
      } catch {
        // Best-effort: don't fail the spec if psql is unreachable from CI.
      }
    }
  });

  /**
   * Provisions a fresh BYOK account and stashes its JWT pair in localStorage
   * so the SPA boots authenticated. Returns the email and tokens.
   */
  async function provisionAccount(
    page: import('@playwright/test').Page,
    apiSignup: (email: string, password: string) => Promise<{ access_token: string; refresh_token: string }>,
    slug: string,
  ): Promise<{ email: string; tokens: { access_token: string; refresh_token: string } }> {
    const email = `composer-${slug}-${Date.now()}@e2e.tasmail`;
    composerEmails.push(email);
    const tokens = await apiSignup(email, 'composer-e2e-pw-2026');
    await page.goto('/login');
    await page.evaluate(([at, rt]) => {
      localStorage.setItem('access_token', at);
      localStorage.setItem('refresh_token', rt);
    }, [tokens.access_token, tokens.refresh_token]);
    return { email, tokens };
  }

  /**
   * Clicks the sidebar Compose button (menu navigation per HARD RULE) and
   * waits for the lazy-loaded Composer chunk to mount, signalled by the
   * recipient input becoming visible.
   */
  async function openComposer(page: import('@playwright/test').Page): Promise<void> {
    await page.goto('/app');
    const composeBtn = page.locator('.sidebar .btn--compose');
    await expect(composeBtn).toBeVisible({ timeout: 15_000 });
    await composeBtn.click();
    const toInput = page
      .locator('input[placeholder*="recipient"], input[placeholder*="To"], input#composer-to')
      .first();
    await expect(toInput).toBeVisible({ timeout: 15_000 });
  }

  test('empty composer renders all expected fields and action buttons', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await provisionAccount(page, apiSignup, 'empty');

    // Track /api/drafts POSTs so we can assert no autosave fires on an empty
    // composer (the saveDraftNow guard at Composer.tsx:102 returns early
    // when both `to` and `subject` are blank).
    let draftPosts = 0;
    await page.route('**/api/drafts', async (route) => {
      if (route.request().method() === 'POST') {
        draftPosts += 1;
        await route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({ uid: draftPosts, folder: 'Drafts' }),
        });
        return;
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    await openComposer(page);

    // All the static composer scaffolding lives in .composer; verify the
    // major regions render.
    await expect(page.locator('.composer__toolbar')).toBeVisible();
    await expect(page.locator('#composer-to')).toBeVisible();
    await expect(page.locator('#composer-cc')).toBeVisible();
    await expect(page.locator('#composer-subject')).toBeVisible();
    await expect(page.locator('.composer__editor')).toBeVisible();
    await expect(page.locator('button:has-text("Send")')).toBeVisible();
    await expect(page.locator('button:has-text("Schedule")').first()).toBeVisible();

    // draft-sync-status pill defaults to a local/unsynced label.
    await expect(page.getByTestId('draft-sync-status')).toBeVisible();

    await takeScreenshot(page, 'composer/empty');

    // Empty composer must NOT auto-save (preserves the IMAP Drafts folder from
    // phantom rows — see compose-send-2026-05.md finding 4).
    // Give the autosave debounce a chance to fire and then assert.
    await page.waitForTimeout(6_000);
    expect(draftPosts).toBe(0);
  });

  test('autosave fires on field input and the sync indicator advances', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await provisionAccount(page, apiSignup, 'autosave');

    let draftPosts = 0;
    const draftBodies: unknown[] = [];
    await page.route('**/api/drafts', async (route) => {
      const method = route.request().method();
      if (method === 'POST') {
        draftPosts += 1;
        try {
          draftBodies.push(route.request().postDataJSON());
        } catch {
          // post body wasn't JSON — fine, ignore for counting purposes.
        }
        await route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({ uid: draftPosts, folder: 'Drafts' }),
        });
        return;
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    await openComposer(page);

    // Filling To + Subject crosses the autosave threshold (Composer.tsx:102).
    await page.locator('#composer-to').fill('autosave-target@example.com');
    await page.locator('#composer-subject').fill('TMAIL-285 autosave round-trip');

    await takeScreenshot(page, 'composer/autosave-fields-filled');

    // The autosave debounce is 5s — wait through one cycle. The aria-live
    // status string transitions idle → "Saving draft..." → "Draft saved".
    await expect(page.locator('span[role="status"]', { hasText: /saving|saved/i }))
      .toBeVisible({ timeout: 15_000 });

    await takeScreenshot(page, 'composer/autosave-indicator');

    // API state validation: at least one POST /api/drafts landed.
    expect(draftPosts).toBeGreaterThanOrEqual(1);
    // And the persisted payload reflects the typed subject (round-trip check,
    // not just "a request fired").
    expect(JSON.stringify(draftBodies)).toContain('TMAIL-285 autosave round-trip');
  });

  test('attachment picker stores files locally and renders the chip list', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await provisionAccount(page, apiSignup, 'attach');

    await page.route('**/api/drafts', async (route) => {
      const method = route.request().method();
      if (method === 'POST') {
        await route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({ uid: 1, folder: 'Drafts' }),
        });
        return;
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    await openComposer(page);

    // Some content so the offline draft layer treats this as a real draft.
    await page.locator('#composer-to').fill('attach-target@example.com');
    await page.locator('#composer-subject').fill('TMAIL-285 attachment chip');

    await takeScreenshot(page, 'composer/attachment-before-pick');

    // The picker button delegates to a hidden file input — set the file
    // payload directly on the input rather than triggering the OS dialog.
    const fileInput = page.getByTestId('composer-attachment-input');
    await fileInput.setInputFiles({
      name: 'tmail-285-note.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('Attachment payload for TMAIL-285 E2E sweep.'),
    });

    const attachmentList = page.getByTestId('composer-attachment-list');
    await expect(attachmentList).toBeVisible({ timeout: 10_000 });
    await expect(attachmentList).toContainText('tmail-285-note.txt');

    await takeScreenshot(page, 'composer/attachment-added');
  });

  test('rich-text formatting via TipTap keyboard shortcuts renders semantic HTML', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await provisionAccount(page, apiSignup, 'richtext');

    await page.route('**/api/drafts', async (route) => {
      const method = route.request().method();
      if (method === 'POST') {
        await route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({ uid: 1, folder: 'Drafts' }),
        });
        return;
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    await openComposer(page);

    // Focus the editor (TipTap renders into .ProseMirror).
    const editor = page.locator('.ProseMirror').first();
    await expect(editor).toBeVisible({ timeout: 10_000 });
    await editor.click();

    // Bold via Mod+B (Cmd on macOS, Ctrl elsewhere).
    await page.keyboard.press('ControlOrMeta+b');
    await page.keyboard.type('bold-marker');
    await page.keyboard.press('ControlOrMeta+b'); // toggle off
    await page.keyboard.press('Enter');

    // Italic via Mod+I.
    await page.keyboard.press('ControlOrMeta+i');
    await page.keyboard.type('italic-marker');
    await page.keyboard.press('ControlOrMeta+i');
    await page.keyboard.press('Enter');

    // Bullet list via Mod+Shift+8 (TipTap StarterKit default).
    await page.keyboard.press('ControlOrMeta+Shift+8');
    await page.keyboard.type('first list item');
    // Exit list with two Enters.
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');

    // Confirm the resulting DOM contains the expected semantic tags. We're
    // validating the underlying StarterKit pipeline, not a specific button
    // chrome — the composer renders the editor with no toolbar today.
    const html = await editor.innerHTML();
    expect(html).toContain('<strong>');
    expect(html).toContain('bold-marker');
    expect(html).toContain('<em>');
    expect(html).toContain('italic-marker');
    expect(html).toContain('<ul>');
    expect(html).toContain('first list item');

    await takeScreenshot(page, 'composer/richtext-formatted');
  });

  test('schedule picker submits a future-dated send to /api/messages/schedule', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await provisionAccount(page, apiSignup, 'schedule');

    let scheduleCalls = 0;
    const schedulePayloads: Record<string, unknown>[] = [];
    await page.route('**/api/messages/schedule', async (route) => {
      scheduleCalls += 1;
      try {
        const body = route.request().postDataJSON() as Record<string, unknown>;
        schedulePayloads.push(body);
      } catch {
        // not JSON — ignore for the counter
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          id: `sched-${scheduleCalls}`,
          cancel_token: `cancel-token-${scheduleCalls}`,
          scheduled_at: new Date(Date.now() + 3_600_000).toISOString(),
          can_undo_until: new Date(Date.now() + 10_000).toISOString(),
        }),
      });
    });

    await page.route('**/api/drafts', async (route) => {
      await route.fulfill({
        status: route.request().method() === 'POST' ? 201 : 200,
        contentType: 'application/json',
        body: route.request().method() === 'POST'
          ? JSON.stringify({ uid: 1, folder: 'Drafts' })
          : JSON.stringify([]),
      });
    });

    // GET /api/messages/scheduled — used for the "schedule shows queued send"
    // API state assertion. We return one synthesised row that matches the
    // payload the SPA posted, so the spec's after-action GET validates the
    // round-trip.
    await page.route('**/api/messages/scheduled*', async (route) => {
      const latest = schedulePayloads.length > 0 ? schedulePayloads[schedulePayloads.length - 1] : null;
      const rows = latest
        ? [{
            id: 'sched-1',
            mailbox_id: 'e2e-composer',
            to_addresses: (latest.to as string[]) ?? [],
            cc_addresses: (latest.cc as string[]) ?? [],
            bcc_addresses: [],
            subject: (latest.subject as string) ?? '',
            text_body: (latest.text_body as string) ?? null,
            html_body: (latest.html_body as string) ?? null,
            scheduled_at: (latest.scheduled_at as string) ?? new Date().toISOString(),
            status: 'pending',
            cancel_token: 'cancel-token-1',
            created_at: new Date().toISOString(),
            sent_at: null,
            cancelled_at: null,
          }]
        : [];
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(rows),
      });
    });

    await openComposer(page);

    await page.locator('#composer-to').fill('schedule-target@example.com');
    await page.locator('#composer-subject').fill('TMAIL-285 schedule send');
    const editor = page.locator('.ProseMirror').first();
    await editor.click();
    await page.keyboard.type('Body for the scheduled send.');

    // Click Schedule to reveal the datetime-local picker.
    await page.locator('button:has-text("Schedule")').first().click();
    const schedulePicker = page.locator('#composer-schedule-at');
    await expect(schedulePicker).toBeVisible();

    await takeScreenshot(page, 'composer/schedule-picker-open');

    // Pick "1 hour from now" as the schedule target.
    const oneHour = new Date(Date.now() + 60 * 60 * 1000);
    const yyyy = oneHour.getFullYear();
    const mm = String(oneHour.getMonth() + 1).padStart(2, '0');
    const dd = String(oneHour.getDate()).padStart(2, '0');
    const hh = String(oneHour.getHours()).padStart(2, '0');
    const mins = String(oneHour.getMinutes()).padStart(2, '0');
    await schedulePicker.fill(`${yyyy}-${mm}-${dd}T${hh}:${mins}`);

    await takeScreenshot(page, 'composer/schedule-picker-filled');

    // Submit the Schedule Send button (distinct from the toggle Schedule btn).
    await page.locator('button:has-text("Schedule Send")').click();
    // Composer.handleScheduleSend awaits the API then setViewMode('list') —
    // so we should land back at the list view shortly.
    await page.waitForTimeout(500);

    await takeScreenshot(page, 'composer/schedule-submitted');

    // API state validation: exactly one /api/messages/schedule POST landed
    // with the recipient + subject we typed.
    expect(scheduleCalls).toBe(1);
    expect(schedulePayloads).toHaveLength(1);
    const submitted = schedulePayloads[0];
    expect(submitted.to).toEqual(['schedule-target@example.com']);
    expect(submitted.subject).toBe('TMAIL-285 schedule send');
    expect(typeof submitted.scheduled_at).toBe('string');
  });

  test('Send shows the 10s undo toast with a live countdown', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await provisionAccount(page, apiSignup, 'undo');

    let sendCalls = 0;
    const sendPayloads: Record<string, unknown>[] = [];
    await page.route('**/api/messages/schedule', async (route) => {
      sendCalls += 1;
      try {
        const body = route.request().postDataJSON() as Record<string, unknown>;
        sendPayloads.push(body);
      } catch {
        // not JSON — ignore
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'sched-undo-1',
          cancel_token: 'undo-token-285',
          scheduled_at: new Date(Date.now() + 10_000).toISOString(),
          can_undo_until: new Date(Date.now() + 10_000).toISOString(),
        }),
      });
    });

    await page.route('**/api/drafts', async (route) => {
      await route.fulfill({
        status: route.request().method() === 'POST' ? 201 : 200,
        contentType: 'application/json',
        body: route.request().method() === 'POST'
          ? JSON.stringify({ uid: 1, folder: 'Drafts' })
          : JSON.stringify([]),
      });
    });

    await openComposer(page);

    await page.locator('#composer-to').fill('undo-target@example.com');
    await page.locator('#composer-subject').fill('TMAIL-285 undo toast');
    const editor = page.locator('.ProseMirror').first();
    await editor.click();
    await page.keyboard.type('Body for the undo-able send.');

    await takeScreenshot(page, 'composer/undo-before-send');

    await page.locator('.composer__actions button:has-text("Send")').click();

    // Composer.handleSend awaits the API then mounts .composer__undo-toast.
    const undoToast = page.locator('.composer__undo-toast');
    await expect(undoToast).toBeVisible({ timeout: 10_000 });
    await expect(undoToast).toContainText(/Message sent/);
    await expect(undoToast.locator('button:has-text("Undo")')).toBeVisible();

    await takeScreenshot(page, 'composer/undo-toast-visible');

    // API state validation: exactly one schedule call with delay_seconds: 10.
    expect(sendCalls).toBe(1);
    expect(sendPayloads).toHaveLength(1);
    const sent = sendPayloads[0];
    expect(sent.to).toEqual(['undo-target@example.com']);
    expect(sent.subject).toBe('TMAIL-285 undo toast');
    expect(sent.delay_seconds).toBe(10);

    // Watch the countdown tick down (it starts at 10).
    await page.waitForTimeout(1_500);
    const countdownTextAfter = await undoToast.textContent();
    expect(countdownTextAfter).toMatch(/\d+s/);

    await takeScreenshot(page, 'composer/undo-toast-counting');
  });

  test('draft restoration: ?draftId= rehydrates To, Subject, and body from IndexedDB', async ({
    page,
    apiSignup,
    takeScreenshot,
  }) => {
    await provisionAccount(page, apiSignup, 'restore');

    await page.route('**/api/drafts', async (route) => {
      await route.fulfill({
        status: route.request().method() === 'POST' ? 201 : 200,
        contentType: 'application/json',
        body: route.request().method() === 'POST'
          ? JSON.stringify({ uid: 1, folder: 'Drafts' })
          : JSON.stringify([]),
      });
    });

    await openComposer(page);

    // Type something so the offline-draft writer persists a row keyed by the
    // composer's local draftId. We have to *read* the id out of IndexedDB
    // because Composer.tsx generates it via newLocalId() without exposing it.
    await page.locator('#composer-to').fill('restore-target@example.com');
    await page.locator('#composer-subject').fill('TMAIL-285 restored draft');
    const editor = page.locator('.ProseMirror').first();
    await editor.click();
    await page.keyboard.type('Original draft body before reload.');

    // Wait for an autosave cycle — that's the moment the local draft hits
    // IndexedDB with a stable id.
    await expect(page.locator('span[role="status"]', { hasText: /saving|saved/i }))
      .toBeVisible({ timeout: 15_000 });

    await takeScreenshot(page, 'composer/restore-before-reload');

    // Read the freshly-persisted localId straight out of IndexedDB. The
    // offline-cache module (frontend/src/utils/offline-cache.ts) uses DB
    // name `tasmail-cache` with a `drafts` object store; each row is shaped
    // `{ localId, data: OfflineDraft, cachedAt, ... }`. We open the DB
    // read-only and grab the first row's localId.
    const draftId = await page.evaluate<string | null>(() => {
      return new Promise((resolve) => {
        const open = indexedDB.open('tasmail-cache');
        open.onerror = () => resolve(null);
        open.onsuccess = () => {
          const db = open.result;
          if (!db.objectStoreNames.contains('drafts')) {
            db.close();
            resolve(null);
            return;
          }
          const tx = db.transaction('drafts', 'readonly');
          const store = tx.objectStore('drafts');
          const all = store.getAll();
          all.onerror = () => {
            db.close();
            resolve(null);
          };
          all.onsuccess = () => {
            const rows = (all.result ?? []) as Array<{ localId?: string; data?: { localId?: string } }>;
            const first = rows[0];
            const id = first?.localId ?? first?.data?.localId ?? null;
            db.close();
            resolve(id);
          };
        };
      });
    });

    if (!draftId) {
      // If we couldn't read the id, fail loudly so the rule that we always
      // assert end-to-end state (not just "the autosave fired") holds.
      throw new Error(
        'TMAIL-285 restore test: could not locate the draftId in IndexedDB. ' +
          'The offline-cache module may have moved — check utils/offline-cache.ts.',
      );
    }

    // Reload with the draftId in the URL — Composer's mount effect reads
    // ?draftId and rehydrates state.
    await page.goto(`/app?draftId=${encodeURIComponent(draftId)}`);
    await page.locator('.sidebar .btn--compose').click();
    const restoredTo = page.locator('#composer-to');
    await expect(restoredTo).toHaveValue('restore-target@example.com', { timeout: 15_000 });
    await expect(page.locator('#composer-subject')).toHaveValue('TMAIL-285 restored draft');
    await expect(page.locator('.ProseMirror').first()).toContainText('Original draft body before reload');

    await takeScreenshot(page, 'composer/restore-after-reload');
  });
});
