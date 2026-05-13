#!/usr/bin/env node
/**
 * TMAIL-188 — refresh the nlm CLI auth cookies via Playwright/Firefox.
 *
 * Why Firefox?
 *   The nlm CLI is happy with any browser's cookies as long as the SAPISID /
 *   __Secure-1PSID family lands in the file. Firefox is the user's preferred
 *   browser per the workspace HARD RULE on E2E, so we use it here too.
 *
 * Flow:
 *   1. Launch a *headed* Firefox window pointed at notebooklm.google.com
 *   2. Wait for the user to complete Google login + 2FA interactively
 *   3. As soon as the SPA's notebook list endpoint loads (any cookie containing
 *      __Secure-1PSID is present) we know auth is good
 *   4. Dump the full cookie jar in HTTP `name=value; name=value` form to
 *      ~/.nlm/cookies.txt and run `nlm login --check` to confirm
 *
 * Run with:
 *   node scripts/notebooklm-login-firefox.mjs
 *
 * Override the cookies output path via NLM_COOKIES_PATH if needed.
 */
import { firefox } from '/home/ddr/Documents/code/project-email-service/frontend/node_modules/playwright/index.mjs';
import { writeFileSync, mkdirSync } from 'fs';
import { dirname } from 'path';
import { execFileSync } from 'child_process';
import { homedir } from 'os';

const COOKIES_PATH = process.env.NLM_COOKIES_PATH ?? `${homedir()}/.nlm/cookies.txt`;
const TARGET = 'https://notebooklm.google.com/';
const READY_COOKIE = '__Secure-1PSID';
const TIMEOUT_MS = 5 * 60_000;

function log(line) { console.log(`[nlm-login] ${line}`); }

const browser = await firefox.launch({ headless: false });
const ctx = await browser.newContext();
const page = await ctx.newPage();

log(`Opening ${TARGET} — complete Google login + 2FA in the Firefox window.`);
await page.goto(TARGET, { waitUntil: 'domcontentloaded' });

const start = Date.now();
let cookies = [];
while (Date.now() - start < TIMEOUT_MS) {
  await page.waitForTimeout(2000);
  cookies = await ctx.cookies();
  const hasAuth = cookies.some((c) => c.name === READY_COOKIE);
  if (hasAuth) break;
  process.stdout.write('.');
}
process.stdout.write('\n');

if (!cookies.some((c) => c.name === READY_COOKIE)) {
  log(`No ${READY_COOKIE} cookie after ${TIMEOUT_MS / 1000}s — login probably failed. Aborting.`);
  await browser.close();
  process.exit(1);
}

// Filter to .google.com cookies (matches what nlm expects). Format the file as
// the CLI's manual mode wants: a single line of `name=value; name=value; …`.
const googleCookies = cookies.filter((c) => /\.?google\.com$/i.test(c.domain));
const cookieHeader = googleCookies.map((c) => `${c.name}=${c.value}`).join('; ');

mkdirSync(dirname(COOKIES_PATH), { recursive: true });
writeFileSync(COOKIES_PATH, cookieHeader);
log(`Wrote ${googleCookies.length} cookies to ${COOKIES_PATH} (${cookieHeader.length} chars).`);

await browser.close();

// Hand the cookies to nlm and verify. execFileSync prevents shell injection on the path.
log(`Running nlm login --manual --file ${COOKIES_PATH} …`);
try {
  execFileSync('nlm', ['login', '--manual', '--file', COOKIES_PATH], { stdio: 'inherit' });
} catch (e) {
  log(`nlm login exited non-zero: ${e?.message ?? e}`);
}

log('Verifying with `nlm login --check` …');
const check = execFileSync('nlm', ['login', '--check'], { stdio: 'pipe' }).toString();
console.log(check);
if (/Authenticated/i.test(check) || /valid/i.test(check)) {
  log('nlm auth refreshed.');
  process.exit(0);
} else {
  log('nlm auth check did not confirm — inspect output above.');
  process.exit(2);
}
