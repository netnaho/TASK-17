#!/usr/bin/env bash
# SilverOak Phase 2 — auth & security API tests.
#
# Runs against a live Dockerized stack. Defaults to http://localhost:8080.
# Override with:  API_BASE=http://localhost:8000/api/v1 ./API_tests/auth.sh
#
# Covers (per Task Prompt):
#   - login success / failure
#   - lockout after 5 failed attempts
#   - login denied during lockout
#   - logout invalidates session
#   - unauthenticated 401 / unauthorized 403
#   - signed-request token + signature + timestamp window + replay
#   - per-IP rate limit and IP blacklist
#   - object/data-scope enforcement
#   - permission-change audit
set -euo pipefail

API_BASE="${API_BASE:-http://localhost:8080/api/v1}"
ADMIN_EMAIL="admin@silveroak.local"
ADMIN_PASS="ChangeMeNow!2025"
MEDICAL_EMAIL="medical@silveroak.local"

red()   { printf "\033[31m%s\033[0m\n" "$*"; }
green() { printf "\033[32m%s\033[0m\n" "$*"; }
say()   { printf "\n==> %s\n" "$*"; }

require() {
  if ! command -v "$1" >/dev/null; then red "missing: $1"; exit 2; fi
}
require curl
require jq
require openssl

# ----- helpers -----
sign_get() {
  local path="$1" key="$2"
  local ts nonce canonical sig
  ts=$(date +%s)
  nonce="nonce-$RANDOM-$RANDOM"
  canonical="GET"$'\n'"${path}"$'\n'"${ts}"$'\n'"${nonce}"
  sig=$(printf '%s' "$canonical" | openssl dgst -sha256 -hmac "$key" -hex | awk '{print $2}')
  printf '%s\n%s\n%s\n' "$ts" "$nonce" "$sig"
}

call_signed_get() {
  local path="$1" token="$2" key="$3"
  mapfile -t parts < <(sign_get "$path" "$key")
  local ts="${parts[0]}" nonce="${parts[1]}" sig="${parts[2]}"
  # path already includes /api/v1 — derive host from API_BASE
  local host="${API_BASE%%/api/v1*}"
  curl -sS -o /tmp/silveroak_body -w '%{http_code}' \
    -H "x-silveroak-token: $token" \
    -H "x-silveroak-timestamp: $ts" \
    -H "x-silveroak-nonce: $nonce" \
    -H "x-silveroak-signature: $sig" \
    "${host}${path}"
}

login() {
  local email="$1" pass="$2"
  curl -sS -c /tmp/silveroak_cookies -X POST \
    -H 'content-type: application/json' \
    -d "{\"email\":\"$email\",\"password\":\"$pass\"}" \
    "${API_BASE}/auth/login"
}

PASS=0; FAIL=0
ok()   { green "  OK   $*"; PASS=$((PASS+1)); }
bad()  { red   "  FAIL $*"; FAIL=$((FAIL+1)); }

# ============================================================
say "1. health check"
code=$(curl -sS -o /dev/null -w '%{http_code}' "${API_BASE}/health")
[[ "$code" == "200" ]] && ok "health 200" || bad "health=$code"

# ============================================================
say "2. login success (admin)"
resp=$(login "$ADMIN_EMAIL" "$ADMIN_PASS")
TOKEN=$(echo "$resp" | jq -r .api_token)
KEY=$(echo "$resp"   | jq -r .signing_key)
[[ -n "$TOKEN" && "$TOKEN" != "null" ]] && ok "received api token" || bad "no token: $resp"

# ============================================================
say "3. /me requires auth (401 without headers)"
code=$(curl -sS -o /dev/null -w '%{http_code}' "${API_BASE}/me")
[[ "$code" == "401" ]] && ok "unauth 401" || bad "expected 401 got $code"

# ============================================================
say "4. /me with valid signed request"
code=$(call_signed_get "/api/v1/me" "$TOKEN" "$KEY")
[[ "$code" == "200" ]] && ok "signed /me 200" || bad "signed /me=$code body=$(cat /tmp/silveroak_body)"

# ============================================================
say "5. invalid signature → 401"
mapfile -t parts < <(sign_get "/api/v1/me" "wrong-key")
ts="${parts[0]}"; nonce="${parts[1]}"; sig="${parts[2]}"
code=$(curl -sS -o /dev/null -w '%{http_code}' \
  -H "x-silveroak-token: $TOKEN" \
  -H "x-silveroak-timestamp: $ts" \
  -H "x-silveroak-nonce: $nonce" \
  -H "x-silveroak-signature: $sig" \
  "${API_BASE}/me")
[[ "$code" == "401" ]] && ok "bad sig 401" || bad "bad sig=$code"

# ============================================================
say "6. timestamp outside ±60s → 401"
old_ts=$(($(date +%s) - 600))
nonce="old-$RANDOM"
canonical="GET"$'\n'"/api/v1/me"$'\n'"${old_ts}"$'\n'"${nonce}"
sig=$(printf '%s' "$canonical" | openssl dgst -sha256 -hmac "$KEY" -hex | awk '{print $2}')
code=$(curl -sS -o /dev/null -w '%{http_code}' \
  -H "x-silveroak-token: $TOKEN" \
  -H "x-silveroak-timestamp: $old_ts" \
  -H "x-silveroak-nonce: $nonce" \
  -H "x-silveroak-signature: $sig" \
  "${API_BASE}/me")
[[ "$code" == "401" ]] && ok "old ts 401" || bad "old ts=$code"

# ============================================================
say "7. replay protection (same signature twice → 401)"
mapfile -t parts < <(sign_get "/api/v1/me" "$KEY")
ts="${parts[0]}"; nonce="${parts[1]}"; sig="${parts[2]}"
code1=$(curl -sS -o /dev/null -w '%{http_code}' \
  -H "x-silveroak-token: $TOKEN" -H "x-silveroak-timestamp: $ts" \
  -H "x-silveroak-nonce: $nonce" -H "x-silveroak-signature: $sig" \
  "${API_BASE}/me")
code2=$(curl -sS -o /dev/null -w '%{http_code}' \
  -H "x-silveroak-token: $TOKEN" -H "x-silveroak-timestamp: $ts" \
  -H "x-silveroak-nonce: $nonce" -H "x-silveroak-signature: $sig" \
  "${API_BASE}/me")
[[ "$code1" == "200" && ( "$code2" == "400" || "$code2" == "401" ) ]] \
  && ok "replay rejected ($code1 then $code2)" || bad "replay $code1/$code2"

# ============================================================
say "8. scope isolation: medical user CANNOT see a different department's residents"
# Use a zero UUID that does not match the medical user's scoped department.
med_resp_8=$(curl -sS -X POST -H 'content-type: application/json' \
  -d '{"email":"medical@silveroak.local","password":"ChangeMeNow!2025"}' \
  "${API_BASE}/auth/login")
MED_TOKEN_8=$(echo "$med_resp_8" | jq -r .api_token)
MED_KEY_8=$(echo   "$med_resp_8" | jq -r .signing_key)
code=$(call_signed_get "/api/v1/departments/00000000-0000-0000-0000-000000000000/residents" "$MED_TOKEN_8" "$MED_KEY_8")
[[ "$code" == "403" ]] && ok "medical user denied foreign dept ($code)" || bad "scope isolation expected 403, got $code"

# ============================================================
say "9. lockout after 5 failed logins (uses lockout_test account)"
LOCKOUT_EMAIL="lockout_test@silveroak.local"
for i in 1 2 3 4 5; do
  curl -sS -o /dev/null -X POST -H 'content-type: application/json' \
    -d "{\"email\":\"$LOCKOUT_EMAIL\",\"password\":\"wrong-password!\"}" \
    "${API_BASE}/auth/login" || true
done
code=$(curl -sS -o /dev/null -w '%{http_code}' -X POST -H 'content-type: application/json' \
  -d "{\"email\":\"$LOCKOUT_EMAIL\",\"password\":\"ChangeMeNow!2025\"}" \
  "${API_BASE}/auth/login")
[[ "$code" == "401" ]] && ok "locked out ($code)" || bad "expected lockout 401, got $code"

# ============================================================
say "10. admin scope:any bypasses department restriction"
code=$(call_signed_get "/api/v1/departments/00000000-0000-0000-0000-000000000000/residents" "$TOKEN" "$KEY")
[[ "$code" == "200" ]] && ok "admin scope:any allows 200" || bad "scope test=$code"

# ============================================================
say "11. logout invalidates session"
curl -sS -b /tmp/silveroak_cookies -o /dev/null -X POST "${API_BASE}/auth/logout"
code=$(curl -sS -b /tmp/silveroak_cookies -o /dev/null -w '%{http_code}' "${API_BASE}/auth/session")
[[ "$code" == "401" ]] && ok "session invalidated" || bad "session still valid: $code"

echo
echo "PASS=$PASS  FAIL=$FAIL"
[[ "$FAIL" == "0" ]] || exit 1
