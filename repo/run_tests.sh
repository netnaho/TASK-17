#!/usr/bin/env bash
# SilverOak Operations Suite — test runner
#
# Usage:
#   ./run_tests.sh                          run everything (API tests require live stack)
#   SKIP_API_TESTS=1 ./run_tests.sh        skip API tests
#   API_BASE=http://host:8080 ./run_tests.sh  custom API base URL
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

red()   { printf "\033[31m%s\033[0m\n" "$*"; }
green() { printf "\033[32m%s\033[0m\n" "$*"; }
bold()  { printf "\033[1m%s\033[0m\n" "$*"; }
dim()   { printf "\033[2m%s\033[0m\n" "$*"; }

START_TIME=$(date +%s)
SUITE_FAILURES=0

run_suite() {
    local name="$1" script="$2"
    echo ""
    bold "── $name ──"
    if bash "$script"; then
        green "  ✓ $name passed"
    else
        red   "  ✗ $name FAILED"
        SUITE_FAILURES=$((SUITE_FAILURES + 1))
    fi
}

# Probe admin login; if demo accounts are absent (APP__RUN_SEEDS was false),
# restart backend-api using the dev overlay which sets APP__RUN_SEEDS=true and
# also enables in-memory rate limiting + compat signing for test stability.
ensure_seeded() {
    local base="${API_BASE:-http://localhost:8080}"
    local login_url="${base}/api/v1/auth/login"
    local resp compose_cmd

    resp=$(curl -sS --max-time 5 -X POST \
        -H 'content-type: application/json' \
        -d '{"email":"admin@silveroak.local","password":"ChangeMeNow!2025"}' \
        "$login_url" 2>/dev/null || true)

    if echo "$resp" | grep -q '"api_token"'; then
        green "  ✓ demo accounts present"
        return 0
    fi

    dim "  demo accounts missing (APP__RUN_SEEDS was false) — seeding now …"

    if docker compose version >/dev/null 2>&1; then
        compose_cmd="docker compose"
    elif command -v docker-compose >/dev/null 2>&1; then
        compose_cmd="docker-compose"
    else
        red "  docker compose not found — seed manually and re-run:"
        red "    docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d --no-build backend-api"
        return 1
    fi

    if [[ -f "${ROOT}/docker-compose.dev.yml" ]]; then
        $compose_cmd -f "${ROOT}/docker-compose.yml" -f "${ROOT}/docker-compose.dev.yml" \
            up -d --no-build backend-api
    else
        APP__RUN_SEEDS=true $compose_cmd -f "${ROOT}/docker-compose.yml" \
            up -d --no-build backend-api
    fi || { red "  failed to restart backend-api — check docker logs silveroak_api"; return 1; }

    # Wait up to 60 s for the API to come back healthy
    local i=0
    while [[ $i -lt 60 ]]; do
        curl -sf --max-time 3 "${base}/api/v1/health" >/dev/null 2>&1 && break
        sleep 2; i=$((i+2))
    done
    sleep 3  # allow seed SQL to commit

    resp=$(curl -sS --max-time 5 -X POST \
        -H 'content-type: application/json' \
        -d '{"email":"admin@silveroak.local","password":"ChangeMeNow!2025"}' \
        "$login_url" 2>/dev/null || true)

    if echo "$resp" | grep -q '"api_token"'; then
        green "  ✓ seeding complete"
    else
        red   "  seeding verification failed — check: docker logs silveroak_api"
        return 1
    fi
}

bold "════════════════════════════════════════"
bold "  SilverOak test runner  —  $(date '+%Y-%m-%d %H:%M:%S')"
bold "════════════════════════════════════════"
echo ""

# ── 1. Static analysis ────────────────────────────────────────────────────────
bold "── cargo check (workspace, excluding frontend-yew) ──"
if ! command -v cargo >/dev/null 2>&1; then
    dim "  (skipped: cargo not found)"
else
    if cargo check --workspace --exclude frontend-yew 2>&1; then
        green "  ✓ cargo check passed"
    else
        red "  ✗ cargo check FAILED"
        SUITE_FAILURES=$((SUITE_FAILURES + 1))
    fi
fi

# ── 2. Unit tests ─────────────────────────────────────────────────────────────
echo ""
bold "── unit tests ──"
if ! command -v cargo >/dev/null 2>&1; then
    dim "  (skipped: cargo not found)"
else
    UNIT_OUT=$(cargo test --workspace --exclude frontend-yew 2>&1)
    UNIT_RC=$?
    echo "$UNIT_OUT" | grep -E '(^test .+\.\.\.|FAILED|^test result)' || true
    if [[ $UNIT_RC -ne 0 ]] || echo "$UNIT_OUT" | grep -q "FAILED"; then
        red "  ✗ unit tests FAILED"
        SUITE_FAILURES=$((SUITE_FAILURES + 1))
    else
        green "  ✓ unit tests passed"
    fi
fi

# ── 3. API tests (live stack) ─────────────────────────────────────────────────
echo ""
if [[ "${SKIP_API_TESTS:-0}" == "1" ]]; then
    bold "── API tests SKIPPED  (SKIP_API_TESTS=1) ──"
    dim  "  To run: docker compose up --build, then ./run_tests.sh"
elif curl -sf --max-time 3 "${API_BASE:-http://localhost:8080}/api/v1/health" \
        >/dev/null 2>&1; then
    bold "── API tests  (stack at ${API_BASE:-http://localhost:8080}) ──"
    if ensure_seeded; then
        run_suite "Auth & Security"       API_tests/auth.sh
        run_suite "Requisitions"          API_tests/requisitions.sh
        run_suite "Orders & Cart"         API_tests/orders.sh
        run_suite "Master Data"           API_tests/master_data.sh
        run_suite "Phase 6 (Compliance)"  API_tests/phase6.sh
    else
        red "  ✗ cannot run API tests without demo seed data"
        SUITE_FAILURES=$((SUITE_FAILURES + 5))
    fi
else
    echo ""
    bold "── API tests SKIPPED — no live stack detected ──"
    dim  "  Start the stack:   docker compose up --build"
    dim  "  Then re-run:       ./run_tests.sh"
    dim  "  Custom endpoint:   API_BASE=http://host:8080 ./run_tests.sh"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))
echo ""
bold "════════════════════════════════════════"
if [[ "$SUITE_FAILURES" -eq 0 ]]; then
    green "  ALL SUITES PASSED  (${ELAPSED}s)"
else
    red   "  $SUITE_FAILURES SUITE(S) FAILED  (${ELAPSED}s)"
fi
bold "════════════════════════════════════════"
exit "$SUITE_FAILURES"
