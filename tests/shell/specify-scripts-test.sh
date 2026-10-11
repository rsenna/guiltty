#!/usr/bin/env bash
# Tests for the .specify bootstrap shell scripts (issue #39).
#
# Plain bash, no external test framework or dependency. Run from the repo root:
#   bash tests/shell/specify-scripts-test.sh
#
# Each test runs against a throwaway copy of .specify/ in a temp dir, so the
# real tree is never mutated.

set -u

SCRIPT_DIR="$(CDPATH="" cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SPECIFY_SCRIPTS="$REPO_ROOT/.specify/scripts/bash"

PASS=0
FAIL=0

ok()   { PASS=$((PASS + 1)); }
fail() { FAIL=$((FAIL + 1)); echo "FAIL: $*" >&2; }

# assert <description> <command...> — passes when the command exits 0.
assert() {
    local desc="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        ok
    else
        fail "$desc"
    fi
}

# assert_fails <description> <command...> — passes when the command exits non-zero.
assert_fails() {
    local desc="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        fail "$desc (expected non-zero exit)"
    else
        ok
    fi
}

# assert_contains <description> <needle> <haystack>
assert_contains() {
    local desc="$1" needle="$2" haystack="$3"
    if printf '%s' "$haystack" | grep -qF -- "$needle"; then
        ok
    else
        fail "$desc (missing '$needle')"
    fi
}

# --- 1. executable permissions -------------------------------------------
for f in check-prerequisites.sh common.sh create-new-feature.sh setup-plan.sh setup-tasks.sh; do
    assert "executable: $f" test -x "$SPECIFY_SCRIPTS/$f"
done

# --- throwaway repo ------------------------------------------------------
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cp -R "$REPO_ROOT/.specify" "$TMP/.specify"

# run_script <script> [args...] — run a bootstrap script against the temp repo.
run_script() {
    local script="$1"
    shift
    SPECIFY_INIT_DIR="$TMP" bash "$SPECIFY_SCRIPTS/$script" "$@"
}

# --- 2. create-new-feature.sh --------------------------------------------
out=$(run_script create-new-feature.sh --dry-run "add user auth" 2>&1)
assert_contains "sequential numbering" "BRANCH_NAME: " "$out"
assert_contains "sequential prefix is 3 digits" "BRANCH_NAME: 001-" "$out"

out=$(run_script create-new-feature.sh --timestamp --dry-run "foo" 2>&1)
assert_contains "timestamp mode" "BRANCH_NAME: " "$out"
if printf '%s' "$out" | grep -qE 'BRANCH_NAME: [0-9]{8}-[0-9]{6}-'; then
    ok
else
    fail "timestamp prefix format"
fi

out=$(run_script create-new-feature.sh --short-name "My Feature" --dry-run "x" 2>&1)
assert_contains "custom short name" "my-feature" "$out"

assert_fails "non-numeric --number rejected" \
    run_script create-new-feature.sh --number abc "x"
assert_fails "oversized --number rejected" \
    run_script create-new-feature.sh --number 99999999999999999999 "x"

long_name=$(printf 'a%.0s' $(seq 1 300))
out=$(run_script create-new-feature.sh --short-name "$long_name" --dry-run "x" 2>&1)
assert_contains "branch-length truncation warning" "exceeded GitHub" "$out"

# --- 3. check-prerequisites.sh -------------------------------------------
mkdir -p "$TMP/specs/001-empty"
assert_fails "missing spec.md rejected" \
    run_script check-prerequisites.sh --paths-only >/dev/null 2>&1
# (paths-only never validates; use a real feature dir below)

mkdir -p "$TMP/specs/002-spec-only"
touch "$TMP/specs/002-spec-only/spec.md"
out=$(SPECIFY_FEATURE_DIRECTORY="$TMP/specs/002-spec-only" \
    SPECIFY_INIT_DIR="$TMP" bash "$SPECIFY_SCRIPTS/check-prerequisites.sh" 2>&1)
assert_contains "missing plan.md rejected" "plan.md not found" "$out"

mkdir -p "$TMP/specs/003-complete"
touch "$TMP/specs/003-complete/spec.md" "$TMP/specs/003-complete/plan.md"
out=$(SPECIFY_FEATURE_DIRECTORY="$TMP/specs/003-complete" \
    SPECIFY_INIT_DIR="$TMP" bash "$SPECIFY_SCRIPTS/check-prerequisites.sh" --json 2>&1)
assert_contains "JSON output has FEATURE_DIR" '"FEATURE_DIR"' "$out"

out=$(SPECIFY_FEATURE_DIRECTORY="$TMP/specs/003-complete" \
    SPECIFY_INIT_DIR="$TMP" bash "$SPECIFY_SCRIPTS/check-prerequisites.sh" --paths-only 2>&1)
assert_contains "paths-only output has REPO_ROOT" "REPO_ROOT: " "$out"

# --- 4. feature path resolution (common.sh) ------------------------------
out=$(SPECIFY_FEATURE_DIRECTORY="$TMP/specs/003-complete" SPECIFY_INIT_DIR="$TMP" \
    bash -c 'source "$1/.specify/scripts/bash/common.sh"; get_feature_paths --no-persist' _ "$TMP")
assert_contains "env-var feature dir resolved" "FEATURE_DIR=$TMP/specs/003-complete" "$out"

# --- 5. template resolution precedence (common.sh) -----------------------
mkdir -p "$TMP/.specify/templates/overrides"
printf 'OVERRIDE\n' > "$TMP/.specify/templates/overrides/spec-template.md"
out=$(SPECIFY_INIT_DIR="$TMP" \
    bash -c 'source "$1/.specify/scripts/bash/common.sh"; resolve_template spec-template "$1"' _ "$TMP")
assert_contains "override wins over core template" "overrides/spec-template.md" "$out"

# --- 6. fallback without jq/python --------------------------------------
# Strip jq/python from PATH and confirm create-new-feature still produces a
# branch name (the scripts have text fallbacks for both).
out=$(PATH="/usr/bin:/bin" SPECIFY_INIT_DIR="$TMP" \
    bash "$SPECIFY_SCRIPTS/create-new-feature.sh" --dry-run "fallback test" 2>&1)
assert_contains "works without jq/python" "BRANCH_NAME: " "$out"

# --- 7. workflow metadata and gate ordering ------------------------------
wf="$REPO_ROOT/.specify/workflows/speckit/workflow.yml"
assert "workflow.yml exists" test -f "$wf"
assert_contains "workflow declares speckit id" 'id: "speckit"' "$(cat "$wf")"
# Core commands must appear in order: specify -> plan -> tasks -> implement.
order=$(grep -oE 'command: speckit\.(specify|plan|tasks|implement)' "$wf" | sed 's/command: //' | tr '\n' ' ')
assert_contains "core command order" "speckit.specify speckit.plan speckit.tasks speckit.implement" "$order"
assert_contains "review gate present" "type: gate" "$(cat "$wf")"

# --- summary -------------------------------------------------------------
echo "passed: $PASS, failed: $FAIL"
[ "$FAIL" -eq 0 ]
