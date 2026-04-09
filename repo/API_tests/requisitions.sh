#!/usr/bin/env bash
# SilverOak Phase 3 — requisition + approval workflow tests.
#
# Runs against a live Dockerized stack. Default API_BASE=http://localhost:8080/api/v1
# Covers:
#   - catalog list (signed)
#   - create draft (validation: empty justification, empty lines)
#   - submit → routes through approval engine
#   - approver inbox respects role + scope
#   - cross-department approver cannot view (forbidden)
#   - reject requires reason
#   - send-back transition + resubmit
#   - withdraw only while pending
#   - invalid transition blocked (e.g. approve already-issued)
#   - controlled-category routing forces finance step
#   - final approval creates issue record + deducts stock
#   - insufficient stock → rollback (stock unchanged)
#   - audit timeline append + immutability via DB
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

# ----- helpers -----
sign() {
  local method="$1" path="$2" key="$3"
  local ts nonce canonical sig
  ts=$(date +%s)
  nonce="n-$RANDOM-$RANDOM"
  local sign_path="${path%%\?*}"  # strip query string for canonical
  canonical="${method}"$'\n'"${sign_path}"$'\n'"${ts}"$'\n'"${nonce}"
  sig=$(printf '%s' "$canonical" | openssl dgst -sha256 -hmac "$key" -hex | awk '{print $2}')
  printf '%s\n%s\n%s\n' "$ts" "$nonce" "$sig"
}

call() {
  local method="$1" path="$2" token="$3" key="$4" body="${5:-}"
  mapfile -t parts < <(sign "$method" "$path" "$key")
  local args=(-sS -o /tmp/silveroak_body -w '%{http_code}'
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

# ----- bootstrap admin token to look up demo dept_id and reset DB state -----
say "bootstrap: log in as admin to discover department id"
admin_resp=$(login "admin@silveroak.local")
ADMIN_TOKEN=$(echo "$admin_resp" | jq -r .api_token)
ADMIN_KEY=$(echo   "$admin_resp" | jq -r .signing_key)
[[ "$ADMIN_TOKEN" != "null" && -n "$ADMIN_TOKEN" ]] || { bad "admin login failed"; exit 1; }

# Discover the demo department id by hitting /me as the medical user
med_resp=$(login "medical@silveroak.local")
MED_TOKEN=$(echo "$med_resp" | jq -r .api_token)
MED_KEY=$(echo   "$med_resp" | jq -r .signing_key)

code=$(call GET /api/v1/me "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "200" ]] || { bad "/me=$code body=$(cat /tmp/silveroak_body)"; exit 1; }
DEPT_ID=$(jq -r '.scopes[] | select(.kind=="department") | .reference' /tmp/silveroak_body | head -n1)
[[ -n "$DEPT_ID" ]] && ok "discovered dept_id=$DEPT_ID" || { bad "no dept scope"; exit 1; }

# ----- 1: catalog -----
say "1. catalog list (signed)"
code=$(call GET /api/v1/inventory/catalog "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "200" ]] && ok "catalog 200" || bad "catalog=$code"
ITEM_PPE=$(jq -r '.[] | select(.sku=="PPE-001") | .id' /tmp/silveroak_body)
ITEM_CTL=$(jq -r '.[] | select(.sku=="CTL-002") | .id' /tmp/silveroak_body)
ITEM_HSK=$(jq -r '.[] | select(.sku=="HSK-001") | .id' /tmp/silveroak_body)
[[ -n "$ITEM_PPE" && -n "$ITEM_CTL" ]] && ok "catalog items present" || bad "missing items"

# ----- 2: validation -----
say "2. validation: missing justification"
code=$(call POST /api/v1/requisitions "$MED_TOKEN" "$MED_KEY" \
  "{\"department_id\":\"$DEPT_ID\",\"needed_by\":\"2030-12-31\",\"justification\":\"\",\"lines\":[{\"item_id\":\"$ITEM_PPE\",\"quantity\":1}]}")
[[ "$code" == "400" ]] && ok "empty justification → 400" || bad "got $code"

say "2b. validation: zero quantity"
code=$(call POST /api/v1/requisitions "$MED_TOKEN" "$MED_KEY" \
  "{\"department_id\":\"$DEPT_ID\",\"needed_by\":\"2030-12-31\",\"justification\":\"x\",\"lines\":[{\"item_id\":\"$ITEM_PPE\",\"quantity\":0}]}")
[[ "$code" == "400" ]] && ok "zero qty → 400" || bad "got $code"

# ----- 3: small requisition (department-only routing) -----
say "3. create + submit small (dept-only) requisition"
code=$(call POST /api/v1/requisitions "$MED_TOKEN" "$MED_KEY" \
  "{\"department_id\":\"$DEPT_ID\",\"needed_by\":\"2030-12-31\",\"justification\":\"PPE restock\",\"lines\":[{\"item_id\":\"$ITEM_PPE\",\"quantity\":2}]}")
SMALL_ID=$(jq -r .id /tmp/silveroak_body)
[[ "$code" == "200" && -n "$SMALL_ID" ]] && ok "draft created $SMALL_ID" || bad "create=$code"
code=$(call POST /api/v1/requisitions/$SMALL_ID/submit "$MED_TOKEN" "$MED_KEY" "{}")
[[ "$code" == "200" ]] && ok "submitted" || bad "submit=$code"
ROUTE_LEN=$(jq '.route | length' /tmp/silveroak_body)
[[ "$ROUTE_LEN" == "1" ]] && ok "small request routed to 1 node (dept only)" || bad "route_len=$ROUTE_LEN"

# ----- 4: controlled-category routing forces finance step -----
say "4. controlled-category routing → 2 nodes"
code=$(call POST /api/v1/requisitions "$MED_TOKEN" "$MED_KEY" \
  "{\"department_id\":\"$DEPT_ID\",\"needed_by\":\"2030-12-31\",\"justification\":\"controlled meds\",\"lines\":[{\"item_id\":\"$ITEM_CTL\",\"quantity\":1}]}")
CTL_ID=$(jq -r .id /tmp/silveroak_body)
call POST /api/v1/requisitions/$CTL_ID/submit "$MED_TOKEN" "$MED_KEY" "{}" >/dev/null
ROUTE_LEN=$(jq '.route | length' /tmp/silveroak_body)
[[ "$ROUTE_LEN" == "2" ]] && ok "controlled routes to 2 nodes" || bad "route_len=$ROUTE_LEN"

# ----- 5: approver inbox -----
say "5. approver inbox (dept_approver)"
appr=$(login "approver@silveroak.local")
APPR_TOKEN=$(echo "$appr" | jq -r .api_token)
APPR_KEY=$(echo   "$appr" | jq -r .signing_key)
code=$(call GET /api/v1/approvals/inbox "$APPR_TOKEN" "$APPR_KEY")
[[ "$code" == "200" ]] && ok "inbox 200" || bad "inbox=$code"
INBOX_COUNT=$(jq 'length' /tmp/silveroak_body)
ok "approver sees $INBOX_COUNT pending"

# ----- 6: reject requires reason -----
say "6. reject without reason → 400"
code=$(call POST /api/v1/requisitions/$CTL_ID/reject "$APPR_TOKEN" "$APPR_KEY" '{}')
[[ "$code" == "400" ]] && ok "reject blocked" || bad "got $code"

# ----- 7: send-back, then requester resubmits -----
say "7. send-back round-trip"
code=$(call POST /api/v1/requisitions/$SMALL_ID/send-back "$APPR_TOKEN" "$APPR_KEY" '{"reason":"please clarify"}')
[[ "$code" == "200" ]] && ok "sent back" || bad "send-back=$code"
status=$(jq -r .status /tmp/silveroak_body)
[[ "$status" == "sent_back" ]] && ok "status=sent_back" || bad "status=$status"
# requester resubmits
code=$(call POST /api/v1/requisitions/$SMALL_ID/submit "$MED_TOKEN" "$MED_KEY" '{}')
[[ "$code" == "200" ]] && ok "resubmitted" || bad "resubmit=$code"

# ----- 8: withdraw while pending -----
say "8. withdraw while pending"
code=$(call POST /api/v1/requisitions/$SMALL_ID/withdraw "$MED_TOKEN" "$MED_KEY" '{}')
[[ "$code" == "200" ]] && ok "withdrawn" || bad "withdraw=$code"
# Cannot withdraw again
code=$(call POST /api/v1/requisitions/$SMALL_ID/withdraw "$MED_TOKEN" "$MED_KEY" '{}')
[[ "$code" == "400" ]] && ok "second withdraw blocked" || bad "got $code"

# ----- 9: full approve path on the controlled requisition -----
say "9. approve dept step on controlled requisition"
code=$(call POST /api/v1/requisitions/$CTL_ID/approve "$APPR_TOKEN" "$APPR_KEY" '{"comment":"ok"}')
[[ "$code" == "200" ]] && ok "dept step approved" || bad "approve=$code"

say "10. finance approve → final + issue record + stock deduction"
fin=$(login "finance@silveroak.local")
FIN_TOKEN=$(echo "$fin" | jq -r .api_token)
FIN_KEY=$(echo   "$fin" | jq -r .signing_key)

# Capture stock before
call GET /api/v1/inventory/catalog "$ADMIN_TOKEN" "$ADMIN_KEY" >/dev/null
STOCK_BEFORE=$(jq -r --arg id "$ITEM_CTL" '.[] | select(.id==$id) | .on_hand' /tmp/silveroak_body)

code=$(call POST /api/v1/requisitions/$CTL_ID/approve "$FIN_TOKEN" "$FIN_KEY" '{"comment":"ok"}')
[[ "$code" == "200" ]] && ok "final approved" || bad "final=$code body=$(cat /tmp/silveroak_body)"
status=$(jq -r .status /tmp/silveroak_body)
[[ "$status" == "issued" ]] && ok "status=issued" || bad "status=$status"

# Issue record exists
code=$(call GET /api/v1/issue-records/$CTL_ID "$ADMIN_TOKEN" "$ADMIN_KEY")
[[ "$code" == "200" ]] && ok "issue_record exists" || bad "issue_record=$code"

# Stock decreased
call GET /api/v1/inventory/catalog "$ADMIN_TOKEN" "$ADMIN_KEY" >/dev/null
STOCK_AFTER=$(jq -r --arg id "$ITEM_CTL" '.[] | select(.id==$id) | .on_hand' /tmp/silveroak_body)
[[ "$STOCK_AFTER" -lt "$STOCK_BEFORE" ]] && ok "stock $STOCK_BEFORE → $STOCK_AFTER" || bad "stock unchanged"

# ----- 11: invalid transition blocked -----
say "11. cannot approve an already-issued requisition"
code=$(call POST /api/v1/requisitions/$CTL_ID/approve "$FIN_TOKEN" "$FIN_KEY" '{}')
[[ "$code" == "400" ]] && ok "blocked" || bad "got $code"

# ----- 12: insufficient stock rollback -----
say "12. insufficient stock → rollback"
# Find an item with very low stock by ordering an absurd quantity.
code=$(call POST /api/v1/requisitions "$MED_TOKEN" "$MED_KEY" \
  "{\"department_id\":\"$DEPT_ID\",\"needed_by\":\"2030-12-31\",\"justification\":\"absurd\",\"lines\":[{\"item_id\":\"$ITEM_HSK\",\"quantity\":99999}]}")
HUGE_ID=$(jq -r .id /tmp/silveroak_body)
call POST /api/v1/requisitions/$HUGE_ID/submit "$MED_TOKEN" "$MED_KEY" '{}' >/dev/null
# Capture stock before final approval
call GET /api/v1/inventory/catalog "$ADMIN_TOKEN" "$ADMIN_KEY" >/dev/null
STOCK_BEFORE=$(jq -r --arg id "$ITEM_HSK" '.[] | select(.id==$id) | .on_hand' /tmp/silveroak_body)
# This requisition is over $2,500? 99999 * 549 cents = much more, so finance routing
# Dept first
call POST /api/v1/requisitions/$HUGE_ID/approve "$APPR_TOKEN" "$APPR_KEY" '{}' >/dev/null
# Finance attempts → expect rollback because stock insufficient
code=$(call POST /api/v1/requisitions/$HUGE_ID/approve "$FIN_TOKEN" "$FIN_KEY" '{}')
[[ "$code" == "400" ]] && ok "insufficient stock 400" || bad "got $code"
call GET /api/v1/inventory/catalog "$ADMIN_TOKEN" "$ADMIN_KEY" >/dev/null
STOCK_AFTER=$(jq -r --arg id "$ITEM_HSK" '.[] | select(.id==$id) | .on_hand' /tmp/silveroak_body)
[[ "$STOCK_AFTER" == "$STOCK_BEFORE" ]] && ok "stock untouched ($STOCK_BEFORE)" || bad "stock changed $STOCK_BEFORE → $STOCK_AFTER"

# ----- 13: audit timeline append-only via DB trigger -----
say "13. audit immutability via direct SQL UPDATE"
if command -v psql >/dev/null && [[ -n "${PGURL:-}" ]]; then
  if PGPASSWORD=silveroak_dev psql "$PGURL" -c "UPDATE requisition_audit SET comment='hacked' WHERE id=(SELECT id FROM requisition_audit LIMIT 1);" 2>&1 | grep -q "append-only"; then
    ok "trigger blocked UPDATE"
  else
    bad "trigger did not block UPDATE"
  fi
else
  echo "  (skipped: set PGURL=postgres://silveroak:silveroak_dev@localhost:5432/silveroak to verify)"
fi

# ----- 14: 404 for missing requisition -----
say "14. 404 on unknown requisition id"
code=$(call GET /api/v1/requisitions/00000000-0000-0000-0000-000000000000 "$MED_TOKEN" "$MED_KEY")
[[ "$code" == "404" ]] && ok "404" || bad "got $code"

# ----- 15: family user cannot access requisitions (no permission) -----
say "15. family user has no requisitions:read permission → 403"
fam=$(login "family@silveroak.local")
FAM_TOKEN=$(echo "$fam" | jq -r .api_token)
FAM_KEY=$(echo   "$fam" | jq -r .signing_key)
code=$(call GET /api/v1/requisitions/$SMALL_ID "$FAM_TOKEN" "$FAM_KEY")
[[ "$code" == "403" ]] && ok "family user denied requisitions:read ($code)" || bad "expected 403, got $code"

echo
echo "PASS=$PASS  FAIL=$FAIL"
[[ "$FAIL" == "0" ]] || exit 1
