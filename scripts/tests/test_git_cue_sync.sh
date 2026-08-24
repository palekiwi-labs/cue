#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SYNC_BIN="$SCRIPT_DIR/git-cue-sync"

TEMP_ROOT=$(mktemp -d /tmp/cue-sync-test-XXXXXX)
trap 'rm -rf "$TEMP_ROOT"' EXIT

echo "Running git-cue-sync tests in $TEMP_ROOT..."

# Setup fake bin directory for mocking `cue`
MOCK_BIN="$TEMP_ROOT/mock_bin"
mkdir -p "$MOCK_BIN"

# Mock cue: records invocations and mimics `cue switch <slug>` by
# writing the slug to .cue/HEAD in the current directory.
CUE_LOG="$TEMP_ROOT/cue_calls.log"
export CUE_LOG
cat << 'EOF' > "$MOCK_BIN/cue"
#!/usr/bin/env bash
echo "switch $2" >> "$CUE_LOG"
mkdir -p .cue
printf '%s\n' "$2" > .cue/HEAD
exit 0
EOF
chmod +x "$MOCK_BIN/cue"
export PATH="$MOCK_BIN:$PATH"

# Setup a test git repo on a feature branch
REPO_DIR="$TEMP_ROOT/repo"
mkdir -p "$REPO_DIR"
cd "$REPO_DIR"
git init -b master
git config user.name "Test User"
git config user.email "test@example.com"
echo "initial" > file.txt
git add file.txt
git commit -m "initial commit"
git checkout -b feature/test-branch
mkdir .cue

cue_was_called() {
  [ -f "$CUE_LOG" ]
}

last_cue_call() {
  [ -f "$CUE_LOG" ] && tail -n 1 "$CUE_LOG"
}

# --------------------------------------------------------------------------
# Test 1: set writes the association key for the current branch
# --------------------------------------------------------------------------
"$SYNC_BIN" set my-task

[ "$(git config "branch.feature/test-branch.cue-task")" = "my-task" ] || {
  echo "FAIL: cue-task key not set to my-task"; exit 1;
}
if cue_was_called; then
  echo "FAIL: set mode must not invoke cue"; exit 1
fi

echo "PASS: Test 1 (set writes key)"

# --------------------------------------------------------------------------
# Test 2: set master clears the association
# --------------------------------------------------------------------------
"$SYNC_BIN" set master

if git config "branch.feature/test-branch.cue-task" 2>/dev/null; then
  echo "FAIL: set master should clear the key"; exit 1
fi

echo "PASS: Test 2 (set master clears)"

# --------------------------------------------------------------------------
# Test 3: set with an explicit empty slug clears the association
# --------------------------------------------------------------------------
git config "branch.feature/test-branch.cue-task" leftover
"$SYNC_BIN" set ""

if git config "branch.feature/test-branch.cue-task" 2>/dev/null; then
  echo "FAIL: set '' should clear the key"; exit 1
fi

echo "PASS: Test 3 (set empty clears)"

# --------------------------------------------------------------------------
# Test 4: unset clears and is idempotent
# --------------------------------------------------------------------------
git config "branch.feature/test-branch.cue-task" my-task
"$SYNC_BIN" unset
if git config "branch.feature/test-branch.cue-task" 2>/dev/null; then
  echo "FAIL: unset should clear the key"; exit 1
fi
"$SYNC_BIN" unset || { echo "FAIL: unset must be idempotent"; exit 1; }

echo "PASS: Test 4 (unset clears, idempotent)"

# --------------------------------------------------------------------------
# Test 5: set with no argument is a usage error
# --------------------------------------------------------------------------
set +e
"$SYNC_BIN" set >/dev/null 2>&1
status=$?
set -e
[ "$status" -ne 0 ] || { echo "FAIL: set without slug must exit non-zero"; exit 1; }

echo "PASS: Test 5 (set requires slug)"

# --------------------------------------------------------------------------
# Test 6: hook mode switches when the key is present
# --------------------------------------------------------------------------
git config "branch.feature/test-branch.cue-task" my-task
"$SYNC_BIN"

[ "$(last_cue_call)" = "switch my-task" ] || {
  echo "FAIL: hook mode did not call cue switch my-task (got: $(last_cue_call))"; exit 1;
}
[ "$(cat .cue/HEAD)" = "my-task" ] || {
  echo "FAIL: .cue/HEAD not switched"; exit 1;
}

echo "PASS: Test 6 (hook mode switches)"

# --------------------------------------------------------------------------
# Test 7: hook mode without the key is a no-op
# --------------------------------------------------------------------------
git config --unset "branch.feature/test-branch.cue-task"
calls_before=$( [ -f "$CUE_LOG" ] && wc -l < "$CUE_LOG" || echo 0 )
"$SYNC_BIN"
calls_after=$( [ -f "$CUE_LOG" ] && wc -l < "$CUE_LOG" || echo 0 )
[ "$calls_before" = "$calls_after" ] || {
  echo "FAIL: hook mode invoked cue without association"; exit 1;
}

echo "PASS: Test 7 (hook mode no-op without key)"

# --------------------------------------------------------------------------
# Test 8: hook mode is a no-op when there is no cue store
# --------------------------------------------------------------------------
git config "branch.feature/test-branch.cue-task" my-task
rm -rf .cue
calls_before=$( [ -f "$CUE_LOG" ] && wc -l < "$CUE_LOG" || echo 0 )
"$SYNC_BIN"
calls_after=$( [ -f "$CUE_LOG" ] && wc -l < "$CUE_LOG" || echo 0 )
[ "$calls_before" = "$calls_after" ] || {
  echo "FAIL: hook mode invoked cue without .cue store"; exit 1;
}
mkdir .cue

echo "PASS: Test 8 (hook mode no-op without store)"

# --------------------------------------------------------------------------
# Test 9: hook mode is a no-op on detached HEAD
# --------------------------------------------------------------------------
git checkout --detach HEAD
"$SYNC_BIN"
calls_after=$( [ -f "$CUE_LOG" ] && wc -l < "$CUE_LOG" || echo 0 )
[ "$calls_before" = "$calls_after" ] || {
  echo "FAIL: hook mode invoked cue on detached HEAD"; exit 1;
}
git checkout feature/test-branch

echo "PASS: Test 9 (hook mode no-op on detached HEAD)"

# --------------------------------------------------------------------------
# Test 10: set on detached HEAD is a loud error
# --------------------------------------------------------------------------
git checkout --detach HEAD
set +e
"$SYNC_BIN" set another-task >/dev/null 2>&1
status=$?
set -e
[ "$status" -ne 0 ] || { echo "FAIL: set on detached HEAD must exit non-zero"; exit 1; }
git checkout feature/test-branch

echo "PASS: Test 10 (set fails on detached HEAD)"

# --------------------------------------------------------------------------
# Test 11: hook mode exits 0 when cue is missing
# --------------------------------------------------------------------------
GIT_ONLY_BIN="$TEMP_ROOT/git_only_bin"
mkdir -p "$GIT_ONLY_BIN"
ln -s "$(command -v bash)" "$GIT_ONLY_BIN/bash"
ln -s "$(command -v git)" "$GIT_ONLY_BIN/git"

set +e
env PATH="$GIT_ONLY_BIN" "$SYNC_BIN"
status=$?
set -e
[ "$status" -eq 0 ] || { echo "FAIL: hook mode must exit 0 when cue is missing"; exit 1; }

echo "PASS: Test 11 (hook mode safe without cue)"

# --------------------------------------------------------------------------
# Test 12: hook mode exits 0 when cue switch fails
# --------------------------------------------------------------------------
cat << 'EOF' > "$MOCK_BIN/cue"
#!/usr/bin/env bash
echo "Error: boom" >&2
exit 1
EOF
chmod +x "$MOCK_BIN/cue"

set +e
"$SYNC_BIN" >/dev/null 2>&1
status=$?
set -e
[ "$status" -eq 0 ] || { echo "FAIL: hook mode must exit 0 when cue fails"; exit 1; }

echo "PASS: Test 12 (hook mode safe when cue fails)"

# --------------------------------------------------------------------------
# Test 13: hook mode exits 0 when cue crashes (kill -9)
# --------------------------------------------------------------------------
cat << 'EOF' > "$MOCK_BIN/cue"
#!/usr/bin/env bash
kill -9 $$
EOF
chmod +x "$MOCK_BIN/cue"

set +e
"$SYNC_BIN" >/dev/null 2>&1
status=$?
set -e
[ "$status" -eq 0 ] || { echo "FAIL: hook mode must exit 0 when cue crashes"; exit 1; }

echo "PASS: Test 13 (hook mode safe when cue crashes)"

# --------------------------------------------------------------------------
# Test 14: set fails loudly when the config write fails
# --------------------------------------------------------------------------
# A stale config.lock (e.g. lock contention) makes `git config key value`
# fail; set mode must surface that as a non-zero exit instead of printing
# the success message.
touch .git/config.lock
set +e
err=$("$SYNC_BIN" set some-task 2>&1 >/dev/null)
status=$?
set -e
rm -f .git/config.lock
[ "$status" -ne 0 ] || { echo "FAIL: set must exit non-zero when config write fails"; exit 1; }
case "$err" in
  *git-cue-sync*) ;;
  *) echo "FAIL: set must report the write failure (got: $err)"; exit 1 ;;
esac
case "$err" in
  *associated*) echo "FAIL: set reported success despite write failure"; exit 1 ;;
esac

echo "PASS: Test 14 (set fails loudly on config write failure)"

echo "ALL TESTS PASSED SUCCESSFULLY!"
