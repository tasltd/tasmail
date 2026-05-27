#!/usr/bin/env bash
# TMAIL-192 — set up the "TASMail Brand" NotebookLM notebook, upload the brand
# spec + every SVG variant, ask NotebookLM to critique contrast, kerning, and
# simplification opportunities, and persist the response for review.
#
# Prerequisite: a fresh `nlm` session. If auth is stale, this script aborts
# with a clear message pointing at the Firefox login helper (TMAIL-188) —
# that step has to be run interactively on a desktop session because Google
# 2FA can't be completed headlessly. Once auth is fresh, this script finishes
# the rest of TMAIL-192's NotebookLM-side work in one go.
#
# After the critique markdown lands at docs/research/brand-notebooklm-critique.md,
# a human (or follow-up agent) reviews it, edits branding/src/build_logo.py to
# apply chosen iterations, then re-runs build_logo.py and build_assets.py to
# regenerate rasters.
#
# Usage:
#   scripts/tasmail-brand-critique.sh                    # idempotent: re-uses
#                                                          existing notebook
#                                                          if found
#   scripts/tasmail-brand-critique.sh --notebook NB_ID   # force a specific id
#   scripts/tasmail-brand-critique.sh --dry-run          # show plan, no nlm calls
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

NOTEBOOK_TITLE="TASMail Brand"
OUTPUT_DIR="docs/research"
OUTPUT_FILE="$OUTPUT_DIR/brand-notebooklm-critique.md"
NOTEBOOK_ID=""
DRY_RUN=0

while (($#)); do
    case "$1" in
        --notebook) NOTEBOOK_ID="$2"; shift 2 ;;
        --dry-run)  DRY_RUN=1; shift ;;
        -h|--help)
            sed -n '2,22p' "$0"
            exit 0 ;;
        *)  echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

log() { printf '[brand-critique] %s\n' "$*"; }
die() { printf '[brand-critique] ERROR: %s\n' "$*" >&2; exit 1; }

command -v nlm >/dev/null || die "nlm CLI not found on PATH (~/.local/bin/nlm)."

# --- 1. auth check -----------------------------------------------------------
log "Checking nlm auth..."
if ! nlm login --check >/dev/null 2>&1; then
    cat >&2 <<'EOF'
[brand-critique] ERROR: nlm authentication is stale.

NotebookLM sessions last ~20 min and Google login requires interactive 2FA, so
this cannot be refreshed from a headless auto-fix container. To unblock:

  1. Open a desktop/VNC session on tas-src-1 (or any workstation with DISPLAY)
  2. Run: node scripts/notebooklm-login-firefox.mjs
  3. Complete Google login + 2FA in the Firefox window the script opens
  4. Re-run this script — it will pick up from here.

See TMAIL-188 for context on the login helper.
EOF
    exit 3
fi
log "Auth OK."

# --- 2. resolve or create notebook ------------------------------------------
if [[ -z "$NOTEBOOK_ID" ]]; then
    log "Looking for existing notebook '$NOTEBOOK_TITLE'..."
    NOTEBOOK_ID="$(nlm notebook list --json 2>/dev/null \
        | python3 -c "
import sys, json
title = '''$NOTEBOOK_TITLE'''.strip().lower()
for nb in json.load(sys.stdin):
    if nb.get('title', '').strip().lower() == title:
        print(nb['id']); break
" || true)"
fi

if [[ -z "$NOTEBOOK_ID" ]]; then
    log "Creating notebook '$NOTEBOOK_TITLE'..."
    if (( DRY_RUN )); then
        NOTEBOOK_ID="DRY-RUN-NB-ID"
    else
        NOTEBOOK_ID="$(nlm notebook create "$NOTEBOOK_TITLE" --json 2>/dev/null \
            | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")"
        [[ -n "$NOTEBOOK_ID" ]] || die "Failed to create notebook (empty id from nlm)."
    fi
    log "Created notebook $NOTEBOOK_ID"
else
    log "Reusing notebook $NOTEBOOK_ID"
fi

# --- 3. enumerate source files ----------------------------------------------
mapfile -t SOURCES < <(
    printf '%s\n' \
        "branding/BRAND.md|TASMail brand spec" \
        "branding/src/build_logo.py|Logo generator script (SVG geometry)"
    for svg in branding/build/svg/*.svg branding/build/wordmark/*.svg; do
        [[ -f "$svg" ]] || continue
        title="$(basename "$svg" .svg)"
        printf '%s|%s\n' "$svg" "SVG: $title"
    done
)

log "Sources to upload (${#SOURCES[@]}):"
for s in "${SOURCES[@]}"; do log "  - ${s%%|*}"; done

if (( DRY_RUN )); then
    log "Dry run — skipping uploads and critique."
    exit 0
fi

# --- 4. add sources ----------------------------------------------------------
for entry in "${SOURCES[@]}"; do
    path="${entry%%|*}"
    title="${entry##*|}"
    log "Uploading $path..."
    nlm source add "$NOTEBOOK_ID" --file "$path" --title "$title" --wait \
        || die "Source upload failed for $path"
done

# --- 5. critique query -------------------------------------------------------
mkdir -p "$OUTPUT_DIR"
log "Querying NotebookLM for the brand critique..."

QUERY=$(cat <<'EOF'
You are critiquing the TASMail brand identity. Inputs are:
  - branding/BRAND.md (palette tokens, geometry, dos/don'ts)
  - branding/src/build_logo.py (the SVG generator — read this to understand
    every stroke is intentional and edits land here)
  - six logo SVGs (primary/dark/mono-black/mono-white + two tiles) and three
    wordmark SVGs (light/dark/blue)

Produce three labelled sections. Be specific and reference exact files or
hex/contrast values:

  ## Contrast issues
  Audit every color pairing the mark uses (envelope outline on light/dark
  surfaces, the teal '@' on each background, the wordmark accent). Flag any
  combo that fails WCAG AA at the size it ships at. The spec already notes
  teal-on-white = 3.4:1 (icon-only) — verify and identify any other risky
  pairs.

  ## Off-balance kerning / spacing
  Look at the inner `t@s` wordmark inside the envelope and the standalone
  `TASMail` wordmark. Call out optical-balance issues (left/right whitespace
  mismatch, baseline drift, '@' competing with neighboring glyphs, clear-space
  violations at small sizes).

  ## Simplifications
  Propose concrete simplifications that would survive 16px/24px favicon
  rendering (stroke weight, corner treatment, flap geometry, glyph weight).
  Each suggestion should name the file + element to touch and the expected
  visual outcome.
EOF
)

# nlm notebook query writes the answer to stdout
nlm notebook query "$NOTEBOOK_ID" "$QUERY" 2>/dev/null > "$OUTPUT_FILE.body" \
    || die "Query failed against notebook $NOTEBOOK_ID"

{
    printf '# TASMail brand — NotebookLM critique\n\n'
    printf '_Generated %s by `scripts/tasmail-brand-critique.sh` (TMAIL-192)._\n\n' "$(date -Iseconds)"
    printf 'Source notebook: `%s` (`%s`)\n\n' "$NOTEBOOK_TITLE" "$NOTEBOOK_ID"
    printf -- '---\n\n'
    cat "$OUTPUT_FILE.body"
} > "$OUTPUT_FILE"
rm -f "$OUTPUT_FILE.body"

log "Critique saved to $OUTPUT_FILE"
log "Next steps:"
log "  1. Review $OUTPUT_FILE"
log "  2. Apply chosen iterations by editing branding/src/build_logo.py"
log "  3. Regenerate: python3 branding/src/build_logo.py && python3 branding/src/build_assets.py"
log "  4. Commit the updated SVGs + rasters together"
