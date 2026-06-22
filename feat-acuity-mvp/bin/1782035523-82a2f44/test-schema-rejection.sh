#!/usr/bin/env sh
# Test: schema version rejection
# Sends a request with a wrong X-Acuity-Schema header.
# Expected response: 400

set -e

curl -s -w "\n%{http_code}" -X POST http://localhost:33222/events \
  -H "Content-Type: application/json" \
  -H "X-Acuity-Schema: 99" \
  -d '{"session_id":"x","project_dir":"/tmp","session_title":null}'
