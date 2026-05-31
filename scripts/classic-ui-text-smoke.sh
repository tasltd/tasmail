#!/usr/bin/env bash
# TMAIL-369 — Text-browser smoke test for the Classic UI.
#
# Renders each Classic UI page through the backend's Axum router (via the
# `classic_dump_pages_for_text_browser_smoke_test` ignored test), then
# pipes the rendered HTML through `lynx -dump` (preferred) or `w3m -dump`
# (fallback) and asserts each dump contains the strings every page MUST
# surface for a screen-reader / text-browser user.
#
# Complements:
#   * Backend in-process structural test:
#       backend/tests/classic_a11y_test.rs
#   * Live-browser axe-core spec:
#       frontend/e2e/specs/classic-ui-a11y.spec.ts
#
# Why dump via the test harness instead of curl-ing the live deployment?
# The live `mail.techatscale.io` Apache vhost currently routes unknown
# `/classic/*` paths to the Vite SPA (a separate deployment bug, not in
# scope for TMAIL-369). Going through `cargo test` exercises the SAME
# Axum router a real deployment hits while remaining hermetic.
#
# Usage:
#
#   scripts/classic-ui-text-smoke.sh
#       Build + run the dump test + smoke-check the dumps.
#
#   TASMAIL_DUMP_DIR=/tmp/foo scripts/classic-ui-text-smoke.sh
#       Override dump directory.
#
# Exit codes:
#   0  — all pages dumped + expected text found
#   1  — a dump failed or an expected string was missing
#   2  — neither lynx nor w3m is installed
#   3  — cargo not on PATH

set -euo pipefail

# ── Pick a text browser. lynx is the canonical text-browser in the
# gap-analysis acceptance criteria; w3m is the universal Linux fallback.
TEXT_BROWSER=""
DUMP_CMD=()
if command -v lynx >/dev/null 2>&1; then
    TEXT_BROWSER="lynx"
    DUMP_CMD=(lynx -dump -nolist -display_charset=utf-8)
elif command -v w3m >/dev/null 2>&1; then
    TEXT_BROWSER="w3m"
    DUMP_CMD=(w3m -dump -T text/html)
else
    echo "ERROR: neither lynx nor w3m is installed. Install one with:" >&2
    echo "    sudo apt-get install -y lynx        # preferred" >&2
    echo "    sudo apt-get install -y w3m         # fallback" >&2
    exit 2
fi

# ── Confirm cargo is on PATH. The dump test runs via `cargo test`.
if ! command -v cargo >/dev/null 2>&1; then
    if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
        export PATH="$HOME/.cargo/bin:$PATH"
    else
        echo "ERROR: cargo not found on PATH and not at ~/.cargo/bin/cargo" >&2
        exit 3
    fi
fi

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BACKEND_DIR="${REPO_ROOT}/backend"
DUMP_DIR="${TASMAIL_DUMP_DIR:-/tmp/tasmail-classic-dumps-$$}"

echo "Text-browser: $TEXT_BROWSER"
echo "Backend dir:  $BACKEND_DIR"
echo "Dump dir:     $DUMP_DIR"
echo ""

mkdir -p "$DUMP_DIR"

cleanup() {
    if [[ -z "${TASMAIL_DUMP_DIR:-}" ]]; then
        rm -rf "$DUMP_DIR" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# ── Render the pages via the ignored dump test. We pass the destination
# directory through TASMAIL_DUMP_DIR so the test writes there.
echo "→ Rendering Classic UI pages via cargo test (this may compile on first run)…"
(
    cd "$BACKEND_DIR"
    TASMAIL_DUMP_DIR="$DUMP_DIR" \
        cargo test --test classic_a11y_test \
        classic_dump_pages_for_text_browser_smoke_test \
        -- --ignored --nocapture 2>&1 | tail -20
)
echo ""

# ── Smoke-check each dump.
PASS=0
FAIL=0

# Each row: dump_file | required_strings (pipe-separated, case-sensitive)
ROWS=(
    "login.html|Sign in to TASMail Classic|Email address|Password|Sign in|Skip to main content|TASMail Classic"
    "not_found.html|Page not found|Return to TASMail Classic|Skip to main content"
)

check_dump() {
    local file="$1"
    local required_csv="$2"
    local dump_path="${DUMP_DIR}/${file}"

    echo "→ ${file}"

    if [[ ! -f "$dump_path" ]]; then
        echo "  FAIL: dump file does not exist: $dump_path"
        return 1
    fi
    if [[ ! -s "$dump_path" ]]; then
        echo "  FAIL: dump file is empty: $dump_path"
        return 1
    fi
    echo "  OK:   $(wc -c < "$dump_path") bytes of HTML"

    local dump
    if ! dump=$("${DUMP_CMD[@]}" "$dump_path" 2>&1); then
        echo "  FAIL: $TEXT_BROWSER -dump returned non-zero"
        head -n 40 "$dump_path"
        return 1
    fi

    if [[ -z "$dump" ]]; then
        echo "  FAIL: $TEXT_BROWSER produced empty dump"
        echo "  ---- input HTML (first 40 lines) ----"
        head -n 40 "$dump_path"
        echo "  --------------------------------------"
        return 1
    fi

    IFS='|' read -r -a required_arr <<< "$required_csv"
    for needle in "${required_arr[@]}"; do
        if [[ -z "$needle" ]]; then continue; fi
        if ! grep -qF -- "$needle" <<< "$dump"; then
            echo "  FAIL: expected string not found in $TEXT_BROWSER dump: '$needle'"
            echo "  ---- dump snippet (first 40 lines) ----"
            head -n 40 <<< "$dump"
            echo "  ----------------------------------------"
            return 1
        fi
    done
    echo "  OK:   all ${#required_arr[@]} required string(s) found in dump"
    return 0
}

for row in "${ROWS[@]}"; do
    IFS='|' read -r file required_csv <<< "$row"
    if check_dump "$file" "$required_csv"; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1))
    fi
    echo ""
done

echo "===================="
echo "Pass: $PASS"
echo "Fail: $FAIL"
echo "===================="
if (( FAIL > 0 )); then
    exit 1
fi
exit 0
