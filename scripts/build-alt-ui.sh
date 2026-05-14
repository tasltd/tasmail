#!/usr/bin/env bash
# TMAIL-222: build the shadcn alt-UI prototype and drop its bundle into
# frontend/public/modern/ so the production Vite host (and the live
# tunnel/Apache vhost) serve it as static files at /modern/.
#
# Re-run whenever themes/shadcn-prototype/ changes. Idempotent — wipes the
# previous output before copying.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$REPO_ROOT/themes/shadcn-prototype"
DEST="$REPO_ROOT/frontend/public/modern"

if [[ ! -d "$SRC" ]]; then
  echo "Alt-UI source not found at $SRC" >&2
  exit 1
fi

echo "[1/3] Installing alt-UI deps (idempotent if up to date)…"
( cd "$SRC" && npm install --no-audit --no-fund --silent )

echo "[2/3] Building alt-UI…"
( cd "$SRC" && npm run build )

echo "[3/3] Replacing $DEST with the freshly built bundle…"
rm -rf "$DEST"
mkdir -p "$DEST"
cp -r "$SRC/dist/." "$DEST/"

echo "Done. Alt-UI is now served at /modern/ from frontend/public/modern/"
ls -la "$DEST" | head
