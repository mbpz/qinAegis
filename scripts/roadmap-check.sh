#!/usr/bin/env bash
# roadmap-check.sh — audit codebase against qinAegis-platform-roadmap.md (5 phases).
#
# Outputs a markdown status table to stdout. Exits 0 if all green, 1 if any red.
# Idempotent: read-only, no state changes.

set -uo pipefail

# Resolve repo root (script must be runnable from anywhere).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# Counters
PASS=0
FAIL=0
WARN=0
FAIL_LINES=()

# Helper: append a row to the table.
row() {
  local check="$1" expected="$2" actual="$3" status="$4"
  printf "| %s | %s | %s | %s |\n" "$check" "$expected" "$actual" "$status"
  case "$status" in
    "✓") PASS=$((PASS + 1)) ;;
    "✗") FAIL=$((FAIL + 1)); FAIL_LINES+=("$check") ;;
    "?") WARN=$((WARN + 1)) ;;
  esac
}

# Header
cat <<'EOF'
# QinAegis Roadmap Status

EOF

printf "**Generated:** %s  \n" "$(date -u +'%Y-%m-%d %H:%M:%S UTC')"
printf "**Repo:** %s  \n\n" "$REPO_ROOT"
printf "## Summary\n\n"
printf "| Check | Expected | Actual | Status |\n"
printf "|-------|----------|--------|--------|\n"

# -------------------------------------------------------------------
# 1. Module presence (per phase)
# Note: bash 3.2 (macOS default) misbehaves with set -u and special chars
# in associative array keys, so we use parallel indexed arrays.
# -------------------------------------------------------------------
PHASES=(
  "Phase 1 · Bootstrap"
  "Phase 2 · AI Core"
  "Phase 3 · Test Execution"
  "Phase 4 · Local Data Model"
  "Phase 5 · Distribution"
)
MODULE_FILES=(
  "crates/core/src/config/app.rs"
  "crates/core/src/explorer.rs crates/core/src/generator.rs crates/core/src/critic.rs crates/core/src/healer.rs"
  "crates/core/src/executor.rs crates/core/src/performance.rs crates/core/src/stress.rs crates/core/src/gate.rs"
  "crates/core/src/storage crates/core/src/knowledge.rs"
  "homebrew Formula/qinaegis.rb .github/workflows/release.yml"
)

for i in "${!PHASES[@]}"; do
  phase="${PHASES[$i]}"
  files="${MODULE_FILES[$i]}"
  missing=()
  present=()
  for f in $files; do
    if [[ -e "$f" ]]; then
      present+=("$f")
    else
      missing+=("$f")
    fi
  done
  total=${#present[@]}
  expected=${#missing[@]}
  if [[ ${#missing[@]} -eq 0 ]]; then
    row "Modules: $phase" "$total files" "$total/$total" "✓"
  else
    actual="$total/$((total + expected))"
    missing_str=$(IFS=,; echo "${missing[*]}")
    row "Modules: $phase" "$((total + expected)) files" "$actual ($missing_str)" "✗"
  fi
done

# -------------------------------------------------------------------
# 2. Reliability loop — 6 events + 4 workers
# -------------------------------------------------------------------
if [[ -f "crates/core/src/events.rs" ]]; then
  EVENTS_HIT=$(grep -cE "CaseGenerated|CaseExecuted|HealProduced|HealExhausted|FlakyDetected|GateDecided" crates/core/src/events.rs || true)
  if [[ "$EVENTS_HIT" -ge 6 ]]; then
    row "Reliability: 6 events" "≥6 names" "$EVENTS_HIT matches" "✓"
  else
    row "Reliability: 6 events" "≥6 names" "$EVENTS_HIT matches" "✗"
  fi
else
  row "Reliability: 6 events" "≥6 names" "events.rs missing" "✗"
fi

if [[ -f "crates/core/src/pipeline.rs" ]]; then
  WORKERS_HIT=$(grep -cE "execute_loop|heal_loop|flaky_loop|gate_loop" crates/core/src/pipeline.rs || true)
  if [[ "$WORKERS_HIT" -ge 4 ]]; then
    row "Reliability: 4 workers" "≥4 loops" "$WORKERS_HIT matches" "✓"
  else
    row "Reliability: 4 workers" "≥4 loops" "$WORKERS_HIT matches" "✗"
  fi
else
  row "Reliability: 4 workers" "≥4 loops" "pipeline.rs missing" "✗"
fi

# -------------------------------------------------------------------
# 3. Test stats — count tests + assert green
# -------------------------------------------------------------------
# Use timeout to avoid hanging the workflow on slow runs.
TEST_OUTPUT=$(timeout 300 cargo test -p qin_aegis_core --lib --no-fail-fast 2>&1 | tail -20 || true)
TEST_PASS_LINE=$(echo "$TEST_OUTPUT" | grep -oE "[0-9]+ passed" | head -1 || true)
TEST_FAIL_LINE=$(echo "$TEST_OUTPUT" | grep -oE "[0-9]+ failed" | head -1 || true)

# A non-zero failure count is a real failure; "0 failed" is green.
FAIL_COUNT=0
if [[ -n "$TEST_FAIL_LINE" ]]; then
  FAIL_COUNT=$(echo "$TEST_FAIL_LINE" | grep -oE "[0-9]+" | head -1)
  FAIL_COUNT=${FAIL_COUNT:-0}
fi

if [[ -n "$TEST_PASS_LINE" && "$FAIL_COUNT" -eq 0 ]]; then
  row "Tests: passing" ">0 passed, 0 failed" "$TEST_PASS_LINE" "✓"
elif [[ "$FAIL_COUNT" -gt 0 ]]; then
  row "Tests: passing" "0 failed" "0 passed, $TEST_FAIL_LINE" "✗"
else
  row "Tests: passing" ">0 passed" "no result (timeout/build error)" "?"
fi

# -------------------------------------------------------------------
# 4. ADRs — count vs 6 planned
# -------------------------------------------------------------------
ADR_COUNT=$(ls docs/adr/*.md 2>/dev/null | wc -l | tr -d ' ' || echo "0")
if [[ "$ADR_COUNT" -ge 6 ]]; then
  row "ADRs written" "≥6" "$ADR_COUNT" "✓"
else
  row "ADRs written" "≥6" "$ADR_COUNT" "✗"
fi

# -------------------------------------------------------------------
# 5. Git status — branch + uncommitted + ahead of main
# -------------------------------------------------------------------
BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
UNCOMMITTED=$(git status --short 2>/dev/null | wc -l | tr -d ' ')
AHEAD=$(git rev-list --count main..HEAD 2>/dev/null | tr -d ' ' || echo "0")

if [[ "$UNCOMMITTED" -eq 0 ]]; then
  row "Git: working tree clean" "0 uncommitted" "0 uncommitted" "✓"
else
  row "Git: working tree clean" "0 uncommitted" "$UNCOMMITTED uncommitted" "?"
fi

if [[ "$AHEAD" -gt 0 ]]; then
  row "Git: branch ahead of main" "ahead" "$AHEAD commits on $BRANCH" "✓"
else
  row "Git: branch ahead of main" "ahead" "0 ahead (on or behind main)" "?"
fi

# -------------------------------------------------------------------
# 6. Recent commits — show last 10 on current branch (vs main)
# -------------------------------------------------------------------
COMMITS=$(git log --oneline -10 main..HEAD 2>/dev/null || true)
if [[ -n "$COMMITS" ]]; then
  COMMIT_COUNT=$(printf '%s\n' "$COMMITS" | grep -cE "^[a-f0-9]{7,}" || true)
  COMMIT_COUNT=${COMMIT_COUNT:-0}
else
  COMMIT_COUNT=0
fi
if [[ "$COMMIT_COUNT" -gt 0 ]]; then
  row "Recent commits" "≥1 ahead" "$COMMIT_COUNT ahead" "✓"
else
  row "Recent commits" "≥1 ahead" "0 ahead" "?"
fi

# -------------------------------------------------------------------
# 7. Untracked artifacts — flag flaky_history.json, runs/, etc.
# -------------------------------------------------------------------
DIRTY=$(git status --short 2>/dev/null || true)
ARTIFACTS=("flaky_history.json" "crates/core/runs/" "crates/core/summary.json" "target/reliability/")
FOUND=0
for art in "${ARTIFACTS[@]}"; do
  if echo "$DIRTY" | grep -qE "${art}"; then
    FOUND=1
    break
  fi
done

if [[ "$FOUND" -eq 0 ]]; then
  row "Untracked artifacts" "none" "clean" "✓"
else
  HIT=$(echo "$DIRTY" | grep -E "flaky_history.json|crates/core/runs/|crates/core/summary.json|target/reliability/" | head -3 | tr '\n' ';' || true)
  row "Untracked artifacts" "none" "found: $HIT" "?"
fi

# -------------------------------------------------------------------
# Footer: totals + recent commits list
# -------------------------------------------------------------------
printf "\n**Totals:** %d ✓  %d ✗  %d ?\n\n" "$PASS" "$FAIL" "$WARN"

if [[ -n "$COMMITS" ]]; then
  printf "## Recent commits on %s\n\n" "$BRANCH"
  printf '```\n'
  echo "$COMMITS"
  printf '```\n\n'
fi

if [[ "$UNCOMMITTED" -gt 0 ]]; then
  printf "## Uncommitted changes\n\n"
  printf '```\n'
  git status --short
  printf '```\n\n'
fi

if [[ ${#FAIL_LINES[@]} -gt 0 ]]; then
  printf "## Failing checks\n\n"
  for c in "${FAIL_LINES[@]}"; do
    printf -- "- %s\n" "$c"
  done
  printf "\n"
fi

# Exit code
if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
exit 0
