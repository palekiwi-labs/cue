#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SYNC_BIN="$SCRIPT_DIR/git-pr-sync"
BASE_BIN="$SCRIPT_DIR/get-pr-base"
NUM_BIN="$SCRIPT_DIR/get-pr-number"

TEMP_ROOT=$(mktemp -d /tmp/pr-scripts-test-XXXXXX)
trap 'rm -rf "$TEMP_ROOT"' EXIT

echo "Running git PR scripts tests in $TEMP_ROOT..."

# Setup fake bin directory for mocking `gh`
MOCK_BIN="$TEMP_ROOT/mock_bin"
mkdir -p "$MOCK_BIN"
export PATH="$MOCK_BIN:$PATH"

# Setup a test git repo
REPO_DIR="$TEMP_ROOT/repo"
mkdir -p "$REPO_DIR"
cd "$REPO_DIR"
git init -b master
git config user.name "Test User"
git config user.email "test@example.com"
echo "initial" > file.txt
git add file.txt
git commit -m "initial commit"

# Setup fake origin remote
ORIGIN_DIR="$TEMP_ROOT/origin"
git clone --bare "$REPO_DIR" "$ORIGIN_DIR"
git remote add origin "$ORIGIN_DIR"
git fetch origin

# Create a feature branch
git checkout -b feature/test-branch
echo "feature work" >> file.txt
git commit -am "feature commit"

# --------------------------------------------------------------------------
# Test 1: Case 1 - PR Found & Authenticated
# --------------------------------------------------------------------------
cat << 'EOF' > "$MOCK_BIN/gh"
#!/usr/bin/env bash
if [ "$1" = "pr" ] && [ "$2" = "view" ]; then
  echo '{"number": 42, "baseRefName": "master"}'
  exit 0
fi
exit 1
EOF
chmod +x "$MOCK_BIN/gh"

"$SYNC_BIN"

# Verify config values
[ "$(git config "branch.feature/test-branch.base")" = "master" ] || { echo "FAIL: base not set to master"; exit 1; }
[ "$(git config "branch.feature/test-branch.pr")" = "42" ] || { echo "FAIL: pr not set to 42"; exit 1; }
# origin/master is not ahead yet (ahead should be unset)
if git config "branch.feature/test-branch.ahead" 2>/dev/null; then
  echo "FAIL: ahead should not be set"
  exit 1
fi

# Verify readers
[ "$("$BASE_BIN")" = "master" ] || { echo "FAIL: get-pr-base failed"; exit 1; }
[ "$("$BASE_BIN" "feature/test-branch")" = "master" ] || { echo "FAIL: get-pr-base with arg failed"; exit 1; }
[ "$("$NUM_BIN")" = "42" ] || { echo "FAIL: get-pr-number failed"; exit 1; }
[ "$("$NUM_BIN" "feature/test-branch")" = "42" ] || { echo "FAIL: get-pr-number with arg failed"; exit 1; }

echo "PASS: Test 1 (PR Found)"

# --------------------------------------------------------------------------
# Test 2: Case 1 with Base Ahead
# --------------------------------------------------------------------------
# Push a new commit to origin/master via another clone
CLONE_DIR="$TEMP_ROOT/clone"
git clone "$ORIGIN_DIR" "$CLONE_DIR"
cd "$CLONE_DIR"
git config user.name "Test User"
git config user.email "test@example.com"
echo "upstream change" >> upstream.txt
git add upstream.txt
git commit -m "upstream commit on master"
git push origin master

cd "$REPO_DIR"
"$SYNC_BIN"

[ "$(git config "branch.feature/test-branch.ahead")" = "true" ] || { echo "FAIL: ahead should be true"; exit 1; }

echo "PASS: Test 2 (Base Ahead Detection)"

# --------------------------------------------------------------------------
# Test 3: Case 3 & 4 - Auth Missing or Network Offline (Preserve Cache)
# --------------------------------------------------------------------------
# Simulate auth failure
cat << 'EOF' > "$MOCK_BIN/gh"
#!/usr/bin/env bash
echo "To authenticate, run: gh auth login" >&2
exit 1
EOF

"$SYNC_BIN"
# Config must still be preserved!
[ "$(git config "branch.feature/test-branch.base")" = "master" ] || { echo "FAIL: auth failure cleared base"; exit 1; }
[ "$(git config "branch.feature/test-branch.pr")" = "42" ] || { echo "FAIL: auth failure cleared pr"; exit 1; }
[ "$(git config "branch.feature/test-branch.ahead")" = "true" ] || { echo "FAIL: auth failure cleared ahead"; exit 1; }

# Simulate network timeout
cat << 'EOF' > "$MOCK_BIN/gh"
#!/usr/bin/env bash
echo "could not resolve host: github.com" >&2
exit 1
EOF

"$SYNC_BIN"
# Config must still be preserved!
[ "$(git config "branch.feature/test-branch.base")" = "master" ] || { echo "FAIL: network failure cleared base"; exit 1; }
[ "$(git config "branch.feature/test-branch.pr")" = "42" ] || { echo "FAIL: network failure cleared pr"; exit 1; }

echo "PASS: Test 3 (Preserve on Auth / Network Failure)"

# --------------------------------------------------------------------------
# Test 4: Case 2 - Definitely No PR
# --------------------------------------------------------------------------
cat << 'EOF' > "$MOCK_BIN/gh"
#!/usr/bin/env bash
echo 'no pull requests found for branch "feature/test-branch"' >&2
exit 1
EOF

"$SYNC_BIN"
# Config must be cleared
if git config "branch.feature/test-branch.base" 2>/dev/null; then
  echo "FAIL: base should be cleared"
  exit 1
fi
if git config "branch.feature/test-branch.pr" 2>/dev/null; then
  echo "FAIL: pr should be cleared"
  exit 1
fi
if git config "branch.feature/test-branch.ahead" 2>/dev/null; then
  echo "FAIL: ahead should be cleared"
  exit 1
fi

# get-pr-number should exit 1
if "$NUM_BIN" 2>/dev/null; then
  echo "FAIL: get-pr-number should exit 1 when not configured"
  exit 1
fi

echo "PASS: Test 4 (Cleared on Definitely No PR)"

# --------------------------------------------------------------------------
# Test 5: Reader Fallback Hierarchy
# --------------------------------------------------------------------------
# When branch.<name>.base is unset:
# Check origin/HEAD fallback
git symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/master
[ "$("$BASE_BIN")" = "master" ] || { echo "FAIL: get-pr-base origin/HEAD fallback failed"; exit 1; }

# Remove origin/HEAD
git update-ref -d refs/remotes/origin/HEAD
# Should fallback to master / main refs
[ "$("$BASE_BIN")" = "master" ] || { echo "FAIL: get-pr-base refs fallback failed"; exit 1; }

echo "PASS: Test 5 (Reader Fallback Hierarchy)"

# --------------------------------------------------------------------------
# Test 6: Hook Safety (Exit 0 always)
# --------------------------------------------------------------------------
# Broken gh command that crashes
cat << 'EOF' > "$MOCK_BIN/gh"
#!/usr/bin/env bash
kill -9 $$
EOF

set +e
"$SYNC_BIN"
status=$?
set -e
[ "$status" -eq 0 ] || { echo "FAIL: git-pr-sync did not exit 0 on crash"; exit 1; }

echo "PASS: Test 6 (Hook Safety)"

echo "ALL TESTS PASSED SUCCESSFULLY!"
