#!/usr/bin/env bash
# SilverOak — Additional endpoint coverage tests.
#
# This suite closes the gap reported in .tmp/test_coverage_and_readme_audit_report.md
# by issuing real signed HTTP requests against every endpoint that was not
# previously exercised in auth.sh, requisitions.sh, orders.sh, master_data.sh,
# or phase6.sh.  All requests flow through the real Actix routes (no mocks).
#
# Endpoints covered here (one assertion minimum each):
#   GET    /api/v1/admin/blacklist
#   POST   /api/v1/admin/blacklist
#   DELETE /api/v1/admin/blacklist/{ip}
#   POST   /api/v1/admin/users/grant-role
#   POST   /api/v1/auth/recovery/request
#   GET    /api/v1/requisitions/mine/list
#   GET    /api/v1/requisitions/{id}/audit
#   POST   /api/v1/orders/cart/coupon-remove
#   GET    /api/v1/master-data/classes/{id}
#   POST   /api/v1/master-data/classes/{id}/update
#   GET    /api/v1/master-data/courses/{id}
#   POST   /api/v1/master-data/courses/{id}/update
#   GET    /api/v1/master-data/students/{id}
#   POST   /api/v1/master-data/students/{id}/update
#   GET    /api/v1/master-data/departments/{id}
#   POST   /api/v1/master-data/departments/{id}/update
#   POST   /api/v1/master-data/semesters/{id}/update
set -euo pipefail

API_BASE="${API_BASE:-http://localhost:8080/api/v1}"
PASS_DEFAULT="ChangeMeNow!2025"

red()   { printf "\033[31m%s\033[0m\n" "$*"; }
green() { printf "\033[32m%s\033[0m\n" "$*"; }
say()   { printf "\n==> %s\n" "$*"; }
PASS=0; FAIL=0
ok()  { green "  OK   $*"; PASS=$((PASS+1)); }
bad() { red   "  FAIL $*"; FAIL=$((FAIL+1)); }

require() { command -v "$1" >/dev/null || { red "missing: $1"; exit 2; }; }
require curl; require jq; require openssl

# ── signing helpers ────────────────────────────────────────────────────────
sign() {
  local method="$1" path="$2" key="$3"
  local ts nonce canonical sig
  ts=$(date +%s)
  nonce="cov-$RANDOM-$RANDOM-$(date +%N)"
  local sign_path="${path%%\?*}"
  canonical="${method}"$'\n'"${sign_path}"$'\n'"${ts}"$'\n'"${nonce}"
  sig=$(printf '%s' "$canonical" | openssl dgst -sha256 -hmac "$key" -hex | awk '{print $2}')
  printf '%s\n%s\n%s\n' "$ts" "$nonce" "$sig"
}

call() {
  local method="$1" path="$2" token="$3" key="$4" body="${5:-}"
  mapfile -t parts < <(sign "$method" "$path" "$key")
  local args=(-sS -o /tmp/cov_body -w '%{http_code}'
    -H "x-silveroak-token: $token"
    -H "x-silveroak-timestamp: ${parts[0]}"
    -H "x-silveroak-nonce: ${parts[1]}"
    -H "x-silveroak-signature: ${parts[2]}"
    -X "$method")
  if [[ -n "$body" ]]; then
    args+=(-H "content-type: application/json" --data "$body")
  fi
  local host="${API_BASE%%/api/v1*}"
  curl "${args[@]}" "${host}${path}"
}

login() {
  local email="$1"
  curl -sS -X POST -H 'content-type: application/json' \
    -d "{\"email\":\"$email\",\"password\":\"$PASS_DEFAULT\"}" \
    "${API_BASE}/auth/login"
}

# ── bootstrap tokens ───────────────────────────────────────────────────────
say "bootstrap: login admin and medical user"
adm=$(login "admin@silveroak.local")
ADM_TOKEN=$(echo "$adm" | jq -r .api_token)
ADM_KEY=$(echo   "$adm" | jq -r .signing_key)
[[ "$ADM_TOKEN" != "null" && -n "$ADM_TOKEN" ]] || { bad "admin login failed"; exit 1; }
ok "admin login"

med=$(login "medical@silveroak.local")
MED_TOKEN=$(echo "$med" | jq -r .api_token)
MED_KEY=$(echo   "$med" | jq -r .signing_key)
[[ "$MED_TOKEN" != "null" && -n "$MED_TOKEN" ]] || { bad "medical login failed"; exit 1; }
ok "medical login"

sen=$(login "senior@silveroak.local")
SEN_TOKEN=$(echo "$sen" | jq -r .api_token)
SEN_KEY=$(echo   "$sen" | jq -r .signing_key)
[[ "$SEN_TOKEN" != "null" && -n "$SEN_TOKEN" ]] || { bad "senior login failed"; exit 1; }
ok "senior login"

# Discover institution (needed for master-data fixtures)
code=$(call GET /api/v1/master-data/institutions "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] || { bad "institutions=$code"; exit 1; }
IID=$(jq -r '.[0].id' /tmp/cov_body)
[[ "$IID" != "null" && -n "$IID" ]] || { bad "no institution id"; exit 1; }
ok "institution id discovered: $IID"

RUN_ID="$RANDOM"

# =========================================================================
# 1. /admin/blacklist  — list / add / delete
# =========================================================================
say "1. admin blacklist — list, add, delete"

code=$(call GET /api/v1/admin/blacklist "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "GET /admin/blacklist → 200" || bad "GET blacklist=$code"
jq -e 'type == "array"' /tmp/cov_body >/dev/null \
  && ok "blacklist response is an array" \
  || bad "blacklist not an array"

# Non-admin is denied — exact status 403 (not a range).
code=$(call GET /api/v1/admin/blacklist "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "403" ]] && ok "non-admin blacklist GET → 403" || bad "got $code"

TEST_IP="203.0.113.${RUN_ID: -2}"
TEST_IP="${TEST_IP//[^0-9.]/}"
# Keep IP in documentation range.
TEST_IP="203.0.113.$(( RUN_ID % 250 + 2 ))"

# Create the entry — exact 200.
code=$(call POST /api/v1/admin/blacklist "$ADM_TOKEN" "$ADM_KEY" \
  "{\"ip\":\"$TEST_IP\",\"reason\":\"coverage_extra test ${RUN_ID}\"}")
[[ "$code" == "200" ]] && ok "POST /admin/blacklist → 200" || bad "add blacklist=$code body=$(cat /tmp/cov_body)"
jq -e '.ok == true' /tmp/cov_body >/dev/null \
  && ok "add response has ok:true" \
  || bad "add response missing ok:true"

# Verify the new IP is listed.  The handler serialises `Vec<(String, String)>`
# as `[[ip, reason], …]`, so we select on the first tuple element.
code=$(call GET /api/v1/admin/blacklist "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] || bad "relist blacklist=$code"
MATCH=$(jq --arg ip "$TEST_IP" '[.[] | select(.[0]==$ip)] | length' /tmp/cov_body)
[[ "$MATCH" -ge 1 ]] && ok "blacklist contains added IP ($TEST_IP)" \
  || bad "added IP not in blacklist (count=$MATCH)"

# Non-admin add rejected.
code=$(call POST /api/v1/admin/blacklist "$MED_TOKEN" "$MED_KEY" \
  "{\"ip\":\"198.51.100.77\",\"reason\":\"nope\"}")
[[ "$code" == "403" ]] && ok "non-admin POST /admin/blacklist → 403" || bad "got $code"

# Remove the entry.
code=$(call DELETE "/api/v1/admin/blacklist/${TEST_IP}" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "DELETE /admin/blacklist/{ip} → 200" || bad "delete=$code"
jq -e '.ok == true' /tmp/cov_body >/dev/null \
  && ok "delete response has ok:true" \
  || bad "delete response missing ok:true"

# Confirm removal.
code=$(call GET /api/v1/admin/blacklist "$ADM_TOKEN" "$ADM_KEY")
AFTER=$(jq --arg ip "$TEST_IP" '[.[] | select(.[0]==$ip)] | length' /tmp/cov_body)
[[ "$AFTER" == "0" ]] && ok "IP removed from blacklist" || bad "still present (count=$AFTER)"

# Non-admin delete denied.
code=$(call DELETE "/api/v1/admin/blacklist/203.0.113.99" "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "403" ]] && ok "non-admin DELETE /admin/blacklist/{ip} → 403" || bad "got $code"

# =========================================================================
# 2. /admin/users/grant-role
# =========================================================================
say "2. admin grant-role"

# First get the senior user's id via /me.
code=$(call GET /api/v1/me "$SEN_TOKEN" "$SEN_KEY")
[[ "$code" == "200" ]] || bad "senior /me=$code"
SENIOR_UID=$(jq -r '.user_id' /tmp/cov_body)
[[ -n "$SENIOR_UID" && "$SENIOR_UID" != "null" ]] && ok "senior uid=$SENIOR_UID" \
  || { bad "no senior uid"; exit 1; }

# Idempotent grant (role already present per seed → ON CONFLICT DO NOTHING still 200).
code=$(call POST /api/v1/admin/users/grant-role "$ADM_TOKEN" "$ADM_KEY" \
  "{\"user_id\":\"$SENIOR_UID\",\"role_key\":\"senior\"}")
[[ "$code" == "200" ]] && ok "POST /admin/users/grant-role → 200" || bad "grant-role=$code body=$(cat /tmp/cov_body)"
jq -e '.ok == true' /tmp/cov_body >/dev/null \
  && ok "grant-role response has ok:true" \
  || bad "grant-role response missing ok:true"

# Unknown role_key → 404 exact.
code=$(call POST /api/v1/admin/users/grant-role "$ADM_TOKEN" "$ADM_KEY" \
  "{\"user_id\":\"$SENIOR_UID\",\"role_key\":\"no_such_role_${RUN_ID}\"}")
[[ "$code" == "404" ]] && ok "unknown role_key → 404" || bad "got $code"

# Non-admin denied.
code=$(call POST /api/v1/admin/users/grant-role "$MED_TOKEN" "$MED_KEY" \
  "{\"user_id\":\"$SENIOR_UID\",\"role_key\":\"senior\"}")
[[ "$code" == "403" ]] && ok "non-admin grant-role → 403" || bad "got $code"

# =========================================================================
# 3. /auth/recovery/request  — public (no signed auth)
# =========================================================================
say "3. auth recovery request"

host="${API_BASE%%/api/v1*}"
# Known account.
code=$(curl -sS -o /tmp/cov_body -w '%{http_code}' -X POST \
  -H 'content-type: application/json' \
  -d '{"email":"admin@silveroak.local"}' \
  "${host}/api/v1/auth/recovery/request")
[[ "$code" == "200" ]] && ok "POST /auth/recovery/request → 200 (known email)" \
  || bad "recovery(known)=$code"
jq -e '.ok == true' /tmp/cov_body >/dev/null \
  && ok "recovery response ok:true" \
  || bad "recovery response missing ok:true"

# Unknown account — must also be 200 to avoid enumeration.
code=$(curl -sS -o /tmp/cov_body -w '%{http_code}' -X POST \
  -H 'content-type: application/json' \
  -d "{\"email\":\"nobody-${RUN_ID}@example.com\"}" \
  "${host}/api/v1/auth/recovery/request")
[[ "$code" == "200" ]] && ok "recovery(unknown) still 200 (anti-enumeration)" \
  || bad "recovery(unknown)=$code"

# =========================================================================
# 4. /requisitions/mine/list  and  /requisitions/{id}/audit
# =========================================================================
say "4. requisitions mine/list + audit timeline"

code=$(call GET /api/v1/requisitions/mine/list "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "200" ]] && ok "GET /requisitions/mine/list → 200" || bad "mine/list=$code body=$(cat /tmp/cov_body)"
jq -e 'type == "array"' /tmp/cov_body >/dev/null \
  && ok "mine/list returns an array" \
  || bad "mine/list not an array"

# Discover medical user's department scope for requisition creation.
code=$(call GET /api/v1/me "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "200" ]] || bad "medical /me=$code"
MED_DEPT=$(jq -r '.scopes[] | select(.kind=="department") | .reference' /tmp/cov_body | head -n1)
[[ -n "$MED_DEPT" && "$MED_DEPT" != "null" ]] || { bad "no medical dept scope"; exit 1; }

# Create a fresh requisition to guarantee at least one audit row.
code=$(call GET /api/v1/inventory/catalog "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "200" ]] || bad "catalog=$code"
ITEM_ID=$(jq -r '.[] | select(.sku=="PPE-002") | .id' /tmp/cov_body)
[[ -n "$ITEM_ID" && "$ITEM_ID" != "null" ]] || { bad "no inventory item"; exit 1; }

code=$(call POST /api/v1/requisitions "$MED_TOKEN" "$MED_KEY" \
  "{\"department_id\":\"$MED_DEPT\",\"needed_by\":\"2030-12-31\",\"justification\":\"coverage_extra audit test ${RUN_ID}\",\"lines\":[{\"item_id\":\"$ITEM_ID\",\"quantity\":1}]}")
# create endpoint returns 200 (not 201) per routes/requisitions.rs
[[ "$code" == "200" ]] && ok "create requisition 200" || bad "create=$code body=$(cat /tmp/cov_body)"
REQ_ID=$(jq -r '.id' /tmp/cov_body)

# Audit on the freshly created requisition.
code=$(call GET "/api/v1/requisitions/${REQ_ID}/audit" "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "200" ]] && ok "GET /requisitions/{id}/audit → 200" || bad "audit=$code body=$(cat /tmp/cov_body)"
jq -e 'type == "array"' /tmp/cov_body >/dev/null \
  && ok "audit returns array" \
  || bad "audit not an array"
AUDIT_COUNT=$(jq 'length' /tmp/cov_body)
[[ "$AUDIT_COUNT" -ge 1 ]] && ok "audit has ≥1 entry (got $AUDIT_COUNT)" \
  || bad "audit empty"

# mine/list must now include the new requisition.
code=$(call GET /api/v1/requisitions/mine/list "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "200" ]] || bad "mine/list relist=$code"
MATCH=$(jq --arg id "$REQ_ID" '[.[] | select(.id==$id)] | length' /tmp/cov_body)
[[ "$MATCH" == "1" ]] && ok "new requisition visible in mine/list" \
  || bad "requisition not in mine/list (match=$MATCH)"

# Audit 404 for non-existent UUID (exact status).
code=$(call GET "/api/v1/requisitions/00000000-0000-0000-0000-000000000000/audit" "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "404" ]] && ok "audit of missing req → 404" || bad "got $code"

# =========================================================================
# 5. /orders/cart/coupon-remove
# =========================================================================
say "5. orders coupon-remove"

# Clean senior's cart first
# The cart must end up empty of coupons regardless of prior state.
# Fetch products to obtain a usable product_id.
code=$(call GET /api/v1/orders/products "$SEN_TOKEN" "$SEN_KEY")
[[ "$code" == "200" ]] || bad "products=$code"
PROD=$(jq -r '.[] | select(.sku=="CAN-001") | .id' /tmp/cov_body)
[[ -n "$PROD" && "$PROD" != "null" ]] || { bad "CAN-001 not found"; exit 1; }

# Defensively clear any residual line from a prior suite, then add a fresh qty=1.
call POST /api/v1/orders/cart/lines "$SEN_TOKEN" "$SEN_KEY" \
  "{\"product_id\":\"$PROD\",\"quantity\":0}" >/dev/null 2>&1 || true
call POST /api/v1/orders/cart/lines "$SEN_TOKEN" "$SEN_KEY" \
  "{\"product_id\":\"$PROD\",\"quantity\":1}" >/dev/null

# LOYAL5 stacks with senior member price (see orders.sh step 10).
code=$(call POST /api/v1/orders/cart/coupon "$SEN_TOKEN" "$SEN_KEY" '{"code":"LOYAL5"}')
[[ "$code" == "200" ]] || bad "apply LOYAL5=$code"
code=$(call GET /api/v1/orders/cart "$SEN_TOKEN" "$SEN_KEY")
APPLIED=$(jq '.pricing.coupon_applied' /tmp/cov_body)
[[ "$APPLIED" == "true" ]] && ok "LOYAL5 applied before removal" || bad "coupon_applied=$APPLIED"

# Now exercise the new endpoint.  The handler does not extract a body, so we
# send an empty POST; sending a body would still be accepted but the test
# stays closer to how a real client issues this call.
code=$(call POST /api/v1/orders/cart/coupon-remove "$SEN_TOKEN" "$SEN_KEY")
[[ "$code" == "200" ]] && ok "POST /orders/cart/coupon-remove → 200" || bad "coupon-remove=$code body=$(cat /tmp/cov_body)"

code=$(call GET /api/v1/orders/cart "$SEN_TOKEN" "$SEN_KEY")
APPLIED_AFTER=$(jq '.pricing.coupon_applied' /tmp/cov_body)
COUPON_CODE=$(jq -r '.coupon_code // empty' /tmp/cov_body)
[[ "$APPLIED_AFTER" != "true" ]] && ok "coupon cleared after remove (coupon_applied=$APPLIED_AFTER)" \
  || bad "coupon still applied"
[[ -z "$COUPON_CODE" || "$COUPON_CODE" == "null" ]] && ok "cart.coupon_code unset after remove" \
  || bad "coupon_code lingers: $COUPON_CODE"

# Idempotent — removing when there is no coupon must still succeed.
code=$(call POST /api/v1/orders/cart/coupon-remove "$SEN_TOKEN" "$SEN_KEY")
[[ "$code" == "200" ]] && ok "coupon-remove is idempotent (200)" || bad "got $code"

# Clean up senior cart.
call POST /api/v1/orders/cart/lines "$SEN_TOKEN" "$SEN_KEY" \
  "{\"product_id\":\"$PROD\",\"quantity\":0}" >/dev/null

# =========================================================================
# 6. master-data: get/update for semesters, classes, courses, students, depts
# =========================================================================
say "6. master-data get + update coverage"

# --- 6a. Create a fresh semester + dept + class + course + student ---------

# Create semester
code=$(call POST "/api/v1/master-data/institutions/$IID/semesters" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"code\":\"SCOV-${RUN_ID}\",\"label\":\"Cov Semester ${RUN_ID}\",\"starts_on\":\"2029-01-10\",\"ends_on\":\"2029-05-31\",\"is_active\":true}")
[[ "$code" == "201" ]] && ok "create semester 201" || bad "create semester=$code body=$(cat /tmp/cov_body)"
SEM_ID=$(jq -r .id /tmp/cov_body)

# Find an existing site + create a department.
code=$(call GET "/api/v1/master-data/institutions/$IID/departments?per_page=1" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] || bad "list departments=$code"
SITE_ID=$(jq -r '.items[0].site_id // empty' /tmp/cov_body)
[[ -n "$SITE_ID" ]] || { bad "no site_id found for institution"; exit 1; }

code=$(call POST "/api/v1/master-data/institutions/$IID/departments" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"name\":\"Cov Dept ${RUN_ID}\",\"site_id\":\"$SITE_ID\"}")
[[ "$code" == "201" ]] && ok "create dept 201" || bad "create dept=$code body=$(cat /tmp/cov_body)"
DEPT_ID=$(jq -r .id /tmp/cov_body)

code=$(call POST "/api/v1/master-data/institutions/$IID/classes" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"code\":\"CLSC-${RUN_ID}\",\"label\":\"Cov Class\",\"semester_id\":\"$SEM_ID\",\"department_id\":\"$DEPT_ID\",\"is_active\":true}")
[[ "$code" == "201" ]] && ok "create class 201" || bad "create class=$code body=$(cat /tmp/cov_body)"
CLS_ID=$(jq -r .id /tmp/cov_body)

code=$(call POST "/api/v1/master-data/institutions/$IID/courses" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"code\":\"CRSC-${RUN_ID}\",\"title\":\"Cov Course\",\"credits\":3,\"is_active\":true}")
[[ "$code" == "201" ]] && ok "create course 201" || bad "create course=$code body=$(cat /tmp/cov_body)"
CRS_ID=$(jq -r .id /tmp/cov_body)

code=$(call POST "/api/v1/master-data/institutions/$IID/students" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"student_number\":\"STUC-${RUN_ID}\",\"first_name\":\"Cov\",\"last_name\":\"Student\",\"class_id\":\"$CLS_ID\"}")
[[ "$code" == "201" ]] && ok "create student 201" || bad "create student=$code body=$(cat /tmp/cov_body)"
STU_ID=$(jq -r .id /tmp/cov_body)

# --- 6b. GET single-entity endpoints ---------------------------------------

code=$(call GET "/api/v1/master-data/semesters/$SEM_ID" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "GET /master-data/semesters/{id} → 200" || bad "get semester=$code"
jq -e --arg c "SCOV-${RUN_ID}" '.code == $c' /tmp/cov_body >/dev/null \
  && ok "semester body matches" || bad "body mismatch"

code=$(call GET "/api/v1/master-data/classes/$CLS_ID" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "GET /master-data/classes/{id} → 200" || bad "get class=$code"
jq -e --arg c "CLSC-${RUN_ID}" '.code == $c' /tmp/cov_body >/dev/null \
  && ok "class body matches" || bad "body mismatch"

code=$(call GET "/api/v1/master-data/courses/$CRS_ID" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "GET /master-data/courses/{id} → 200" || bad "get course=$code"
jq -e --arg c "CRSC-${RUN_ID}" '.code == $c' /tmp/cov_body >/dev/null \
  && ok "course body matches" || bad "body mismatch"

code=$(call GET "/api/v1/master-data/students/$STU_ID" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "GET /master-data/students/{id} → 200" || bad "get student=$code"
jq -e --arg s "STUC-${RUN_ID}" '.student_number == $s' /tmp/cov_body >/dev/null \
  && ok "student body matches" || bad "body mismatch"

code=$(call GET "/api/v1/master-data/departments/$DEPT_ID" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "GET /master-data/departments/{id} → 200" || bad "get dept=$code"
jq -e --arg n "Cov Dept ${RUN_ID}" '.name == $n' /tmp/cov_body >/dev/null \
  && ok "department body matches" || bad "body mismatch"

# GET of non-existent id → 404 exact.
code=$(call GET "/api/v1/master-data/semesters/00000000-0000-0000-0000-000000000000" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "404" ]] && ok "get missing semester → 404" || bad "got $code"

# --- 6c. POST {id}/update endpoints ----------------------------------------

# Update semester — change label.
code=$(call POST "/api/v1/master-data/semesters/$SEM_ID/update" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"code\":\"SCOV-${RUN_ID}\",\"label\":\"Cov Semester Updated\",\"starts_on\":\"2029-01-10\",\"ends_on\":\"2029-06-30\",\"is_active\":true}")
[[ "$code" == "200" ]] && ok "POST /master-data/semesters/{id}/update → 200" || bad "update sem=$code body=$(cat /tmp/cov_body)"
jq -e '.label == "Cov Semester Updated"' /tmp/cov_body >/dev/null \
  && ok "semester label updated" || bad "label not updated"

# Update class — change label.
code=$(call POST "/api/v1/master-data/classes/$CLS_ID/update" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"code\":\"CLSC-${RUN_ID}\",\"label\":\"Cov Class Updated\",\"semester_id\":\"$SEM_ID\",\"department_id\":\"$DEPT_ID\",\"is_active\":true}")
[[ "$code" == "200" ]] && ok "POST /master-data/classes/{id}/update → 200" || bad "update class=$code body=$(cat /tmp/cov_body)"
jq -e '.label == "Cov Class Updated"' /tmp/cov_body >/dev/null \
  && ok "class label updated" || bad "label not updated"

# Update course — change credits to 4.
code=$(call POST "/api/v1/master-data/courses/$CRS_ID/update" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"code\":\"CRSC-${RUN_ID}\",\"title\":\"Cov Course v2\",\"credits\":4,\"is_active\":true}")
[[ "$code" == "200" ]] && ok "POST /master-data/courses/{id}/update → 200" || bad "update course=$code body=$(cat /tmp/cov_body)"
jq -e '.credits == 4' /tmp/cov_body >/dev/null \
  && ok "course credits updated to 4" || bad "credits not updated"

# Update student — change first_name.
code=$(call POST "/api/v1/master-data/students/$STU_ID/update" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"student_number\":\"STUC-${RUN_ID}\",\"first_name\":\"CovUpdated\",\"last_name\":\"Student\",\"class_id\":\"$CLS_ID\"}")
[[ "$code" == "200" ]] && ok "POST /master-data/students/{id}/update → 200" || bad "update student=$code body=$(cat /tmp/cov_body)"
jq -e '.first_name == "CovUpdated"' /tmp/cov_body >/dev/null \
  && ok "student first_name updated" || bad "name not updated"

# Update department — change name.
code=$(call POST "/api/v1/master-data/departments/$DEPT_ID/update" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"name\":\"Cov Dept v2 ${RUN_ID}\",\"site_id\":\"$SITE_ID\"}")
[[ "$code" == "200" ]] && ok "POST /master-data/departments/{id}/update → 200" || bad "update dept=$code body=$(cat /tmp/cov_body)"
jq -e --arg n "Cov Dept v2 ${RUN_ID}" '.name == $n' /tmp/cov_body >/dev/null \
  && ok "department name updated" || bad "name not updated"

# Update for non-existent id → 404.
code=$(call POST "/api/v1/master-data/semesters/00000000-0000-0000-0000-000000000000/update" "$ADM_TOKEN" "$ADM_KEY" \
  '{"code":"X","label":"X","starts_on":"2030-01-01","ends_on":"2030-06-30"}')
[[ "$code" == "404" ]] && ok "update missing semester → 404" || bad "got $code"

# Non-admin forbidden on a real update target.
code=$(call POST "/api/v1/master-data/semesters/$SEM_ID/update" "$MED_TOKEN" "$MED_KEY" \
  "{\"code\":\"SCOV-${RUN_ID}\",\"label\":\"pwn\",\"starts_on\":\"2029-01-10\",\"ends_on\":\"2029-06-30\"}")
[[ "$code" == "403" ]] && ok "non-admin semester update → 403" || bad "got $code"

# --- 6d. Cleanup (reverse of FK order) -------------------------------------
say "6d. cleanup cov fixtures"
call POST "/api/v1/master-data/students/$STU_ID/delete"    "$ADM_TOKEN" "$ADM_KEY" "" >/dev/null
call POST "/api/v1/master-data/courses/$CRS_ID/delete"     "$ADM_TOKEN" "$ADM_KEY" "" >/dev/null
call POST "/api/v1/master-data/classes/$CLS_ID/delete"     "$ADM_TOKEN" "$ADM_KEY" "" >/dev/null
call POST "/api/v1/master-data/semesters/$SEM_ID/delete"   "$ADM_TOKEN" "$ADM_KEY" "" >/dev/null
call POST "/api/v1/master-data/departments/$DEPT_ID/delete" "$ADM_TOKEN" "$ADM_KEY" "" >/dev/null
ok "fixtures cleaned up"

# =========================================================================
printf "\n"
printf "Results: %s passed, %s failed\n" "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]] || exit 1
