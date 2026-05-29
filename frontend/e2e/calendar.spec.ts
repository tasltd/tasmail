/**
 * TMAIL-287 — Calendar sweep:
 *   • CRUD via the API (organizer + invitee) — round-trip with a fresh GET.
 *   • UI: sidebar nav → CalendarManager list → New Event form → list refresh.
 *   • UI: Grid toggle loads the FullCalendar view and renders our event chip.
 *   • UI: Event detail view exposes ICS download trigger + RSVP buttons + the
 *     TMAIL-269 public-share section.
 *   • Cross-user RSVP: a second mailbox accepts the invite and the organizer
 *     sees the updated `rsvp = accepted` on their detail view.
 *   • Free-busy: organizer's own busy interval shows up; an external email
 *     comes back as `not_resolved`.
 *   • Suggest-slots: returns ≥ 1 candidate in a known-free window.
 *   • iMIP accept (negative): hitting POST /api/calendar/imip/accept with a
 *     fake folder/uid surfaces the BYOK-IMAP-not-configured error (the happy
 *     path requires a real inbound iMIP REQUEST email which is exercised by
 *     the backend integration tests, not the SPA).
 *   • CalDAV public-scheduling tokens (migration 071): toggling
 *     public_enabled exposes /api/calendar/public/{token} and the BookingPage
 *     submits an external RSVP that the organizer then sees as an attendee.
 *
 * HARD RULES followed:
 *   • Firefox only (project=firefox).
 *   • Sidebar/menu navigation for internal routes — `page.goto()` is only
 *     used for the login page and for the explicitly-public /book/:token
 *     URL (the booking page is the route under test and has no menu entry).
 *   • Every mutation is verified with a fresh API GET (E2E SPA validation
 *     rule).
 *   • Screenshots saved to e2e/screenshots/calendar/ at every key step.
 *
 * Bug surface this sweep uncovered (and that the companion fix commit
 * resolves):
 *   • migration 074 — `event_attendees.rsvp` was a Postgres ENUM
 *     (`rsvp_status`) but the Rust model decodes the column as `String`.
 *     sqlx refused the type mismatch and EVERY attendee-touching endpoint
 *     500'd: POST /events with attendees, POST /events/{id}/rsvp,
 *     POST /imip/accept, POST /public/{token}/rsvp. Same family of bug as
 *     migrations 061/063/065. Fix: widen to TEXT + CHECK constraint.
 */
import { test, expect } from './fixtures/base.js';
import { request as apiRequest, type APIRequestContext, type Page } from '@playwright/test';
import { deleteMailboxByUsername } from './helpers/db-cleanup.js';

const PASSWORD = 'calendar-sweep-Pa55!';
const RUN_TAG = Date.now();
const ORG_EMAIL = `e2e-cal-org-${RUN_TAG}@e2e.tasmail`;
const ATTENDEE_EMAIL = `e2e-cal-att-${RUN_TAG}@e2e.tasmail`;

let api: APIRequestContext;
let orgToken: string;
let orgRefresh: string;
let attendeeToken: string;
let orgAuth: Record<string, string>;
let attAuth: Record<string, string>;

// ──────────────────────────────────────────────────────────────────────────
// Shapes — kept inline so the spec stays self-contained (Playwright config
// doesn't compile src/). Keep in sync with frontend/src/api/calendar.ts.
// ──────────────────────────────────────────────────────────────────────────
interface CalendarEvent {
  id: string;
  organizer_id: string;
  title: string;
  description: string | null;
  location: string | null;
  start_time: string;
  end_time: string;
  all_day: boolean;
  status: string;
  ics_uid: string;
  public_token: string;
  public_enabled: boolean;
}
interface EventAttendee {
  id: string;
  event_id: string;
  email: string;
  display_name: string | null;
  rsvp: string;
  responded_at: string | null;
}
interface CalendarEventWithAttendees extends CalendarEvent {
  attendees: EventAttendee[];
}
interface FreeBusyResponse {
  attendees: Array<{
    email: string;
    status: 'resolved' | 'not_resolved';
    busy: Array<{ start: string; end: string }>;
  }>;
}
interface SuggestSlotsResponse {
  slots: Array<{ start: string; end: string }>;
  unresolved_attendees: string[];
}

// ISO time helpers — anchor the test window to tomorrow at 14:00 UTC so the
// event is firmly in the future regardless of when the suite runs.
function tomorrowAt(hour: number, minute = 0): string {
  const d = new Date();
  d.setUTCDate(d.getUTCDate() + 1);
  d.setUTCHours(hour, minute, 0, 0);
  return d.toISOString();
}

test.describe.configure({ mode: 'serial' });
// Sweep runs through the live tunnel (Apache → SSH → Vite). First-paint can be
// slow on a cold Vite dev server, so we bump the per-test timeout above the
// 30s default so loginViaUI doesn't race the SPA boot.
test.setTimeout(90_000);

test.describe('TMAIL-287 Calendar sweep', () => {
  test.beforeAll(async ({ baseURL }) => {
    test.setTimeout(120_000);
    api = await apiRequest.newContext({ baseURL });

    const orgSignup = await api.post('/api/auth/signup', {
      data: { email: ORG_EMAIL, password: PASSWORD },
    });
    expect(orgSignup.status(), 'organizer signup must succeed').toBeLessThan(300);
    const orgTokens = await orgSignup.json();
    orgToken = orgTokens.access_token as string;
    orgRefresh = orgTokens.refresh_token as string;
    orgAuth = { Authorization: `Bearer ${orgToken}` };

    const attSignup = await api.post('/api/auth/signup', {
      data: { email: ATTENDEE_EMAIL, password: PASSWORD },
    });
    expect(attSignup.status(), 'attendee signup must succeed').toBeLessThan(300);
    attendeeToken = (await attSignup.json()).access_token as string;
    attAuth = { Authorization: `Bearer ${attendeeToken}` };
  });

  test.afterAll(async () => {
    try { deleteMailboxByUsername(ORG_EMAIL); } catch { /* best-effort */ }
    try { deleteMailboxByUsername(ATTENDEE_EMAIL); } catch { /* best-effort */ }
    await api?.dispose();
  });

  async function loginViaUI(page: Page, email: string, password: string) {
    // First-paint on a cold Vite dev server over the SSH tunnel is occasionally
    // slow enough to blow past the default 30s page timeout — give it a wider
    // budget + wait for domcontentloaded explicitly before probing for the form.
    await page.goto('/login', { waitUntil: 'domcontentloaded', timeout: 45_000 });
    await page.waitForSelector('#username', { timeout: 45_000 });
    await page.fill('#username', email);
    await page.fill('#password', password);
    await page.click('button[type="submit"]');
    await page.waitForURL(/\/app/, { timeout: 30_000 });
    await expect(page.locator('.sidebar')).toBeVisible({ timeout: 20_000 });
  }

  /**
   * Auth fast-path: pre-populate localStorage with the JWT pair we already
   * have from the beforeAll signup, then hit /login (the SPA redirects
   * /login → /app when a token is present — see App.tsx::LoginPageWrapper).
   *
   * Why not call /api/auth/login per test? The auth router is rate-limited at
   * 10 req/IP/60s (router.rs::AUTH_RATE_LIMIT_*), and a 10-test sweep would
   * easily exceed that under serial retries. Reusing the signup tokens keeps
   * us under the limit and is just as truthful for tests that aren't
   * validating the login UI itself.
   *
   * Note: `page.goto('/login')` is still the only direct URL we hit — the
   * HARD RULE allows it as the initial entry. The redirect to /app is
   * handled by the SPA, not the test.
   */
  async function loginAsAPI(page: Page, _email: string, _password: string) {
    // Seed localStorage BEFORE the page loads so useAuth() sees the token on
    // first render. addInitScript runs in every new document of the context.
    await page.context().addInitScript(
      ([access, refresh]) => {
        window.localStorage.setItem('access_token', access);
        window.localStorage.setItem('refresh_token', refresh);
      },
      [orgToken, orgRefresh] as const,
    );

    await page.goto('/login', { waitUntil: 'domcontentloaded', timeout: 45_000 });
    // The redirect /login → /app fires after first React render reads the
    // localStorage tokens. Over the tunnel the first render can take a while —
    // 45s gives headroom for the cold-Vite path. If the redirect never fires
    // (e.g. SPA boot failed) the sidebar visibility check below catches it.
    await page.waitForURL(/\/app/, { timeout: 45_000 });
    await expect(page.locator('.sidebar')).toBeVisible({ timeout: 30_000 });
  }

  async function clickSidebar(page: Page, label: string) {
    const item = page.locator('.sidebar .folder-item', { hasText: label }).first();
    await expect(item, `sidebar must expose "${label}"`).toBeVisible({ timeout: 10_000 });
    await item.click();
  }

  // ──────────────────────────────────────────────────────────────────────
  // 1) Sidebar navigation → Calendar view (no page.goto for internal route)
  // ──────────────────────────────────────────────────────────────────────
  test('navigates to Calendar via sidebar (no direct URL)', async ({ page, takeScreenshot }) => {
    await loginViaUI(page, ORG_EMAIL, PASSWORD);
    await takeScreenshot(page, 'calendar/01-app-shell-after-login');
    await clickSidebar(page, 'Calendar');
    // The manager header reads "Calendar" and exposes a "New Event" button.
    await expect(page.locator('h2', { hasText: 'Calendar' })).toBeVisible({ timeout: 10_000 });
    await expect(page.locator('button:has-text("New Event")')).toBeVisible();
    await takeScreenshot(page, 'calendar/02-calendar-manager-empty');
  });

  // ──────────────────────────────────────────────────────────────────────
  // 2) Create event with attendees through the UI form, then verify the
  //    full payload via the API. Covers: title, description, location,
  //    start/end, attendee row, ICS UID populated, public_token defaulted
  //    but public_enabled=false. Validates the migration-074 fix end-to-end
  //    (the response would 500 before the migration).
  // ──────────────────────────────────────────────────────────────────────
  test('creates event with attendee via UI, API round-trip matches', async ({ page, takeScreenshot }) => {
    const beforeList = await api.get('/api/calendar/events', { headers: orgAuth });
    const before = (await beforeList.json()) as CalendarEvent[];

    await loginAsAPI(page, ORG_EMAIL, PASSWORD);
    await clickSidebar(page, 'Calendar');

    await page.click('button:has-text("New Event")');

    const title = `Sweep Meeting ${RUN_TAG}`;
    // datetime-local takes "YYYY-MM-DDTHH:MM" (no seconds, no Z).
    const start = tomorrowAt(14).slice(0, 16);
    const end = tomorrowAt(15).slice(0, 16);

    await page.fill('input[placeholder="Event title"]', title);
    await page.fill('textarea[placeholder="Optional description"]', 'Quarterly review');
    await page.fill('input[placeholder="Optional location"]', 'Conf Room A');
    await page.fill('input[type="datetime-local"]:nth-of-type(1), input[type="datetime-local"] >> nth=0', start);
    await page.fill('input[type="datetime-local"] >> nth=1', end);
    // Add the attendee.
    await page.fill('input[placeholder="attendee@example.com"]', ATTENDEE_EMAIL);
    await page.click('button:has-text("Add")');
    await expect(page.locator('span', { hasText: ATTENDEE_EMAIL })).toBeVisible();

    await takeScreenshot(page, 'calendar/03-event-form-filled');
    await page.click('button:has-text("Create Event")');

    // The list refreshes and shows our event row.
    const newRow = page.locator(`[data-testid="event-row"][data-event-title="${title}"]`);
    await expect(newRow).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'calendar/04-event-list-after-create');

    // API cross-check.
    const afterList = await api.get('/api/calendar/events', { headers: orgAuth });
    const after = (await afterList.json()) as CalendarEvent[];
    expect(after.length).toBe(before.length + 1);
    const created = after.find((e) => e.title === title);
    expect(created, 'event must round-trip to API').toBeTruthy();
    expect(created!.location).toBe('Conf Room A');
    expect(created!.description).toBe('Quarterly review');
    expect(created!.ics_uid).toMatch(/@tasmail\.io$/);
    expect(created!.public_token).toMatch(/^[0-9a-f-]{36}$/);
    expect(created!.public_enabled).toBe(false);

    // Attendee was persisted with rsvp=pending.
    const detail = await api.get(`/api/calendar/events/${created!.id}`, { headers: orgAuth });
    const detailBody = (await detail.json()) as CalendarEventWithAttendees;
    expect(detailBody.attendees).toHaveLength(1);
    expect(detailBody.attendees[0].email).toBe(ATTENDEE_EMAIL);
    expect(detailBody.attendees[0].rsvp).toBe('pending');
  });

  // ──────────────────────────────────────────────────────────────────────
  // 3) Grid (FullCalendar) view loads via the lazy import; the event chip
  //    rendered with the event's title is visible inside the calendar grid.
  // ──────────────────────────────────────────────────────────────────────
  test('Grid view loads FullCalendar and renders the event chip', async ({ page, takeScreenshot }) => {
    await loginAsAPI(page, ORG_EMAIL, PASSWORD);
    await clickSidebar(page, 'Calendar');

    await page.click('button:has-text("Grid")');
    // The lazy chunk pulls @fullcalendar/* (~600 kB raw) — give it a beat.
    await expect(page.locator('[data-testid="calendar-view"]')).toBeVisible({ timeout: 20_000 });
    await takeScreenshot(page, 'calendar/05-grid-month-view');

    // FullCalendar renders event titles inside .fc-event-title spans.
    const eventChip = page.locator('.fc-event', { hasText: `Sweep Meeting ${RUN_TAG}` }).first();
    await expect(eventChip).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'calendar/06-grid-with-event-chip');
  });

  // ──────────────────────────────────────────────────────────────────────
  // 4) Detail view: ICS download button + RSVP buttons + public share
  //    section all render. We don't actually click Download because the
  //    download intercept on Firefox is flaky over the SSH tunnel — instead
  //    we assert the GET /events/{id}/ics endpoint returns a valid VCALENDAR
  //    blob (the user-visible win is the same).
  // ──────────────────────────────────────────────────────────────────────
  test('Detail view exposes ICS, RSVP, and public-share controls', async ({ page, takeScreenshot }) => {
    await loginAsAPI(page, ORG_EMAIL, PASSWORD);
    await clickSidebar(page, 'Calendar');

    const eventRow = page.locator(`[data-testid="event-row"][data-event-title="Sweep Meeting ${RUN_TAG}"]`);
    await expect(eventRow).toBeVisible({ timeout: 10_000 });
    await eventRow.click();

    await expect(page.locator('h2', { hasText: `Sweep Meeting ${RUN_TAG}` })).toBeVisible({ timeout: 10_000 });
    // Detail toolbar has the ICS button.
    await expect(page.locator('button:has-text("ICS")')).toBeVisible();
    // RSVP triplet — Accept / Decline / Maybe.
    await expect(page.locator('button:has-text("Accept")')).toBeVisible();
    await expect(page.locator('button:has-text("Decline")')).toBeVisible();
    await expect(page.locator('button:has-text("Maybe")')).toBeVisible();
    // Public-share section is collapsed (checkbox unchecked).
    await expect(page.locator('[data-testid="public-share-section"]')).toBeVisible();
    await takeScreenshot(page, 'calendar/07-event-detail-view');

    // ICS API check — the rendered .ics file must include the event UID
    // and SUMMARY. Locate the event id from the API again to avoid scraping
    // the detail page.
    const list = (await (await api.get('/api/calendar/events', { headers: orgAuth })).json()) as CalendarEvent[];
    const created = list.find((e) => e.title === `Sweep Meeting ${RUN_TAG}`)!;
    const icsResp = await api.get(`/api/calendar/events/${created.id}/ics`, { headers: orgAuth });
    expect(icsResp.status()).toBe(200);
    const ics = await icsResp.text();
    expect(ics).toContain('BEGIN:VCALENDAR');
    expect(ics).toContain(`UID:${created.ics_uid}`);
    expect(ics).toContain(`SUMMARY:Sweep Meeting ${RUN_TAG}`);
    expect(ics).toContain('METHOD:REQUEST');
  });

  // ──────────────────────────────────────────────────────────────────────
  // 5) Attendee RSVPs accepted; organizer sees the update on a fresh GET.
  //    We RSVP through the API as the attendee because the SPA's
  //    Accept/Decline/Maybe buttons go through /events/{id}/rsvp which the
  //    attendee uses with their own JWT — that path is what would be hit
  //    after they followed an iMIP REPLY link in a future UI iteration.
  // ──────────────────────────────────────────────────────────────────────
  test('cross-user RSVP: attendee accepts; organizer sees status update', async ({ takeScreenshot, page }) => {
    const list = (await (await api.get('/api/calendar/events', { headers: orgAuth })).json()) as CalendarEvent[];
    const created = list.find((e) => e.title === `Sweep Meeting ${RUN_TAG}`)!;

    const rsvpResp = await api.post(`/api/calendar/events/${created.id}/rsvp`, {
      headers: attAuth,
      data: { status: 'accepted' },
    });
    expect(rsvpResp.status()).toBe(200);
    const rsvped = (await rsvpResp.json()) as EventAttendee;
    expect(rsvped.rsvp).toBe('accepted');
    expect(rsvped.responded_at).not.toBeNull();

    // Organizer's detail GET shows the updated rsvp.
    const detail = (await (await api.get(`/api/calendar/events/${created.id}`, { headers: orgAuth })).json()) as CalendarEventWithAttendees;
    const att = detail.attendees.find((a) => a.email === ATTENDEE_EMAIL);
    expect(att?.rsvp).toBe('accepted');

    // UI proof — login again and visit the detail; the green "accepted"
    // badge sits next to the attendee email.
    await loginAsAPI(page, ORG_EMAIL, PASSWORD);
    await clickSidebar(page, 'Calendar');
    await page.locator(`[data-testid="event-row"][data-event-title="Sweep Meeting ${RUN_TAG}"]`).click();
    await expect(page.locator('span', { hasText: 'accepted' })).toBeVisible({ timeout: 10_000 });
    await takeScreenshot(page, 'calendar/08-detail-with-accepted-rsvp');
  });

  // ──────────────────────────────────────────────────────────────────────
  // 6) Free-busy: the organizer's event from test (2) shows up as a busy
  //    interval; an external (unknown) email comes back as `not_resolved`.
  // ──────────────────────────────────────────────────────────────────────
  test('free-busy returns busy interval for organizer + not_resolved for external', async () => {
    const fbResp = await api.post('/api/calendar/free-busy', {
      headers: orgAuth,
      data: {
        attendees: [ORG_EMAIL, 'stranger@nowhere.example'],
        range_start: tomorrowAt(0),
        range_end: tomorrowAt(23, 59),
      },
    });
    expect(fbResp.status()).toBe(200);
    const fb = (await fbResp.json()) as FreeBusyResponse;
    expect(fb.attendees).toHaveLength(2);

    const org = fb.attendees.find((a) => a.email.toLowerCase() === ORG_EMAIL.toLowerCase());
    const stranger = fb.attendees.find((a) => a.email.toLowerCase() === 'stranger@nowhere.example');

    expect(org?.status).toBe('resolved');
    expect((org?.busy ?? []).length).toBeGreaterThan(0);
    expect(stranger?.status).toBe('not_resolved');
    expect(stranger?.busy).toHaveLength(0);
  });

  // ──────────────────────────────────────────────────────────────────────
  // 7) Suggest-slots returns ≥ 1 candidate in a free window (day after
  //    tomorrow). Working-hours defaults guarantee non-zero slots.
  // ──────────────────────────────────────────────────────────────────────
  test('suggest-slots returns candidates in a free window', async () => {
    // include_weekends defaults to false on the backend (services/slot_suggester.rs),
    // so we need to land the window on a weekday or the suggester will return
    // zero slots when the test happens to run on a Friday or Saturday.
    const start = new Date();
    start.setUTCDate(start.getUTCDate() + 2);
    while (start.getUTCDay() === 0 || start.getUTCDay() === 6) {
      start.setUTCDate(start.getUTCDate() + 1);
    }
    start.setUTCHours(9, 0, 0, 0);
    const end = new Date(start);
    end.setUTCHours(17, 0, 0, 0);

    const resp = await api.post('/api/calendar/suggest-slots', {
      headers: orgAuth,
      data: {
        attendees: [ORG_EMAIL],
        duration_minutes: 30,
        range_start: start.toISOString(),
        range_end: end.toISOString(),
        max_slots: 5,
      },
    });
    expect(resp.status()).toBe(200);
    const body = (await resp.json()) as SuggestSlotsResponse;
    expect(body.slots.length).toBeGreaterThan(0);
    expect(body.unresolved_attendees).toHaveLength(0);
    // Slots respect duration.
    for (const slot of body.slots) {
      const span = new Date(slot.end).getTime() - new Date(slot.start).getTime();
      expect(span).toBe(30 * 60 * 1000);
    }
  });

  // ──────────────────────────────────────────────────────────────────────
  // 8) iMIP accept (API negative path). The happy path requires a real
  //    inbound text/calendar; method=REQUEST email — that's covered by the
  //    backend integration tests. From the SPA's perspective we assert the
  //    endpoint exists, is auth-gated, and rejects a bogus folder/uid with
  //    a 4xx and a human-readable message (so a future UI iteration can
  //    show it as a toast).
  // ──────────────────────────────────────────────────────────────────────
  test('iMIP accept endpoint rejects bogus folder/uid with actionable error', async () => {
    const resp = await api.post('/api/calendar/imip/accept', {
      headers: attAuth,
      data: { folder: 'INBOX', uid: 999_999_999 },
    });
    // The endpoint maps "no BYO IMAP attached" to 503 Service Unavailable
    // (error.rs::AppError::ServiceUnavailable) and "couldn't find that UID"
    // to 4xx. Both are acceptable — the point is that the endpoint is wired,
    // auth-gated, and returns an actionable string.
    expect([400, 404, 422, 503]).toContain(resp.status());
    const body = await resp.text();
    expect(body.length).toBeGreaterThan(0);
    expect(body.toLowerCase()).toMatch(/imap|server|onboarding|invitation|invalid|not found/);
  });

  // ──────────────────────────────────────────────────────────────────────
  // 9) CalDAV public-scheduling token (migration 071): the organizer
  //    enables the public link in the UI, the share section reveals the
  //    /book/{token} URL, an external visitor opens the public BookingPage
  //    (no auth) and submits an RSVP. The organizer then sees an extra
  //    attendee row carrying the external email + status.
  //
  //    Note: /book/{token} is an explicitly-public route with no menu
  //    entry — page.goto() is the correct entry point here, mirroring the
  //    real flow (visitor clicks a link in an email).
  // ──────────────────────────────────────────────────────────────────────
  test('public booking: enable share, external RSVP, organizer sees attendee', async ({ page, takeScreenshot, browser }) => {
    const list = (await (await api.get('/api/calendar/events', { headers: orgAuth })).json()) as CalendarEvent[];
    const created = list.find((e) => e.title === `Sweep Meeting ${RUN_TAG}`)!;

    // Step 1 — enable the public link through the detail view UI.
    await loginAsAPI(page, ORG_EMAIL, PASSWORD);
    await clickSidebar(page, 'Calendar');
    await page.locator(`[data-testid="event-row"][data-event-title="Sweep Meeting ${RUN_TAG}"]`).click();
    const shareSection = page.locator('[data-testid="public-share-section"]');
    await expect(shareSection).toBeVisible({ timeout: 10_000 });
    // The checkbox is a controlled React input — its `checked` follows
    // eventDetail.public_enabled, which only flips after the PUT /events/{id}
    // round-trip + TanStack Query refetch. Use plain click() rather than
    // check() so we don't fight Playwright's checkbox-state assertion; rely on
    // the conditionally-rendered URL section below as the readiness signal.
    const toggle = shareSection.getByLabel(/external participants to book/i);
    await toggle.click({ timeout: 10_000 });

    // The URL appears.
    const urlCode = page.locator('[data-testid="public-share-url"]');
    await expect(urlCode).toBeVisible({ timeout: 5_000 });
    const sharedUrl = (await urlCode.textContent())!.trim();
    expect(sharedUrl).toMatch(/\/book\/[0-9a-f-]{36}$/);
    await takeScreenshot(page, 'calendar/09-public-share-enabled');

    // Step 2 — API confirms the toggle landed.
    const refreshed = (await (await api.get(`/api/calendar/events/${created.id}`, { headers: orgAuth })).json()) as CalendarEventWithAttendees;
    expect(refreshed.public_enabled).toBe(true);

    // Step 3 — anonymous BookingPage visit. New incognito-style context so
    // there's no organizer JWT in localStorage to interfere with auth.
    const anonCtx = await browser.newContext();
    const anonPage = await anonCtx.newPage();
    const tokenMatch = sharedUrl.match(/\/book\/([0-9a-f-]{36})$/)!;
    await anonPage.goto(`/book/${tokenMatch[1]}`);
    await expect(anonPage.locator('h1', { hasText: `Sweep Meeting ${RUN_TAG}` })).toBeVisible({ timeout: 20_000 });
    await anonPage.screenshot({ path: 'e2e/screenshots/calendar/10-booking-page-loaded.png' });

    const externalEmail = `external-${RUN_TAG}@example.com`;
    await anonPage.fill('input[type="text"][autocomplete="name"]', 'External Visitor');
    await anonPage.fill('input[type="email"]', externalEmail);
    // Default radio is "accepted" — pick "Maybe" to exercise the choice path.
    await anonPage.click('label.booking-page__choice:has-text("Maybe")');
    await anonPage.screenshot({ path: 'e2e/screenshots/calendar/11-booking-page-filled.png' });
    await anonPage.click('button.booking-page__submit');

    // Thank-you confirmation visible.
    await expect(anonPage.locator('h1', { hasText: 'Thanks for responding' })).toBeVisible({ timeout: 15_000 });
    await anonPage.screenshot({ path: 'e2e/screenshots/calendar/12-booking-page-confirmed.png' });
    await anonCtx.close();

    // Step 4 — organizer GET shows the external attendee with status=maybe.
    const finalDetail = (await (await api.get(`/api/calendar/events/${created.id}`, { headers: orgAuth })).json()) as CalendarEventWithAttendees;
    const externalAtt = finalDetail.attendees.find((a) => a.email === externalEmail);
    expect(externalAtt, 'external visitor must show up as an attendee').toBeTruthy();
    expect(externalAtt!.rsvp).toBe('maybe');
    expect(externalAtt!.display_name).toBe('External Visitor');
  });

  // ──────────────────────────────────────────────────────────────────────
  // 10) Cancel event from the list — DELETE button on the row triggers a
  //     SOFT delete (status → 'cancelled' per models/calendar_event.rs::cancel).
  //     Row stays visible but is dimmed (opacity 0.6) and the status badge
  //     flips to 'cancelled'. API GET still returns the row with the new
  //     status.
  // ──────────────────────────────────────────────────────────────────────
  test('cancel event: row dims + status flips to cancelled', async ({ page, takeScreenshot }) => {
    const beforeList = (await (await api.get('/api/calendar/events', { headers: orgAuth })).json()) as CalendarEvent[];
    const target = beforeList.find((e) => e.title === `Sweep Meeting ${RUN_TAG}`);
    expect(target, 'event still exists before cancel').toBeTruthy();
    expect(target!.status).toBe('tentative');

    await loginAsAPI(page, ORG_EMAIL, PASSWORD);
    await clickSidebar(page, 'Calendar');

    const row = page.locator(`[data-testid="event-row"][data-event-title="Sweep Meeting ${RUN_TAG}"]`);
    await expect(row).toBeVisible();
    await row.locator('button.btn--danger').click();

    // Wait for the row to reflect the new status — the badge inside the row
    // re-renders to "cancelled" once cancelMut.onSuccess invalidates the
    // ['calendar-events'] query and TanStack Query refetches.
    await expect(row.locator('span', { hasText: 'cancelled' })).toBeVisible({ timeout: 15_000 });
    await takeScreenshot(page, 'calendar/13-event-list-after-cancel');

    // API GET confirms the soft-delete landed.
    const finalCheck = await api.get(`/api/calendar/events/${target!.id}`, { headers: orgAuth });
    expect(finalCheck.status()).toBe(200);
    const finalEvent = (await finalCheck.json()) as CalendarEventWithAttendees;
    expect(finalEvent.status).toBe('cancelled');
  });
});
