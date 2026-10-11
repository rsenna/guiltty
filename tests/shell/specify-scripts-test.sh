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

out=$(run_script create-new-feature.sh --short-name -n --dry-run "x" 2>&1)
assert_contains "short name beginning with -n is not consumed by echo" "001-n" "$out"

assert_fails "empty short name rejected" \
    run_script create-new-feature.sh --short-name '---' "x"
assert_fails "explicitly empty --short-name rejected" \
    run_script create-new-feature.sh --short-name '' "Add user authentication"
assert_fails "non-ASCII-only slug rejected" \
    run_script create-new-feature.sh --short-name '東京' "x"
assert_fails "non-numeric --number rejected" \
    run_script create-new-feature.sh --number abc "x"
assert_fails "oversized --number rejected" \
    run_script create-new-feature.sh --number 99999999999999999999 "x"

long_name=$(printf 'a%.0s' $(seq 1 300))
out=$(run_script create-new-feature.sh --short-name "$long_name" --dry-run "x" 2>&1)
assert_contains "branch-length truncation warning" "exceeded GitHub" "$out"

# Missing/empty spec templates must fail before creating a feature directory.
spec_template="$TMP/.specify/templates/spec-template.md"
cp "$spec_template" "$TMP/spec-template.md.backup"
rm "$spec_template"
if SPECIFY_INIT_DIR="$TMP" bash "$SPECIFY_SCRIPTS/create-new-feature.sh" \
    --number 998 --short-name missing-template "Missing template" >"$TMP/template-error.out" 2>&1; then
    fail "missing spec template rejected (expected non-zero exit)"
else
    ok
fi
assert_contains "missing template error is clear" "Spec template not found" "$(cat "$TMP/template-error.out")"
assert "missing template creates no feature directory" test ! -e "$TMP/specs/998-missing-template"
mv "$TMP/spec-template.md.backup" "$spec_template"

# A successful creation persists feature context, and that generated state stays
# out of git so it cannot dirty a developer's checkout.
if SPECIFY_INIT_DIR="$TMP" bash "$SPECIFY_SCRIPTS/create-new-feature.sh" \
    --number 997 --short-name smoke-feature "Smoke feature" >"$TMP/create-feature.out" 2>&1; then
    ok
else
    fail "successful feature creation exits zero"
fi
assert "successful feature creation writes spec.md" test -s "$TMP/specs/997-smoke-feature/spec.md"
assert "successful feature creation persists feature.json" test -s "$TMP/.specify/feature.json"
if python3 -c 'import json,sys; assert json.load(open(sys.argv[1]))["feature_directory"] == "specs/997-smoke-feature"' "$TMP/.specify/feature.json"; then
    ok
else
    fail "feature.json stores a relative feature_directory"
fi
assert "feature.json is gitignored" git -C "$REPO_ROOT" check-ignore -q .specify/feature.json

# --- 3. check-prerequisites.sh -------------------------------------------
mkdir -p "$TMP/specs/001-empty"
if SPECIFY_FEATURE_DIRECTORY="$TMP/specs/001-empty" SPECIFY_INIT_DIR="$TMP" \
    bash "$SPECIFY_SCRIPTS/check-prerequisites.sh" >"$TMP/missing-spec.out" 2>&1; then
    fail "missing spec.md rejected (expected non-zero exit)"
else
    ok
fi
out=$(cat "$TMP/missing-spec.out")
assert_contains "missing spec.md error text" "spec.md not found" "$out"

mkdir -p "$TMP/specs/002-spec-only"
touch "$TMP/specs/002-spec-only/spec.md"
if SPECIFY_FEATURE_DIRECTORY="$TMP/specs/002-spec-only" SPECIFY_INIT_DIR="$TMP" \
    bash "$SPECIFY_SCRIPTS/check-prerequisites.sh" >"$TMP/missing-plan.out" 2>&1; then
    fail "missing plan.md rejected (expected non-zero exit)"
else
    ok
fi
out=$(cat "$TMP/missing-plan.out")
assert_contains "missing plan.md error text" "plan.md not found" "$out"

mkdir -p "$TMP/specs/003-complete"
touch "$TMP/specs/003-complete/spec.md" "$TMP/specs/003-complete/plan.md"
out=$(SPECIFY_FEATURE_DIRECTORY="$TMP/specs/003-complete" \
    SPECIFY_INIT_DIR="$TMP" bash "$SPECIFY_SCRIPTS/check-prerequisites.sh" --json 2>/dev/null)
if printf '%s\n' "$out" | python3 -c 'import json,sys; data=json.load(sys.stdin); assert isinstance(data,dict) and "FEATURE_DIR" in data'; then
    ok
else
    fail "check-prerequisites --json emits valid JSON with FEATURE_DIR"
fi

out=$(SPECIFY_FEATURE_DIRECTORY="$TMP/specs/003-complete" \
    SPECIFY_INIT_DIR="$TMP" bash "$SPECIFY_SCRIPTS/check-prerequisites.sh" --paths-only 2>&1)
assert_contains "paths-only output has REPO_ROOT" "REPO_ROOT: " "$out"

# setup-tasks must direct a feature with both files missing to specify first.
mkdir -p "$TMP/specs/004-no-prerequisites"
if SPECIFY_FEATURE_DIRECTORY="$TMP/specs/004-no-prerequisites" SPECIFY_INIT_DIR="$TMP" \
    bash "$SPECIFY_SCRIPTS/setup-tasks.sh" >"$TMP/tasks-prerequisite.out" 2>&1; then
    fail "setup-tasks rejects missing spec before plan (expected non-zero exit)"
else
    ok
fi
assert_contains "setup-tasks asks for specify before plan" "spec.md not found" "$(cat "$TMP/tasks-prerequisite.out")"


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

# --- 6. JSON output (jq or printf fallback) ------------------------------
# --json --dry-run must emit a JSON object with BRANCH_NAME whether jq is
# available or the printf-based fallback is used. (A full no-jq/no-python3
# run is environment-dependent — jq is exposed as a shell builtin here — so
# this asserts the output contract rather than the specific code path.)
out=$(SPECIFY_INIT_DIR="$TMP" \
    bash "$SPECIFY_SCRIPTS/create-new-feature.sh" --json --dry-run "fallback test" 2>/dev/null)
if printf '%s\n' "$out" | python3 -c 'import json,sys; data=json.load(sys.stdin); assert isinstance(data,dict) and isinstance(data.get("BRANCH_NAME"),str) and data["BRANCH_NAME"]'; then
    ok
else
    fail "create-new-feature --json emits valid JSON with BRANCH_NAME"
fi

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
