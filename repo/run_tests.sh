#!/usr/bin/env bash
# SilverOak Operations Suite — Docker-contained test runner.
#
# The ONLY host dependency is Docker (with the Compose plugin).  Every check,
# every unit test, and every API script executes inside a container — there
# is no fallback to a local cargo / jq / openssl / python toolchain.
#
# Pipeline:
#   1. bring the stack up (postgres, backend-api, worker, proxy)  — dev overlay
#   2. wait for backend-api health
#   3. ensure demo seed data is loaded
#   4. build the tests-runner image  (rust + bash + curl + jq + openssl + py)
#   5. run  cargo check --workspace --exclude frontend-yew   inside tests-runner
#   6. run  cargo test  --workspace --exclude frontend-yew   inside tests-runner
#   7. run  API_tests/*.sh  (auth, requisitions, orders, master_data, phase6,
#                            coverage_extra)                 inside tests-runner
#   8. aggregate results and exit non-zero on any failure
#
# Flags:
#   SKIP_API_TESTS=1   — skip step 7 (unit tests still run)
#   SKIP_BOOT=1        — assume the stack is already up (don't attempt to start)
#   KEEP_STACK=1       — leave the stack running after tests (default: tears
#                        down the tests-runner container only; services stay up)
#   REBUILD_TESTS=1    — force `docker compose build tests`
#
# Exit codes: 0 = all suites passed, N = number of failed suites.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

# ── colours / helpers ─────────────────────────────────────────────────────────
red()   { printf "\033[31m%s\033[0m\n" "$*"; }
green() { printf "\033[32m%s\033[0m\n" "$*"; }
bold()  { printf "\033[1m%s\033[0m\n" "$*"; }
dim()   { printf "\033[2m%s\033[0m\n" "$*"; }
die()   { red "ERROR: $*"; exit 1; }

START_TIME=$(date +%s)
SUITE_FAILURES=0
FAILED_SUITES=()

# ── sanity: only Docker required on the host ──────────────────────────────────
if ! command -v docker >/dev/null 2>&1; then
    die "docker not found on PATH — this runner is Docker-only.  Install Docker then re-run."
fi

if docker compose version >/dev/null 2>&1; then
    COMPOSE=(docker compose)
elif command -v docker-compose >/dev/null 2>&1; then
    # Legacy v1 CLI — supported because the README mandates the exact
    # `docker-compose up` command string.
    COMPOSE=(docker-compose)
else
    die "docker compose plugin not found.  Install it (or docker-compose v1) and re-run."
fi

COMPOSE_FILES=(-f docker-compose.yml -f docker-compose.dev.yml -f docker-compose.tests.yml)
compose() { "${COMPOSE[@]}" "${COMPOSE_FILES[@]}" "$@"; }

# ── clean-up trap ─────────────────────────────────────────────────────────────
cleanup() {
    local rc=$?
    # Always remove the ephemeral tests-runner container (started with --rm in
    # most calls, but one-off failures can leak `silveroak_tests`).
    docker rm -f silveroak_tests >/dev/null 2>&1 || true
    if [[ "${KEEP_STACK:-0}" == "1" ]]; then
        dim "  KEEP_STACK=1 → leaving stack running"
    fi
    exit "$rc"
}
trap cleanup EXIT INT TERM

# ── banner ────────────────────────────────────────────────────────────────────
bold "════════════════════════════════════════"
bold "  SilverOak Docker test runner  —  $(date '+%Y-%m-%d %H:%M:%S')"
bold "════════════════════════════════════════"
dim  "  compose: ${COMPOSE[*]}"
dim  "  files:   docker-compose.yml + dev + tests overlays"
echo ""

# ── 1. bring the stack up ─────────────────────────────────────────────────────
if [[ "${SKIP_BOOT:-0}" == "1" ]]; then
    bold "── stack boot SKIPPED (SKIP_BOOT=1) ──"
else
    bold "── starting SilverOak stack (dev overlay, seeds enabled) ──"
    compose up -d --build postgres backend-api backend-worker frontend proxy \
        || die "failed to bring up the stack (see: ${COMPOSE[*]} ${COMPOSE_FILES[*]} logs)"
    green "  ✓ docker compose up -d complete"
fi

# ── 2. wait for backend-api health ────────────────────────────────────────────
bold "── waiting for backend-api health (up to 90 s) ──"
i=0
until docker exec silveroak_api wget -qO- http://localhost:8080/api/v1/health >/dev/null 2>&1; do
    if (( i >= 90 )); then
        red "  ✗ backend-api did not become healthy within 90 s"
        dim "  Inspect:  ${COMPOSE[*]} ${COMPOSE_FILES[*]} logs backend-api"
        die "stack not healthy"
    fi
    sleep 2; i=$((i+2))
    printf '.'
done
echo
green "  ✓ backend-api healthy"

# ── 3. ensure demo seed data is present ───────────────────────────────────────
bold "── verifying demo seed data ──"
LOGIN_JSON=$(docker exec silveroak_api wget -qO- \
    --post-data='{"email":"admin@silveroak.local","password":"ChangeMeNow!2025"}' \
    --header='content-type: application/json' \
    http://localhost:8080/api/v1/auth/login 2>/dev/null || true)

if echo "$LOGIN_JSON" | grep -q '"api_token"'; then
    green "  ✓ demo accounts present"
else
    dim "  demo accounts missing — restarting backend-api with APP__RUN_SEEDS=true"
    compose up -d --no-build backend-api
    i=0
    until docker exec silveroak_api wget -qO- http://localhost:8080/api/v1/health >/dev/null 2>&1; do
        if (( i >= 60 )); then
            die "backend-api did not come back healthy after seed restart"
        fi
        sleep 2; i=$((i+2))
    done
    # Grace period for seed SQL commit.
    sleep 3
    LOGIN_JSON=$(docker exec silveroak_api wget -qO- \
        --post-data='{"email":"admin@silveroak.local","password":"ChangeMeNow!2025"}' \
        --header='content-type: application/json' \
        http://localhost:8080/api/v1/auth/login 2>/dev/null || true)
    if echo "$LOGIN_JSON" | grep -q '"api_token"'; then
        green "  ✓ demo accounts seeded"
    else
        die "seed verification failed; check docker logs silveroak_api"
    fi
fi

# ── 4. build the tests-runner image ───────────────────────────────────────────
bold "── building tests-runner image ──"
if [[ "${REBUILD_TESTS:-0}" == "1" ]]; then
    compose build tests || die "failed to build tests-runner image"
else
    # Compose only rebuilds when the Dockerfile or build context changes.
    compose build tests || die "failed to build tests-runner image"
fi
green "  ✓ tests-runner image ready"

# Short helper: run a command inside the tests-runner container.  All tests go
# through this single entry point — there is no path that shells out to host
# cargo / curl / jq / openssl / python.
in_tests() {
    compose run --rm --no-deps tests "$*"
}

# Track per-suite outcomes.
note_result() {
    local name="$1" rc="$2"
    if [[ "$rc" -eq 0 ]]; then
        green "  ✓ ${name} passed"
    else
        red   "  ✗ ${name} FAILED (exit ${rc})"
        SUITE_FAILURES=$((SUITE_FAILURES + 1))
        FAILED_SUITES+=("$name")
    fi
}

# ── 5. cargo check (inside tests-runner) ──────────────────────────────────────
echo
bold "── cargo check (workspace, excluding frontend-yew) ──"
set +e
in_tests "cargo check --workspace --exclude frontend-yew --locked || cargo check --workspace --exclude frontend-yew"
rc=$?
set -e
note_result "cargo check" "$rc"

# ── 6. cargo test ─────────────────────────────────────────────────────────────
echo
bold "── cargo test (workspace, excluding frontend-yew) ──"
set +e
in_tests "cargo test --workspace --exclude frontend-yew --no-fail-fast"
rc=$?
set -e
note_result "cargo test" "$rc"

# ── 7. API tests ──────────────────────────────────────────────────────────────
echo
if [[ "${SKIP_API_TESTS:-0}" == "1" ]]; then
    bold "── API tests SKIPPED (SKIP_API_TESTS=1) ──"
else
    bold "── API tests (inside tests-runner → http://backend-api:8080/api/v1) ──"
    run_api_suite() {
        local name="$1" script="$2"
        echo
        bold "── ${name} ──"
        set +e
        in_tests "API_BASE=http://backend-api:8080/api/v1 bash ${script}"
        local rc=$?
        set -e
        note_result "${name}" "${rc}"
    }
    run_api_suite "Auth & Security"          "API_tests/auth.sh"
    run_api_suite "Requisitions"             "API_tests/requisitions.sh"
    run_api_suite "Orders & Cart"            "API_tests/orders.sh"
    run_api_suite "Master Data"              "API_tests/master_data.sh"
    run_api_suite "Phase 6 (Compliance)"     "API_tests/phase6.sh"
    run_api_suite "Endpoint coverage extras" "API_tests/coverage_extra.sh"
fi

# ── 8. Fullstack E2E (Playwright + real browser through proxy) ────────────────
echo
if [[ "${SKIP_E2E_TESTS:-0}" == "1" ]]; then
    bold "── E2E tests SKIPPED (SKIP_E2E_TESTS=1) ──"
else
    bold "── building e2e-runner image ──"
    if compose build e2e; then
        green "  ✓ e2e-runner image ready"
        bold "── E2E tests (Playwright → http://proxy:80) ──"
        set +e
        compose run --rm e2e playwright test
        rc=$?
        set -e
        note_result "Fullstack E2E" "$rc"
    else
        red "  ✗ e2e-runner image build FAILED"
        SUITE_FAILURES=$((SUITE_FAILURES + 1))
        FAILED_SUITES+=("Fullstack E2E (build failed)")
    fi
fi

# ── summary ───────────────────────────────────────────────────────────────────
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))
echo
bold "════════════════════════════════════════"
if [[ "$SUITE_FAILURES" -eq 0 ]]; then
    green "  ALL SUITES PASSED  (${ELAPSED}s, Docker-contained)"
else
    red   "  ${SUITE_FAILURES} SUITE(S) FAILED  (${ELAPSED}s)"
    for name in "${FAILED_SUITES[@]}"; do
        red "    ✗ ${name}"
    done
fi
bold "════════════════════════════════════════"

# Graceful: exit with the count of failures (capped at 125 for POSIX)
EXIT_CODE="$SUITE_FAILURES"
if [[ "$EXIT_CODE" -gt 125 ]]; then EXIT_CODE=125; fi
exit "$EXIT_CODE"
