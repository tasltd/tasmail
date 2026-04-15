#!/usr/bin/env bash
# Added: IP warm-up schedule generator and enforcer for TMAIL-17
# PURPOSE: Generates an 8-week email sending schedule for new IPs,
#          tracks daily limits, and can be called by cron to check/enforce limits
# USAGE:
#   ./ip-warmup.sh --generate          Print the full 8-week schedule
#   ./ip-warmup.sh --check             Check today's limit and remaining sends
#   ./ip-warmup.sh --status            Show warm-up progress for all tracked IPs
#   ./ip-warmup.sh --start <IP>        Start tracking a new IP
#   ./ip-warmup.sh --help              Show this help message

set -euo pipefail

# Added: State file location (configurable via env var)
STATE_DIR="${TASMAIL_STATE_DIR:-/var/lib/tasmail}"
STATE_FILE="${STATE_DIR}/warmup-state.json"

# Added: 8-week warm-up progression (day_limit per week)
# Week 8 limit of 0 means unlimited
declare -a WEEKLY_LIMITS=(50 100 250 500 1000 2500 5000 0)
declare -a WEEKLY_DESCRIPTIONS=(
    "Initial warm-up — low volume, establish reputation"
    "Gradual increase — monitor bounce rates"
    "Moderate volume — check spam folder placement"
    "Steady growth — review engagement metrics"
    "Scaling up — maintain consistent sending patterns"
    "High volume ramp — monitor deliverability scores"
    "Near-full capacity — verify inbox placement rates"
    "Warm-up complete — unlimited sending"
)

# Added: Color output helpers
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

usage() {
    cat <<EOF
IP Warm-up Schedule Manager for TASMail

Usage: $(basename "$0") [OPTION]

Options:
  --generate        Print the full 8-week warm-up schedule
  --check           Check today's sending limit and remaining capacity
  --status          Show warm-up progress for all tracked IPs
  --start <IP>      Start tracking warm-up for a new sending IP
  --help            Show this help message

State file: ${STATE_FILE}

Environment:
  TASMAIL_STATE_DIR  Override state directory (default: /var/lib/tasmail)

Examples:
  $(basename "$0") --generate
  $(basename "$0") --start 203.0.113.10
  $(basename "$0") --check
  $(basename "$0") --status
EOF
}

# Added: Ensure state directory and file exist
ensure_state_file() {
    if [ ! -d "${STATE_DIR}" ]; then
        mkdir -p "${STATE_DIR}" 2>/dev/null || {
            echo -e "${RED}Error: Cannot create state directory ${STATE_DIR}${NC}" >&2
            echo "Try: sudo mkdir -p ${STATE_DIR} && sudo chown \$(whoami) ${STATE_DIR}" >&2
            exit 1
        }
    fi

    if [ ! -f "${STATE_FILE}" ]; then
        echo '{"ips":{}}' > "${STATE_FILE}"
    fi
}

# Added: Get the daily limit for a given day number (1-56)
get_limit_for_day() {
    local day=$1
    if [ "$day" -le 0 ] || [ "$day" -gt 56 ]; then
        echo "0"
        return
    fi
    local week_index=$(( (day - 1) / 7 ))
    echo "${WEEKLY_LIMITS[$week_index]}"
}

# Added: Get the week number for a given day (1-8)
get_week_for_day() {
    local day=$1
    echo $(( (day - 1) / 7 + 1 ))
}

# Added: Print the full 8-week warm-up schedule
cmd_generate() {
    echo -e "${BLUE}=== TASMail IP Warm-up Schedule (8 Weeks) ===${NC}"
    echo ""
    printf "%-8s %-15s %-55s\n" "Week" "Daily Limit" "Description"
    printf "%-8s %-15s %-55s\n" "----" "-----------" "-----------"

    for i in "${!WEEKLY_LIMITS[@]}"; do
        local week=$((i + 1))
        local limit=${WEEKLY_LIMITS[$i]}
        local desc="${WEEKLY_DESCRIPTIONS[$i]}"

        if [ "$limit" -eq 0 ]; then
            limit_display="Unlimited"
        else
            limit_display="${limit}/day"
        fi

        printf "%-8s %-15s %-55s\n" "Week ${week}" "${limit_display}" "${desc}"
    done

    echo ""
    echo -e "${YELLOW}Total duration: 56 days (8 weeks)${NC}"
    echo -e "${YELLOW}After week 8, sending is unrestricted.${NC}"
}

# Added: Check today's limit for all tracked IPs
cmd_check() {
    ensure_state_file

    local today
    today=$(date +%Y-%m-%d)

    # Added: Parse state file with jq if available, otherwise use python
    if command -v jq &>/dev/null; then
        local ip_count
        ip_count=$(jq '.ips | length' "${STATE_FILE}")

        if [ "$ip_count" -eq 0 ]; then
            echo -e "${YELLOW}No IPs are being tracked. Use --start <IP> to begin.${NC}"
            return
        fi

        echo -e "${BLUE}=== Daily Limit Check (${today}) ===${NC}"
        echo ""

        jq -r '.ips | to_entries[] | "\(.key) \(.value.current_day) \(.value.emails_sent_today) \(.value.last_reset_date)"' "${STATE_FILE}" | \
        while read -r ip day sent last_reset; do
            # Added: Reset daily counter if date changed
            if [ "${last_reset}" != "${today}" ]; then
                day=$((day + 1))
                sent=0
                # Added: Update state file with new day
                local tmp
                tmp=$(jq --arg ip "$ip" --arg today "$today" --argjson day "$day" \
                    '.ips[$ip].current_day = $day | .ips[$ip].emails_sent_today = 0 | .ips[$ip].last_reset_date = $today' \
                    "${STATE_FILE}")
                echo "$tmp" > "${STATE_FILE}"
            fi

            local limit
            limit=$(get_limit_for_day "$day")
            local week
            week=$(get_week_for_day "$day")

            if [ "$day" -gt 56 ]; then
                echo -e "${GREEN}IP ${ip}: Warm-up COMPLETE — unlimited sending${NC}"
            elif [ "$limit" -eq 0 ]; then
                echo -e "${GREEN}IP ${ip}: Week ${week}, Day ${day} — unlimited sending${NC}"
            else
                local remaining=$((limit - sent))
                if [ "$remaining" -le 0 ]; then
                    remaining=0
                    echo -e "${RED}IP ${ip}: Week ${week}, Day ${day} — LIMIT REACHED (${sent}/${limit})${NC}"
                else
                    echo -e "${GREEN}IP ${ip}: Week ${week}, Day ${day} — ${sent}/${limit} sent, ${remaining} remaining${NC}"
                fi
            fi
        done
    else
        echo -e "${YELLOW}jq is required for --check and --status. Install with: sudo apt install jq${NC}" >&2
        exit 1
    fi
}

# Added: Show warm-up progress for all tracked IPs
cmd_status() {
    ensure_state_file

    if ! command -v jq &>/dev/null; then
        echo -e "${YELLOW}jq is required. Install with: sudo apt install jq${NC}" >&2
        exit 1
    fi

    local ip_count
    ip_count=$(jq '.ips | length' "${STATE_FILE}")

    if [ "$ip_count" -eq 0 ]; then
        echo -e "${YELLOW}No IPs are being tracked. Use --start <IP> to begin.${NC}"
        return
    fi

    echo -e "${BLUE}=== IP Warm-up Status ===${NC}"
    echo ""
    printf "%-20s %-8s %-8s %-12s %-15s %-10s\n" "IP Address" "Day" "Week" "Limit" "Sent Today" "Status"
    printf "%-20s %-8s %-8s %-12s %-15s %-10s\n" "----------" "---" "----" "-----" "----------" "------"

    jq -r '.ips | to_entries[] | "\(.key) \(.value.current_day) \(.value.emails_sent_today) \(.value.total_emails_sent) \(.value.paused) \(.value.started_at)"' "${STATE_FILE}" | \
    while read -r ip day sent total paused started; do
        local week
        week=$(get_week_for_day "$day")
        local limit
        limit=$(get_limit_for_day "$day")
        local status

        if [ "$day" -gt 56 ]; then
            status="Complete"
            limit="Unlimited"
        elif [ "${paused}" = "true" ]; then
            status="Paused"
        else
            status="Active"
        fi

        if [ "$limit" = "Unlimited" ] || [ "$limit" -eq 0 ] 2>/dev/null; then
            limit="Unlimited"
        fi

        printf "%-20s %-8s %-8s %-12s %-15s %-10s\n" "${ip}" "${day}" "${week}" "${limit}" "${sent}" "${status}"
    done

    echo ""
    echo -e "${YELLOW}State file: ${STATE_FILE}${NC}"
}

# Added: Start tracking a new IP
cmd_start() {
    local ip="${1:-}"

    if [ -z "$ip" ]; then
        echo -e "${RED}Error: IP address required. Usage: $(basename "$0") --start <IP>${NC}" >&2
        exit 1
    fi

    ensure_state_file

    if ! command -v jq &>/dev/null; then
        echo -e "${YELLOW}jq is required. Install with: sudo apt install jq${NC}" >&2
        exit 1
    fi

    # Added: Check if IP is already tracked
    local existing
    existing=$(jq --arg ip "$ip" '.ips[$ip] // empty' "${STATE_FILE}")

    if [ -n "$existing" ]; then
        echo -e "${YELLOW}IP ${ip} is already being tracked.${NC}"
        return
    fi

    local today
    today=$(date +%Y-%m-%d)
    local now
    now=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # Added: Initialize tracking state for the new IP
    local tmp
    tmp=$(jq --arg ip "$ip" --arg today "$today" --arg now "$now" \
        '.ips[$ip] = {
            "current_day": 1,
            "emails_sent_today": 0,
            "total_emails_sent": 0,
            "last_reset_date": $today,
            "started_at": $now,
            "paused": false
        }' "${STATE_FILE}")
    echo "$tmp" > "${STATE_FILE}"

    echo -e "${GREEN}Started warm-up tracking for IP ${ip}${NC}"
    echo -e "Day 1, Week 1 — Daily limit: ${WEEKLY_LIMITS[0]} emails/day"
}

# Added: Main entry point — parse command-line arguments
case "${1:-}" in
    --generate)
        cmd_generate
        ;;
    --check)
        cmd_check
        ;;
    --status)
        cmd_status
        ;;
    --start)
        cmd_start "${2:-}"
        ;;
    --help|-h)
        usage
        ;;
    *)
        usage
        exit 1
        ;;
esac
