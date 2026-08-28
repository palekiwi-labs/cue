#!/usr/bin/env sh
# Test: valid session.idle event forwarded to Gotify
# Requires: ACUITY_GOTIFY_TOKEN set in environment, acuity running on port 33222.
# Expected response: 200 and a Gotify notification appears.

set -e

if [ -z "$ACUITY_GOTIFY_TOKEN" ]; then
  echo "error: ACUITY_GOTIFY_TOKEN is not set" >&2
  exit 1
fi

curl -s -w "\n%{http_code}" -X POST http://localhost:33222/events \
  -H "Content-Type: application/json" \
  -H "X-Acuity-Schema: 1" \
  -d '{"session_id":"test-123","project_dir":"/home/pl/code/palekiwi-labs/cue","session_title":"test notification"}'
