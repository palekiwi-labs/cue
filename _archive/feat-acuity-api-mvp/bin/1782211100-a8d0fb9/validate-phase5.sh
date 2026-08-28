#!/usr/bin/env bash
# Phase 5 curl validation — acuity read model (GET /events + SSE smoke)
#
# Usage:
#   bash .cue/feat-acuity-api-mvp/tmp/<timestamp>/validate-phase5.sh
#
# Override the server URL:
#   ACUITY_URL=http://localhost:33222 bash validate-phase5.sh
#
# Requirements: curl, jq

set -uo pipefail

BASE="${ACUITY_URL:-http://localhost:33223}"
SCHEMA="X-Acuity-Schema: 1"
CT="Content-Type: application/json"

# --- colour helpers ----------------------------------------------------------

GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

pass()    { echo -e "  ${GREEN}PASS${NC}  $1"; }
fail()    { echo -e "  ${RED}FAIL${NC}  $1"; FAILURES=$((FAILURES + 1)); }
section() { echo -e "\n${BOLD}${CYAN}--- $1 ---${NC}"; }

FAILURES=0

# --- helpers -----------------------------------------------------------------

post_event() {
    curl -s -o /dev/null -w "%{http_code}" \
        -X POST "$BASE/events" \
        -H "$CT" -H "$SCHEMA" \
        -d "$1"
}

get_events() {
    curl -s "$BASE/events${1:+?$1}"
}

count_events() {
    jq '.events | length' <<< "$1"
}

first_seq() {
    jq '.events[0].seq' <<< "$1"
}

# =============================================================================

echo -e "\n${BOLD}acuity Phase 5 — curl validation${NC}"
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

# --- 2. Seed test events -----------------------------------------------------

section "2. Seed events (POST /events)"

SID="curl-val-$$"   # unique per run via PID

s=$(post_event "{\"type\":\"session_idle\",\"session_id\":\"$SID\",\"project_dir\":\"/home/me/proj\",\"session_title\":\"curl-validation\"}")
[[ "$s" == "200" ]] && pass "POST session_idle" || fail "POST session_idle (HTTP $s)"

s=$(post_event "{\"type\":\"agent_turn_completed\",\"session_id\":\"$SID\",\"turn_id\":\"t1\",\"input_tokens\":120,\"output_tokens\":340}")
[[ "$s" == "200" ]] && pass "POST agent_turn_completed" || fail "POST agent_turn_completed (HTTP $s)"

s=$(post_event "{\"type\":\"tool_call_requested\",\"session_id\":\"$SID\",\"turn_id\":\"t1\",\"tool_call_id\":\"c1\",\"tool_name\":\"read\",\"args\":{\"path\":\"/x\"}}")
[[ "$s" == "200" ]] && pass "POST tool_call_requested" || fail "POST tool_call_requested (HTTP $s)"

s=$(post_event "{\"type\":\"tool_call_completed\",\"session_id\":\"$SID\",\"turn_id\":\"t1\",\"tool_call_id\":\"c1\",\"tool_name\":\"read\",\"is_error\":false}")
[[ "$s" == "200" ]] && pass "POST tool_call_completed" || fail "POST tool_call_completed (HTTP $s)"

# --- 3. GET /events — unfiltered ---------------------------------------------

section "3. GET /events (unfiltered)"

RESP=$(get_events "")
COUNT=$(count_events "$RESP")
echo "  $(jq -c '{events: [.events[] | {seq,event_type,session_id}]}' <<< "$RESP")"
[[ "$COUNT" -ge 4 ]] \
    && pass "events array has $COUNT entries (>= 4 expected)" \
    || fail "expected >= 4 events, got $COUNT"

# --- 4. session_id filter ----------------------------------------------------

section "4. GET /events?session_id=$SID"

RESP=$(get_events "session_id=$SID")
COUNT=$(count_events "$RESP")
echo "  $(jq -c '[.events[] | {seq,event_type}]' <<< "$RESP")"
[[ "$COUNT" == "4" ]] \
    && pass "session_id filter returned exactly 4 events" \
    || fail "expected 4, got $COUNT"

# --- 5. event_type filter ----------------------------------------------------

section "5. GET /events?event_type=session_idle&session_id=$SID"

RESP=$(get_events "event_type=session_idle&session_id=$SID")
COUNT=$(count_events "$RESP")
echo "  $(jq -c '[.events[] | {seq,event_type}]' <<< "$RESP")"
[[ "$COUNT" == "1" ]] \
    && pass "event_type filter returned exactly 1 event" \
    || fail "expected 1, got $COUNT"

# --- 6. after= cursor --------------------------------------------------------

section "6. GET /events?after=<first_seq>&session_id=$SID"

RESP_ALL=$(get_events "session_id=$SID")
FIRST=$(first_seq "$RESP_ALL")
RESP=$(get_events "after=$FIRST&session_id=$SID")
COUNT=$(count_events "$RESP")
echo "  after=$FIRST: $(jq -c '[.events[] | {seq,event_type}]' <<< "$RESP")"
[[ "$COUNT" == "3" ]] \
    && pass "after=$FIRST skipped first event, returned 3" \
    || fail "expected 3, got $COUNT"

# --- 7. limit= cap -----------------------------------------------------------

section "7. GET /events?limit=2&session_id=$SID"

RESP=$(get_events "limit=2&session_id=$SID")
COUNT=$(count_events "$RESP")
echo "  $(jq -c '[.events[] | {seq,event_type}]' <<< "$RESP")"
[[ "$COUNT" == "2" ]] \
    && pass "limit=2 returned exactly 2 events" \
    || fail "expected 2, got $COUNT"

# --- 8. EventRecord field check ----------------------------------------------

section "8. EventRecord field completeness"

RESP=$(get_events "session_id=$SID&event_type=session_idle")
REC=$(jq '.events[0]' <<< "$RESP")
echo "  $REC"

for field in seq received_at event_type session_id payload; do
    VAL=$(jq -r ".$field // empty" <<< "$REC")
    [[ -n "$VAL" ]] \
        && pass "field '$field' present and non-empty" \
        || fail "field '$field' missing or empty"
done

# turn_id is null for session_idle — that is correct
TURN=$(jq '.turn_id' <<< "$REC")
[[ "$TURN" == "null" ]] \
    && pass "turn_id is null for session_idle (correct)" \
    || fail "turn_id should be null for session_idle, got $TURN"

# payload round-trips to valid JSON
jq '.' <<< "$(jq -r '.payload' <<< "$REC")" > /dev/null \
    && pass "payload is valid JSON" \
    || fail "payload is not valid JSON"

# --- summary -----------------------------------------------------------------

echo ""
if [[ "$FAILURES" -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}All checks passed.${NC}"
    echo ""
    echo "Next: SSE live validation — see instructions below."
else
    echo -e "${RED}${BOLD}$FAILURES check(s) FAILED.${NC}"
    exit 1
fi
