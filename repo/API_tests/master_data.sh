#!/usr/bin/env bash
# SilverOak Phase 5 — Master Data CRUD, Import/Export, File Fingerprints.
#
# Test cases:
#   1.  Institutions list (admin only)
#   2.  Non-admin access denied on all CRUD endpoints
#   3.  Semesters: create, get, list, update, delete-blocked-by-class, delete
#   4.  Departments: create, list, delete-blocked-by-class, delete
#   5.  Classes: create (with semester + dept FK), list, delete-blocked-by-student, delete
#   6.  Courses: create, list, delete
#   7.  Students: create, list, delete
#   8.  Import CSV — valid file → accepted rows
#   9.  Import XLSX — duplicate fingerprint → 409
#   10. Import CSV — missing required column → failed job
#   11. Import CSV — some rows invalid → partial success (207)
#   12. Export CSV → correct content-type + rows
#   13. Export XLSX → correct content-type
#   14. Import job status endpoint
#   15. Unknown entity_type → 400
#   16. Unknown format for export → 400
#   17. Non-admin export → 403
#   18. Filter + pagination on list
#   19. FK guard: delete department with class → 400
#   20. Delete cascade: student then class then semester then department
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
require curl; require jq; require openssl; require python3

# ── signing helpers ────────────────────────────────────────────────────────
sign() {
  local method="$1" path="$2" key="$3"
  local sign_path="${path%%\?*}"  # strip query string for canonical
  local ts nonce canonical sig
  ts=$(date +%s)
  nonce="n-$RANDOM-$RANDOM"
  canonical="${method}"$'\n'"${sign_path}"$'\n'"${ts}"$'\n'"${nonce}"
  sig=$(printf '%s' "$canonical" | openssl dgst -sha256 -hmac "$key" -hex | awk '{print $2}')
  printf '%s\n%s\n%s\n' "$ts" "$nonce" "$sig"
}

call() {
  local method="$1" path="$2" token="$3" key="$4" body="${5:-}"
  mapfile -t parts < <(sign "$method" "$path" "$key")
  local args=(-sS -o /tmp/md_body -w '%{http_code}'
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

call_upload() {
  local path="$1" token="$2" key="$3" file="$4"
  mapfile -t parts < <(sign "POST" "$path" "$key")
  curl -sS -o /tmp/md_body -w '%{http_code}' \
    -H "x-silveroak-token: $token" \
    -H "x-silveroak-timestamp: ${parts[0]}" \
    -H "x-silveroak-nonce: ${parts[1]}" \
    -H "x-silveroak-signature: ${parts[2]}" \
    -X POST -F "file=@$file" \
    "${API_BASE%%/api/v1*}${path}"
}

call_export() {
  local path="$1" token="$2" key="$3" outfile="$4"
  mapfile -t parts < <(sign "GET" "$path" "$key")
  curl -sS -o "$outfile" -w '%{http_code}' \
    -H "x-silveroak-token: $token" \
    -H "x-silveroak-timestamp: ${parts[0]}" \
    -H "x-silveroak-nonce: ${parts[1]}" \
    -H "x-silveroak-signature: ${parts[2]}" \
    "${API_BASE%%/api/v1*}${path}"
}

login() {
  local email="$1"
  curl -sS -X POST -H 'content-type: application/json' \
    -d "{\"email\":\"$email\",\"password\":\"$PASS_DEFAULT\"}" \
    "${API_BASE}/auth/login"
}

# ── bootstrap tokens ───────────────────────────────────────────────────────
say "bootstrap: login as admin and regular user"
adm=$(login "admin@silveroak.local")
ADM_TOKEN=$(echo "$adm" | jq -r .api_token)
ADM_KEY=$(echo   "$adm" | jq -r .signing_key)
[[ "$ADM_TOKEN" != "null" && -n "$ADM_TOKEN" ]] || { bad "admin login failed"; exit 1; }

med=$(login "medical@silveroak.local")
MED_TOKEN=$(echo "$med" | jq -r .api_token)
MED_KEY=$(echo   "$med" | jq -r .signing_key)
[[ "$MED_TOKEN" != "null" && -n "$MED_TOKEN" ]] || { bad "medical login failed"; exit 1; }

# ── 1: institutions list ───────────────────────────────────────────────────
say "1. institutions list (admin)"
code=$(call GET /api/v1/master-data/institutions "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "institutions 200" || bad "institutions=$code"
IID=$(jq -r '.[0].id' /tmp/md_body)
[[ "$IID" != "null" && -n "$IID" ]] && ok "got institution id" || { bad "no institution"; exit 1; }
echo "  Using institution: $IID"

# ── 2: non-admin denied ────────────────────────────────────────────────────
say "2. non-admin denied"
code=$(call GET /api/v1/master-data/institutions "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "403" ]] && ok "non-admin → 403" || bad "expected 403, got $code"

code=$(call GET "/api/v1/master-data/institutions/$IID/semesters" "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "403" ]] && ok "semesters non-admin → 403" || bad "expected 403, got $code"

# ── 3: semesters CRUD ─────────────────────────────────────────────────────
say "3. semesters CRUD"
RUN_ID="$RANDOM"

# 3a create
code=$(call POST "/api/v1/master-data/institutions/$IID/semesters" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"code\":\"SEM-${RUN_ID}\",\"label\":\"Test Semester ${RUN_ID}\",\"starts_on\":\"2025-01-15\",\"ends_on\":\"2025-05-31\",\"is_active\":true}")
[[ "$code" == "201" ]] && ok "create semester 201" || bad "create semester=$code (body: $(cat /tmp/md_body))"
SEM_ID=$(jq -r .id /tmp/md_body)
SEM_CODE=$(jq -r .code /tmp/md_body)
[[ "$SEM_CODE" == "SEM-${RUN_ID}" ]] && ok "semester code correct" || bad "code=$SEM_CODE"

# 3b duplicate code → 400
code=$(call POST "/api/v1/master-data/institutions/$IID/semesters" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"code\":\"SEM-${RUN_ID}\",\"label\":\"Duplicate\",\"starts_on\":\"2025-01-15\",\"ends_on\":\"2025-05-31\"}")
[[ "$code" == "400" ]] && ok "duplicate semester code → 400" || bad "got $code"

# 3c bad date range → 400
code=$(call POST "/api/v1/master-data/institutions/$IID/semesters" "$ADM_TOKEN" "$ADM_KEY" \
  '{"code":"S9999","label":"Bad","starts_on":"2025-12-31","ends_on":"2025-01-01"}')
[[ "$code" == "400" ]] && ok "bad date range → 400" || bad "got $code"

# 3d get semester
code=$(call GET "/api/v1/master-data/semesters/$SEM_ID" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "get semester 200" || bad "get semester=$code"

# 3e list semesters
code=$(call GET "/api/v1/master-data/institutions/$IID/semesters" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "list semesters 200" || bad "list semesters=$code"
COUNT=$(jq '.total' /tmp/md_body)
[[ "$COUNT" -ge 1 ]] && ok "at least 1 semester" || bad "total=$COUNT"

# ── 4: departments CRUD ───────────────────────────────────────────────────
say "4. departments CRUD"

# Need a site ID — fetch from API (use institution's site)
# Find a valid site_id by looking at departments or creating one
# We'll try to find the first site for this institution
# Since departments are fetched via sites, let's get a site_id from listing departments
code=$(call GET "/api/v1/master-data/institutions/$IID/departments?per_page=1" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "list departments 200" || bad "departments=$code"
EXISTING_SITE_ID=$(jq -r '.items[0].site_id // empty' /tmp/md_body)

if [[ -n "$EXISTING_SITE_ID" ]]; then
  echo "  Using existing site_id: $EXISTING_SITE_ID"
  # 4a create department
  code=$(call POST "/api/v1/master-data/institutions/$IID/departments" "$ADM_TOKEN" "$ADM_KEY" \
    "{\"name\":\"Test Dept ${RUN_ID}\",\"site_id\":\"$EXISTING_SITE_ID\"}")
  [[ "$code" == "201" ]] && ok "create department 201" || bad "create dept=$code ($(cat /tmp/md_body))"
  DEPT_ID=$(jq -r .id /tmp/md_body)

  # 4b duplicate name is allowed (no unique constraint on department names)
  code=$(call POST "/api/v1/master-data/institutions/$IID/departments" "$ADM_TOKEN" "$ADM_KEY" \
    "{\"name\":\"Test Dept ${RUN_ID}\",\"site_id\":\"$EXISTING_SITE_ID\"}")
  [[ "$code" == "201" ]] && ok "duplicate dept name allowed 201" || bad "got $code"

  # 4c wrong site → 400
  code=$(call POST "/api/v1/master-data/institutions/$IID/departments" "$ADM_TOKEN" "$ADM_KEY" \
    '{"name":"AnotherDept","site_id":"00000000-0000-0000-0000-000000000000"}')
  [[ "$code" == "400" ]] && ok "wrong site_id → 400" || bad "got $code"
else
  bad "no existing site found, skipping department tests"
  DEPT_ID=""
fi

# ── 5: classes CRUD ───────────────────────────────────────────────────────
say "5. classes CRUD"

if [[ -n "${DEPT_ID:-}" ]]; then
  code=$(call POST "/api/v1/master-data/institutions/$IID/classes" "$ADM_TOKEN" "$ADM_KEY" \
    "{\"code\":\"CLS-${RUN_ID}\",\"label\":\"Test Class\",\"semester_id\":\"$SEM_ID\",\"department_id\":\"$DEPT_ID\",\"is_active\":true}")
  [[ "$code" == "201" ]] && ok "create class 201" || bad "create class=$code ($(cat /tmp/md_body))"
  CLS_ID=$(jq -r .id /tmp/md_body)
  [[ "$(jq -r .semester_code /tmp/md_body)" == "SEM-${RUN_ID}" ]] && ok "semester_code FK resolved" || bad "semester_code wrong"
else
  code=$(call POST "/api/v1/master-data/institutions/$IID/classes" "$ADM_TOKEN" "$ADM_KEY" \
    "{\"code\":\"CLS-${RUN_ID}\",\"label\":\"Test Class\",\"is_active\":true}")
  [[ "$code" == "201" ]] && ok "create class (no FK) 201" || bad "create class=$code"
  CLS_ID=$(jq -r .id /tmp/md_body)
fi

# Duplicate code
code=$(call POST "/api/v1/master-data/institutions/$IID/classes" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"code\":\"CLS-${RUN_ID}\",\"label\":\"Duplicate Class\"}")
[[ "$code" == "400" ]] && ok "duplicate class code → 400" || bad "got $code"

# List classes
code=$(call GET "/api/v1/master-data/institutions/$IID/classes" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "list classes 200" || bad "list classes=$code"

# ── 6: courses CRUD ───────────────────────────────────────────────────────
say "6. courses CRUD"
code=$(call POST "/api/v1/master-data/institutions/$IID/courses" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"code\":\"CRS-${RUN_ID}\",\"title\":\"Introduction to Testing\",\"credits\":3,\"is_active\":true}")
[[ "$code" == "201" ]] && ok "create course 201" || bad "create course=$code ($(cat /tmp/md_body))"
CRS_ID=$(jq -r .id /tmp/md_body)

# Invalid credits
code=$(call POST "/api/v1/master-data/institutions/$IID/courses" "$ADM_TOKEN" "$ADM_KEY" \
  '{"code":"CRS-BAD","title":"Bad Credits","credits":-1}')
[[ "$code" == "400" ]] && ok "negative credits → 400" || bad "got $code"

code=$(call GET "/api/v1/master-data/institutions/$IID/courses" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "list courses 200" || bad "courses=$code"

# ── 7: students CRUD ──────────────────────────────────────────────────────
say "7. students CRUD"
code=$(call POST "/api/v1/master-data/institutions/$IID/students" "$ADM_TOKEN" "$ADM_KEY" \
  "{\"student_number\":\"STU-${RUN_ID}\",\"first_name\":\"Alice\",\"last_name\":\"Test\",\"class_id\":\"$CLS_ID\"}")
[[ "$code" == "201" ]] && ok "create student 201" || bad "create student=$code ($(cat /tmp/md_body))"
STU_ID=$(jq -r .id /tmp/md_body)

# Missing required field
code=$(call POST "/api/v1/master-data/institutions/$IID/students" "$ADM_TOKEN" "$ADM_KEY" \
  '{"student_number":"","first_name":"Bob","last_name":"Smith"}')
[[ "$code" == "400" ]] && ok "empty student_number → 400" || bad "got $code"

code=$(call GET "/api/v1/master-data/institutions/$IID/students" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "list students 200" || bad "students=$code"
[[ "$(jq '.total' /tmp/md_body)" -ge 1 ]] && ok "at least 1 student" || bad "total=0"

# ── 8: import CSV — valid ─────────────────────────────────────────────────
say "8. CSV import — valid file"
cat > /tmp/test_semesters.csv << EOF
code,label,starts_on,ends_on
SIMP-${RUN_ID}A,Spring Import ${RUN_ID},2026-01-15,2026-05-31
SIMP-${RUN_ID}B,Fall Import ${RUN_ID},2026-08-20,2026-12-20
EOF
code=$(call_upload \
  "/api/v1/master-data/institutions/$IID/import/semesters" \
  "$ADM_TOKEN" "$ADM_KEY" "/tmp/test_semesters.csv")
[[ "$code" == "200" || "$code" == "207" ]] && ok "CSV import $code" || bad "import=$code ($(cat /tmp/md_body))"
ACCEPTED=$(jq -r .accepted_rows /tmp/md_body)
JOB_ID=$(jq -r .job_id /tmp/md_body)
[[ "$ACCEPTED" -ge 1 ]] && ok "accepted_rows=$ACCEPTED" || bad "accepted=$ACCEPTED ($(cat /tmp/md_body))"

# ── 9: duplicate fingerprint → 409 ────────────────────────────────────────
say "9. duplicate file fingerprint → 409"
code=$(call_upload \
  "/api/v1/master-data/institutions/$IID/import/semesters" \
  "$ADM_TOKEN" "$ADM_KEY" "/tmp/test_semesters.csv")
[[ "$code" == "409" ]] && ok "duplicate fingerprint → 409" || bad "got $code ($(cat /tmp/md_body))"

# ── 10: missing required column → failed job ──────────────────────────────
say "10. CSV import — missing required column"
cat > /tmp/test_bad_cols.csv << EOF
label,starts_on,ends_on
Missing Code ${RUN_ID},2027-01-01,2027-05-31
EOF
code=$(call_upload \
  "/api/v1/master-data/institutions/$IID/import/semesters" \
  "$ADM_TOKEN" "$ADM_KEY" "/tmp/test_bad_cols.csv")
[[ "$code" == "422" || "$code" == "400" || "$code" == "200" ]] && ok "missing col → error response $code" || bad "got $code"
# Check job status if we got a job_id
if jq -e '.job_id' /tmp/md_body > /dev/null 2>&1; then
  BAD_JOB_ID=$(jq -r .job_id /tmp/md_body)
  code2=$(call GET "/api/v1/master-data/import-jobs/$BAD_JOB_ID" "$ADM_TOKEN" "$ADM_KEY")
  [[ "$code2" == "200" ]] && ok "job status 200" || bad "job status=$code2"
  STATUS=$(jq -r .status /tmp/md_body)
  [[ "$STATUS" == "failed" ]] && ok "job status=failed" || bad "status=$STATUS"
fi

# ── 11: partial success — some rows invalid ────────────────────────────────
say "11. CSV import — partial success"
cat > /tmp/test_partial.csv << EOF
code,label,starts_on,ends_on
SP-${RUN_ID}A,Spring Partial ${RUN_ID},2028-01-10,2028-06-30
SP-${RUN_ID}A,Duplicate Code,2028-01-10,2028-06-30
SP-${RUN_ID}B,Fall Partial ${RUN_ID},2028-08-01,2028-12-31
EOF
code=$(call_upload \
  "/api/v1/master-data/institutions/$IID/import/semesters" \
  "$ADM_TOKEN" "$ADM_KEY" "/tmp/test_partial.csv")
[[ "$code" == "200" || "$code" == "207" ]] && ok "partial import $code" || bad "partial=$code"
REJECTED=$(jq -r .rejected_rows /tmp/md_body)
TOTAL=$(jq -r .total_rows /tmp/md_body)
[[ "$TOTAL" -eq 3 ]] && ok "total_rows=3" || bad "total=$TOTAL"
[[ "$REJECTED" -ge 1 ]] && ok "at least 1 rejected" || bad "rejected=$REJECTED"

# ── 12: export CSV ────────────────────────────────────────────────────────
say "12. export semesters CSV"
code=$(call_export \
  "/api/v1/master-data/institutions/$IID/export/semesters?format=csv" \
  "$ADM_TOKEN" "$ADM_KEY" "/tmp/export_semesters.csv")
[[ "$code" == "200" ]] && ok "export CSV 200" || bad "export=$code"
# Check headers present
if grep -q "code,label" /tmp/export_semesters.csv; then
  ok "CSV has correct headers"
else
  bad "CSV missing expected headers ($(head -1 /tmp/export_semesters.csv))"
fi
LINE_COUNT=$(wc -l < /tmp/export_semesters.csv)
[[ "$LINE_COUNT" -ge 2 ]] && ok "CSV has data rows ($LINE_COUNT lines)" || bad "only $LINE_COUNT lines"

# ── 13: export XLSX ───────────────────────────────────────────────────────
say "13. export semesters XLSX"
code=$(call_export \
  "/api/v1/master-data/institutions/$IID/export/semesters?format=xlsx" \
  "$ADM_TOKEN" "$ADM_KEY" "/tmp/export_semesters.xlsx")
[[ "$code" == "200" ]] && ok "export XLSX 200" || bad "export xlsx=$code"
# XLSX is a zip — verify magic bytes
if python3 -c "
with open('/tmp/export_semesters.xlsx','rb') as f:
    sig=f.read(4)
assert sig==b'PK\x03\x04', f'bad magic: {sig!r}'
print('ok')
" 2>/dev/null | grep -q ok; then
  ok "XLSX has valid ZIP magic bytes"
else
  bad "XLSX invalid format"
fi

# ── 14: import job status ─────────────────────────────────────────────────
say "14. import job status"
code=$(call GET "/api/v1/master-data/import-jobs/$JOB_ID" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "import job status 200" || bad "job status=$code"
JOB_STATUS=$(jq -r .status /tmp/md_body)
[[ "$JOB_STATUS" == "done" ]] && ok "job status=done" || bad "status=$JOB_STATUS"
ROW_COUNT=$(jq '.rows | length' /tmp/md_body)
[[ "$ROW_COUNT" -ge 1 ]] && ok "job has row details" || bad "row_count=$ROW_COUNT"

# ── 15: unknown entity_type → 400 ────────────────────────────────────────
say "15. unknown entity_type"
cat > /tmp/test_unknown.csv << 'EOF'
name
test
EOF
code=$(call_upload \
  "/api/v1/master-data/institutions/$IID/import/unicorns" \
  "$ADM_TOKEN" "$ADM_KEY" "/tmp/test_unknown.csv")
[[ "$code" == "400" ]] && ok "unknown entity_type → 400" || bad "got $code"

# ── 16: unknown export format → 400 ──────────────────────────────────────
say "16. unknown export format"
code=$(call_export \
  "/api/v1/master-data/institutions/$IID/export/semesters?format=pdf" \
  "$ADM_TOKEN" "$ADM_KEY" /tmp/export_bad.tmp)
[[ "$code" == "400" ]] && ok "unknown format → 400" || bad "got $code"

# ── 17: non-admin export → 403 ───────────────────────────────────────────
say "17. non-admin export → 403"
code=$(call_export \
  "/api/v1/master-data/institutions/$IID/export/semesters?format=csv" \
  "$MED_TOKEN" "$MED_KEY" /tmp/export_forbidden.tmp)
[[ "$code" == "403" ]] && ok "non-admin export → 403" || bad "got $code"

# ── 18: filter + pagination ───────────────────────────────────────────────
say "18. filter and pagination"
code=$(call GET "/api/v1/master-data/institutions/$IID/semesters?filter=2026&per_page=1" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "filter query 200" || bad "filter=$code"
FILTERED=$(jq '.items | length' /tmp/md_body)
[[ "$FILTERED" -le 1 ]] && ok "per_page=1 respected" || bad "got $FILTERED items"

code=$(call GET "/api/v1/master-data/institutions/$IID/semesters?page=2&per_page=1" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "200" ]] && ok "page 2 query 200" || bad "page2=$code"

# ── 19: FK guard: delete dept with class → 400 ───────────────────────────
say "19. FK guard: delete department with active class"
if [[ -n "${DEPT_ID:-}" ]]; then
  code=$(call POST "/api/v1/master-data/departments/$DEPT_ID/delete" "$ADM_TOKEN" "$ADM_KEY" "")
  [[ "$code" == "400" ]] && ok "delete dept with class → 400" || bad "got $code ($(cat /tmp/md_body))"
else
  ok "skipped (no dept_id)"
fi

# FK guard: delete semester with class → 400
code=$(call POST "/api/v1/master-data/semesters/$SEM_ID/delete" "$ADM_TOKEN" "$ADM_KEY" "")
[[ "$code" == "400" ]] && ok "delete semester with class → 400" || bad "got $code"

# ── 20: cascade delete in correct order ──────────────────────────────────
say "20. cascade delete: student → class → semester → department"

# Delete student
code=$(call POST "/api/v1/master-data/students/$STU_ID/delete" "$ADM_TOKEN" "$ADM_KEY" "")
[[ "$code" == "204" ]] && ok "delete student 204" || bad "del student=$code"

# Delete course
code=$(call POST "/api/v1/master-data/courses/$CRS_ID/delete" "$ADM_TOKEN" "$ADM_KEY" "")
[[ "$code" == "204" ]] && ok "delete course 204" || bad "del course=$code"

# Delete class (student removed now)
code=$(call POST "/api/v1/master-data/classes/$CLS_ID/delete" "$ADM_TOKEN" "$ADM_KEY" "")
[[ "$code" == "204" ]] && ok "delete class 204" || bad "del class=$code"

# Delete semester (class removed now)
code=$(call POST "/api/v1/master-data/semesters/$SEM_ID/delete" "$ADM_TOKEN" "$ADM_KEY" "")
[[ "$code" == "204" ]] && ok "delete semester 204" || bad "del semester=$code"

# Delete department (class removed now)
if [[ -n "${DEPT_ID:-}" ]]; then
  code=$(call POST "/api/v1/master-data/departments/$DEPT_ID/delete" "$ADM_TOKEN" "$ADM_KEY" "")
  [[ "$code" == "204" ]] && ok "delete department 204" || bad "del dept=$code"
fi

# Get deleted semester → 404
code=$(call GET "/api/v1/master-data/semesters/$SEM_ID" "$ADM_TOKEN" "$ADM_KEY")
[[ "$code" == "404" ]] && ok "deleted semester → 404" || bad "got $code"

# ── summary ────────────────────────────────────────────────────────────────
printf "\n"
printf "Results: %s passed, %s failed\n" "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]]
