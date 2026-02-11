#!/bin/bash
# sync-targets.sh — Pull targets from Sigma API and write to Prometheus file_sd
# Run via cron or systemd timer every 60s

set -euo pipefail

SIGMA_URL="${SIGMA_URL:-http://localhost:3000}"
API_KEY="${API_KEY:-}"
OUTPUT_FILE="${OUTPUT_FILE:-/etc/prometheus/targets/sigma.json}"

HEADERS=(-H "Content-Type: application/json")
if [ -n "$API_KEY" ]; then
    HEADERS+=(-H "X-Api-Key: $API_KEY")
fi

# Fetch targets from Sigma
RESPONSE=$(curl -sf "${HEADERS[@]}" "${SIGMA_URL}/api/prometheus/targets")

if [ $? -eq 0 ] && [ -n "$RESPONSE" ]; then
    # Atomic write
    TMPFILE=$(mktemp "${OUTPUT_FILE}.XXXXXX")
    echo "$RESPONSE" | jq '.' > "$TMPFILE"
    mv "$TMPFILE" "$OUTPUT_FILE"
    echo "[$(date -Iseconds)] Synced $(echo "$RESPONSE" | jq 'length') targets to ${OUTPUT_FILE}"
else
    echo "[$(date -Iseconds)] ERROR: Failed to fetch targets from Sigma" >&2
    exit 1
fi
