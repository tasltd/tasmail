#!/usr/bin/env bash
# Added: Operator helper for TMAIL-159 — Path A of the PayPro → TASMail payment
# provider credential migration. Reads a structured JSON file of plaintext
# credentials, POSTs each provider to the TASMail admin endpoint, verifies the
# result, and prints the audit-log row to copy into
# docs/PAYMENT-PROVIDER-MIGRATION.md §7.
#
# This script never reads from PayPro directly — the operator extracts plaintext
# from PayPro's Admin UI (or a manual export) into the JSON file. See the runbook
# for the full unblock procedure.
#
# USAGE:
#   ./migrate-payment-providers.sh --file /path/to/payment-providers.local.json \
#     --base-url https://mail.techatscale.io \
#     --token "$TASMAIL_TOKEN"
#
#   # Dry run — validate the file, print intended payloads, no network calls
#   ./migrate-payment-providers.sh --file ... --dry-run
#
#   # Verify-only — skip POSTs, just GET /api/admin/payment-providers
#   ./migrate-payment-providers.sh --base-url ... --token ... --verify-only
#
# CREDENTIALS FILE FORMAT: see payment-providers.example.json (same directory).

set -euo pipefail

# Added: Defaults — overridable via flags or env vars
BASE_URL="${TASMAIL_BASE:-https://mail.techatscale.io}"
TOKEN="${TASMAIL_TOKEN:-}"
CREDS_FILE=""
DRY_RUN="false"
VERIFY_ONLY="false"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

usage() {
    cat <<'EOF'
migrate-payment-providers.sh — TMAIL-159 Path A operator helper

USAGE:
  ./migrate-payment-providers.sh [options]

OPTIONS:
  --file PATH           JSON credentials file (see payment-providers.example.json)
  --base-url URL        TASMail backend URL (default: $TASMAIL_BASE or
                        https://mail.techatscale.io)
  --token JWT           Admin JWT bearer token (default: $TASMAIL_TOKEN)
  --dry-run             Validate file and print payloads, make no network calls
  --verify-only         Skip POSTs, just GET /api/admin/payment-providers
  --help                Show this help

ENV VARS (alternative to flags):
  TASMAIL_BASE          Backend URL
  TASMAIL_TOKEN         Admin JWT bearer token

NOTES:
  - Path B (DB extraction) is NOT covered by this script — a Path B script would
    contain hardcoded DB DSNs and the PayPro encryption key and must never be
    committed. Write that one ad-hoc, run it, delete it.
  - This script writes to the *target* TASMail backend. Verify $BASE_URL is what
    you expect before running with --dry-run=false.
  - Required tools: curl, jq.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --file) CREDS_FILE="$2"; shift 2 ;;
        --base-url) BASE_URL="$2"; shift 2 ;;
        --token) TOKEN="$2"; shift 2 ;;
        --dry-run) DRY_RUN="true"; shift ;;
        --verify-only) VERIFY_ONLY="true"; shift ;;
        --help|-h) usage; exit 0 ;;
        *) echo -e "${RED}Unknown flag: $1${NC}" >&2; usage; exit 2 ;;
    esac
done

# Added: Tool dependency check — fail early with a clear message
for tool in curl jq; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo -e "${RED}Missing required tool: $tool${NC}" >&2
        echo "Install with: sudo apt install $tool" >&2
        exit 1
    fi
done

if [[ -z "$BASE_URL" ]]; then
    echo -e "${RED}--base-url required (or set TASMAIL_BASE)${NC}" >&2
    exit 2
fi

# Added: Token only required when we actually call the API
if [[ "$DRY_RUN" != "true" && -z "$TOKEN" ]]; then
    echo -e "${RED}--token required (or set TASMAIL_TOKEN). Get one via POST /api/auth/login.${NC}" >&2
    exit 2
fi

ALLOWED_PROVIDERS=(PAYSTACK MASTERCARD CYBERSOURCE BANK_TRANSFER)

# PURPOSE: Validate a single provider object — required-fields rule from
# PayPro's PaymentProviderConfig.hasRequiredCredentials().
validate_provider() {
    local idx="$1"
    local obj="$2"
    local provider
    provider=$(echo "$obj" | jq -r '.provider // empty')

    if [[ -z "$provider" ]]; then
        echo -e "${RED}entry[$idx]: missing 'provider' field${NC}" >&2
        return 1
    fi
    if [[ ! " ${ALLOWED_PROVIDERS[*]} " =~ \ ${provider}\  ]]; then
        echo -e "${RED}entry[$idx]: provider '$provider' not one of: ${ALLOWED_PROVIDERS[*]}${NC}" >&2
        return 1
    fi

    case "$provider" in
        PAYSTACK)
            local secret public
            secret=$(echo "$obj" | jq -r '.secret_key // empty')
            public=$(echo "$obj" | jq -r '.public_key // empty')
            if [[ -z "$secret" ]]; then
                echo -e "${RED}entry[$idx] PAYSTACK: 'secret_key' required${NC}" >&2
                return 1
            fi
            if [[ -z "$public" ]]; then
                echo -e "${RED}entry[$idx] PAYSTACK: 'public_key' required${NC}" >&2
                return 1
            fi
            ;;
        MASTERCARD)
            local mid pw
            mid=$(echo "$obj" | jq -r '.merchant_id // empty')
            pw=$(echo "$obj" | jq -r '.api_password // empty')
            if [[ -z "$mid" ]]; then
                echo -e "${RED}entry[$idx] MASTERCARD: 'merchant_id' required${NC}" >&2
                return 1
            fi
            if [[ -z "$pw" ]]; then
                echo -e "${RED}entry[$idx] MASTERCARD: 'api_password' required${NC}" >&2
                return 1
            fi
            ;;
        CYBERSOURCE)
            local mid kid sec
            mid=$(echo "$obj" | jq -r '.merchant_id // empty')
            kid=$(echo "$obj" | jq -r '.key_id // empty')
            sec=$(echo "$obj" | jq -r '.shared_secret_key // empty')
            if [[ -z "$mid" ]]; then
                echo -e "${RED}entry[$idx] CYBERSOURCE: 'merchant_id' required${NC}" >&2
                return 1
            fi
            if [[ -z "$kid" ]]; then
                echo -e "${RED}entry[$idx] CYBERSOURCE: 'key_id' required${NC}" >&2
                return 1
            fi
            if [[ -z "$sec" ]]; then
                echo -e "${RED}entry[$idx] CYBERSOURCE: 'shared_secret_key' required${NC}" >&2
                return 1
            fi
            ;;
        BANK_TRANSFER)
            local bd
            bd=$(echo "$obj" | jq -c '.bank_details // empty')
            if [[ -z "$bd" || "$bd" == "null" ]]; then
                echo -e "${RED}entry[$idx] BANK_TRANSFER: 'bank_details' object required${NC}" >&2
                return 1
            fi
            ;;
    esac

    return 0
}

# PURPOSE: GET /api/admin/payment-providers and pretty-print the summary
verify_providers() {
    echo -e "${BLUE}Verifying $BASE_URL/api/admin/payment-providers ...${NC}"
    local resp
    if ! resp=$(curl -fsS "$BASE_URL/api/admin/payment-providers" \
        -H "Authorization: Bearer $TOKEN" 2>&1); then
        echo -e "${RED}GET failed: $resp${NC}" >&2
        return 1
    fi

    echo "$resp" | jq '[.[] | {
        provider, name, environment, enabled, archived,
        has_secret_key, has_merchant_id, has_api_password,
        has_key_id, has_shared_secret_key
    }]'

    local enabled_providers
    enabled_providers=$(echo "$resp" | jq -r '
        [.[] | select(.archived == false and .enabled == true) | .provider] | sort | .[]
    ')

    echo
    echo -e "${BLUE}Enabled non-archived providers:${NC}"
    if [[ -z "$enabled_providers" ]]; then
        echo -e "${YELLOW}  (none — /api/billing/plans will still return 503)${NC}"
        return 1
    else
        echo "$enabled_providers" | sed 's/^/  /'
    fi

    local missing=()
    for p in "${ALLOWED_PROVIDERS[@]}"; do
        if ! echo "$enabled_providers" | grep -qFx "$p"; then
            missing+=("$p")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo
        echo -e "${YELLOW}Still missing: ${missing[*]}${NC}"
        return 1
    fi

    echo
    echo -e "${GREEN}All four PayPro providers present and enabled.${NC}"
    return 0
}

# PURPOSE: POST one provider entry to /api/admin/payment-providers
post_provider() {
    local payload="$1"
    local provider
    provider=$(echo "$payload" | jq -r '.provider')

    echo -e "${BLUE}→ POST $provider${NC}"

    if [[ "$DRY_RUN" == "true" ]]; then
        echo "  payload (sensitive fields redacted):"
        echo "$payload" | jq '
            . as $p
            | reduce ["secret_key","public_key","webhook_secret","merchant_id",
                      "api_password","key_id","shared_secret_key"][] as $k
                ($p; if (.[$k] // null) != null then .[$k] = "***REDACTED***" else . end)
        ' | sed 's/^/    /'
        return 0
    fi

    local resp http_code
    resp=$(curl -sS -o /tmp/migrate-payment-providers.body -w "%{http_code}" \
        -X POST "$BASE_URL/api/admin/payment-providers" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d "$payload") || http_code="000"
    http_code="$resp"

    if [[ "$http_code" != "201" ]]; then
        echo -e "${RED}  POST failed (HTTP $http_code)${NC}" >&2
        cat /tmp/migrate-payment-providers.body >&2
        echo >&2
        return 1
    fi

    local summary id env
    summary=$(cat /tmp/migrate-payment-providers.body)
    id=$(echo "$summary" | jq -r '.id')
    env=$(echo "$summary" | jq -r '.environment // "unknown"')

    echo -e "${GREEN}  ✓ created${NC} (id=$id environment=$env)"
    # Added: Audit-log row template for the operator to paste into the doc
    local today
    today=$(date -u +%Y-%m-%d)
    echo "  audit row → | $today | <operator> | $provider | global | $env | id=$id |"
    return 0
}

# Added: Verify-only mode — skip everything else
if [[ "$VERIFY_ONLY" == "true" ]]; then
    verify_providers
    exit $?
fi

if [[ -z "$CREDS_FILE" ]]; then
    echo -e "${RED}--file required (or pass --verify-only)${NC}" >&2
    usage >&2
    exit 2
fi

if [[ ! -r "$CREDS_FILE" ]]; then
    echo -e "${RED}Credentials file not readable: $CREDS_FILE${NC}" >&2
    exit 1
fi

# Added: Parse and validate the whole file before any POST — fail fast
if ! jq empty "$CREDS_FILE" 2>/dev/null; then
    echo -e "${RED}Credentials file is not valid JSON: $CREDS_FILE${NC}" >&2
    exit 1
fi

# Accept either a top-level array, or {"providers": [...]}
ENTRIES=$(jq -c 'if type=="array" then . elif .providers then .providers else error("expected array or {providers: [...]}") end | .[]' "$CREDS_FILE")
COUNT=$(echo "$ENTRIES" | wc -l)
if [[ -z "$ENTRIES" || "$COUNT" -eq 0 ]]; then
    echo -e "${RED}Credentials file has no provider entries${NC}" >&2
    exit 1
fi

echo -e "${BLUE}Loaded $COUNT provider entries from $CREDS_FILE${NC}"
echo -e "${BLUE}Target backend: $BASE_URL${NC}"
[[ "$DRY_RUN" == "true" ]] && echo -e "${YELLOW}DRY RUN — no network calls will be made${NC}"
echo

idx=0
while IFS= read -r entry; do
    if ! validate_provider "$idx" "$entry"; then
        echo -e "${RED}Aborting — fix the file and re-run${NC}" >&2
        exit 1
    fi
    idx=$((idx + 1))
done <<< "$ENTRIES"

echo -e "${GREEN}File validates. Posting providers...${NC}"
echo

failed=0
while IFS= read -r entry; do
    if ! post_provider "$entry"; then
        failed=$((failed + 1))
    fi
done <<< "$ENTRIES"

echo
if [[ $failed -gt 0 ]]; then
    echo -e "${RED}$failed provider POST(s) failed${NC}" >&2
    exit 1
fi

if [[ "$DRY_RUN" == "true" ]]; then
    echo -e "${GREEN}Dry run complete — $COUNT entries validated.${NC}"
    exit 0
fi

echo -e "${GREEN}All POSTs succeeded. Running verification...${NC}"
echo
verify_providers
