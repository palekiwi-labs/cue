#!/usr/bin/env bash
# Phase 5 SSE live validation — acuity GET /events/stream
#
# Usage:
#   bash validate-phase5-sse.sh
#
# Override the server URL:
#   ACUITY_URL=http://localhost:33222 bash validate-phase5-sse.sh
#
# Requirements: curl, jq
#
# What this script does:
#   1. Subscribes to /events/stream in the background
#   2. POSTs one event of each type
#   3. Waits up to 3 s for all four events to arrive on the stream
#   4. Tests Last-Event-ID resume: reconnects from the second event's seq
#      and asserts only the trailing events are replayed
#   5. Prints PASS / FAIL for each check

set -uo pipefail

BASE="${ACUITY_URL:-http://localhost:33223}"
SCHEMA="X-Acuity-Schema: 1"
CT="Content-Type: application/json"
TIMEOUT=3   # seconds to wait for SSE frames

GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

pass()    { echo -e "  ${GREEN}PASS${NC}  $1"; }
fail()    { echo -e "  ${RED}FAIL${NC}  $1"; FAILURES=$((FAILURES + 1)); }
section() { echo -e "\n${BOLD}${CYAN}--- $1 ---${NC}"; }

FAILURES=0

post_event() {
    curl -s -o /dev/null -w "%{http_code}" \
        -X POST "$BASE/events" \
        -H "$CT" -H "$SCHEMA" \
        -d "$1"
}

# Collect SSE data: lines from the stream output
# $1 = output file written by curl, $2 = event type to find, $3 = timeout
wait_for_event_type() {
    local file="$1" want="$2" deadline=$(( $(date +%s) + TIMEOUT ))
    while [[ $(date +%s) -lt $deadline ]]; do
        if grep -q "\"event_type\":\"$want\"" "$file" 2>/dev/null; then
            return 0
        fi
        sleep 0.1
    done
    return 1
}

echo -e "\n${BOLD}acuity Phase 5 — SSE live validation${NC}"
echo "Target: $BASE"

# --- 1. Server reachable -----------------------------------------------------

section "1. Server health"

STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/events" || echo "000")
if [[ "$STATUS" == "200" ]]; then
    pass "GET /events returns 200"
else
    echo -e "  ${RED}FAIL${NC}  Server not reachable at $BASE (HTTP $STATUS)"
    echo "       Start acuity before running this script:"
    echo "       cargo run -p acuity"
    exit 1
fi

# --- 2. Subscribe, then POST, assert all four types arrive -------------------

section "2. Live delivery — all four event types arrive on stream"

SID="sse-val-$$"
SSE_OUT=$(mktemp)
trap 'kill "$SSE_PID" 2>/dev/null; rm -f "$SSE_OUT"' EXIT

# Subscribe to the stream (background); pipe data: lines into the temp file.
curl --no-buffer -s -N \
    -H "Accept: text/event-stream" \
    "$BASE/events/stream" \
    | grep --line-buffered "^data:" \
    >> "$SSE_OUT" &
SSE_PID=$!

# Give curl a moment to establish the connection.
sleep 0.3

# POST all four event types.
post_event "{\"type\":\"session_idle\",\"session_id\":\"$SID\",\"project_dir\":\"/tmp/proj\",\"session_title\":\"sse-val\"}" > /dev/null
post_event "{\"type\":\"agent_turn_completed\",\"session_id\":\"$SID\",\"turn_id\":\"t1\",\"input_tokens\":10,\"output_tokens\":20}" > /dev/null
post_event "{\"type\":\"tool_call_requested\",\"session_id\":\"$SID\",\"turn_id\":\"t1\",\"tool_call_id\":\"c1\",\"tool_name\":\"bash\",\"args\":{}}" > /dev/null
post_event "{\"type\":\"tool_call_completed\",\"session_id\":\"$SID\",\"turn_id\":\"t1\",\"tool_call_id\":\"c1\",\"tool_name\":\"bash\",\"is_error\":false}" > /dev/null

for et in session_idle agent_turn_completed tool_call_requested tool_call_completed; do
    if wait_for_event_type "$SSE_OUT" "$et"; then
        pass "$et arrived on stream"
    else
        fail "$et did not arrive within ${TIMEOUT}s"
    fi
done

kill "$SSE_PID" 2>/dev/null
SSE_PID=0

# --- 3. Last-Event-ID resume -------------------------------------------------

section "3. Last-Event-ID resume"

# Find the seq of the first event for this session.
RESP=$(curl -s "$BASE/events?session_id=$SID")
FIRST_SEQ=$(jq '.events[0].seq' <<< "$RESP")
LAST_SEQ=$(jq '.events[-1].seq' <<< "$RESP")

echo "  session seqs: $FIRST_SEQ .. $LAST_SEQ"
echo "  reconnecting with Last-Event-ID: $FIRST_SEQ"

RESUME_OUT=$(mktemp)
trap 'kill "$RESUME_PID" 2>/dev/null; rm -f "$RESUME_OUT" "$SSE_OUT"' EXIT

curl --no-buffer -s -N \
    -H "Accept: text/event-stream" \
    -H "Last-Event-ID: $FIRST_SEQ" \
    "$BASE/events/stream" \
    | grep --line-buffered "^data:" \
    >> "$RESUME_OUT" &
RESUME_PID=$!

# Wait for the second event (seq after FIRST_SEQ) to arrive.
SECOND_TYPE=$(jq -r '.events[1].event_type' <<< "$RESP")
if wait_for_event_type "$RESUME_OUT" "$SECOND_TYPE"; then
    pass "resumed from seq $FIRST_SEQ — received subsequent events"
else
    fail "no events received after resume from seq $FIRST_SEQ within ${TIMEOUT}s"
fi

# The first event (seq == FIRST_SEQ) must NOT appear — it was before the cursor.
FIRST_TYPE=$(jq -r '.events[0].event_type' <<< "$RESP")
# Give the stream a moment to flush anything it might send.
sleep 0.6
FIRST_IN_RESUME=$(grep -c "\"event_type\":\"$FIRST_TYPE\",\"session_id\":\"$SID\"" "$RESUME_OUT" 2>/dev/null || true)
if [[ "$FIRST_IN_RESUME" -eq 0 ]]; then
    pass "event at seq $FIRST_SEQ not replayed (cursor respected)"
else
    fail "event at seq $FIRST_SEQ was replayed despite Last-Event-ID=$FIRST_SEQ"
fi

kill "$RESUME_PID" 2>/dev/null
RESUME_PID=0

# --- summary -----------------------------------------------------------------

echo ""
if [[ "$FAILURES" -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All SSE checks passed.${NC}"
else
    echo -e "${RED}${BOLD}$FAILURES check(s) FAILED.${NC}"
    exit 1
fi
