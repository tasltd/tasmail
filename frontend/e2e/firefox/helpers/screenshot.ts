// Added: Screenshot helper for Firefox E2E suite (TMAIL-388).
//
// Wraps `page.screenshot()` so every Modern UI spec dumps full-page PNGs into
//   frontend/e2e/screenshots/modern/{feature}/{NN}-{name}.png
// (or the classic/ sibling if the spec lives under firefox/classic/). The
// {feature} segment is inferred from the test info — it's the spec filename
// minus the `.spec.ts` suffix — so individual specs don't have to repeat it.
//
// Disable with E2E_SCREENSHOTS=false (matches the global E2E screenshot rule).
import { test, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// e2e/ directory is two levels up from helpers/.
const E2E_ROOT = path.resolve(__dirname, '..', '..');

const SCREENSHOTS_ENABLED = process.env.E2E_SCREENSHOTS !== 'false';

/**
 * Returns the surface bucket (`modern` or `classic`) inferred from the spec's
 * absolute path. Specs under `firefox/modern/` land in `screenshots/modern/`;
 * specs under `firefox/classic/` land in `screenshots/classic/`.
 */
function inferSurface(specPath: string): 'modern' | 'classic' {
  const normalized = specPath.replace(/\\/g, '/');
  if (normalized.includes('/firefox/classic/')) return 'classic';
  return 'modern';
}

/**
 * Returns the {feature} directory from the spec filename:
 *   /…/firefox/modern/inbox-read.spec.ts → "inbox-read"
 */
function inferFeature(specPath: string): string {
  const base = path.basename(specPath);
  return base.replace(/\.spec\.(ts|tsx|js|mjs|cjs)$/i, '');
}

/**
 * Capture a named, full-page screenshot for the current spec.
 *
 *   await snap(page, '01-login-form');
 *
 * The image lands at
 *   frontend/e2e/screenshots/{modern|classic}/{feature}/01-login-form.png
 *
 * Names should follow the `NN-action` convention so files sort in flow order.
 */
export async function snap(page: Page, name: string): Promise<string | null> {
  if (!SCREENSHOTS_ENABLED) return null;

  // testInfo() is only available inside a running test — that's intentional, the
  // helper is meant to be called from within `test()` blocks. Fail loud if not.
  const info = test.info();
  const surface = inferSurface(info.file);
  const feature = inferFeature(info.file);

  const safeName = name.endsWith('.png') ? name : `${name}.png`;
  const target = path.join(E2E_ROOT, 'screenshots', surface, feature, safeName);

  await page.screenshot({ path: target, fullPage: true });
  return target;
}

/** Exposed for tests that want to write into the same screenshot tree manually. */
export function screenshotDirFor(specFilePath: string): string {
  return path.join(E2E_ROOT, 'screenshots', inferSurface(specFilePath), inferFeature(specFilePath));
}
