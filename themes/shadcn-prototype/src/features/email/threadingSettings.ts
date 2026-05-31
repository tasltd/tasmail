// TMAIL-350: per-folder threading toggle persistence for the alt-UI.
//
// User preference is stored under a single localStorage key as a JSON
// object mapping folder name → boolean. Keeping it folder-scoped matches
// Gmail and Apple Mail — most users want threading on for personal inboxes
// but off for mailing list folders / Sent — and survives across sessions
// without a backend round trip.
//
// Why not a useState in EmailClient? Because reloading the SPA would
// reset to the global default and lose the per-folder choice. Why not in
// the backend user_preferences table? Because the alt-UI ships against
// the same backend as the classic UI, and we don't want to spawn a
// migration + handler pair for a UI-only preference. localStorage is the
// right tier (cf. the scalability rule — pick the tier that scales for
// the feature's actual blast radius).
//
// Pure module — no React. Hooks layer is in EmailClient.tsx.

const STORAGE_KEY = 'tmail.modernui.threadingByFolder';

/** Default threading state when a folder has never been toggled. We
 *  default to ON so first-time users see the conversation grouping
 *  (the whole point of TMAIL-350 was to *add* threading; opting out
 *  is the deviation, not the default). */
const DEFAULT_THREADED = true;

type ThreadingMap = Record<string, boolean>;

function readMap(): ThreadingMap {
  if (typeof window === 'undefined' || !window.localStorage) return {};
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed as ThreadingMap;
    }
    return {};
  } catch {
    // Corrupt JSON / quota error — wipe and start fresh rather than
    // surface an unrecoverable error to the user.
    return {};
  }
}

function writeMap(map: ThreadingMap) {
  if (typeof window === 'undefined' || !window.localStorage) return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(map));
  } catch {
    // localStorage may be disabled (private mode on some browsers) or
    // over quota. Silently fail — the toggle still works for the
    // current session via the EmailClient state, just won't persist.
  }
}

/** Get the threading flag for a folder. Falls back to the default
 *  (currently ON) when the folder has never been toggled. */
export function getThreadedForFolder(folder: string): boolean {
  const map = readMap();
  if (Object.prototype.hasOwnProperty.call(map, folder)) {
    return !!map[folder];
  }
  return DEFAULT_THREADED;
}

/** Persist the threading flag for a folder. Passing `undefined` removes
 *  the entry so the folder reverts to the default on next read. */
export function setThreadedForFolder(folder: string, value: boolean | undefined) {
  const map = readMap();
  if (value === undefined) {
    delete map[folder];
  } else {
    map[folder] = value;
  }
  writeMap(map);
}
