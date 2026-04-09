# Business Logic Questions Log

## 1) Requisition needed-by date format mismatch (Prompt: MM/DD/YYYY)

**Question:** The prompt says staff enter needed-by date in `MM/DD/YYYY`, but what should the API contract use to avoid locale parsing bugs?

**My Understanding/Hypothesis:** UI display format can be `MM/DD/YYYY`, but transport and persistence should use ISO `YYYY-MM-DD` for consistency and safer parsing.

**Solution:** Current implementation accepts/stores `NaiveDate` in backend (`CreateRequisitionInput.needed_by`) and the Yew form sends HTML date input (`yyyy-mm-dd`). We keep ISO for API/database and can format as `MM/DD/YYYY` in UI rendering only.

---

## 2) Requisition submit behavior from draft

**Question:** Should requisitions always be saved as draft first, or submitted immediately when user clicks submit?

**My Understanding/Hypothesis:** Draft-first is safer for auditability and future edits, but common staff flow should still feel one-step.

**Solution:** Current UI creates draft (`POST /requisitions`) then immediately calls `POST /requisitions/{id}/submit`. This preserves state machine correctness while giving users a “single submit” experience.

---

## 3) Approval routing threshold and controlled category precedence

**Question:** If requisition total exceeds cap (e.g., $2,500) or has controlled items, is finance routing additive or either/or?

**My Understanding/Hypothesis:** Either condition should trigger finance routing; controlled categories should override low amount cases.

**Solution:** Implemented route resolution uses snapshot (`total_amount_cents`, `contains_controlled`) via approval engine conditions. UI also shows route hint (`Dept only` vs `Dept → Finance`) when `total > cap || controlled`.

---

## 4) Who can act on approvals (role vs scope)

**Question:** Is having approver role enough, or must approver also be scoped to requisition department?

**My Understanding/Hypothesis:** Both role and data scope should be required to prevent cross-department approvals.

**Solution:** Approval actions (`approve/reject/send-back`) enforce permission + required step role + department scope (`assert_approval_dept_scope`). Inbox listing also filters by in-scope departments unless `scope:any`.

---

## 5) Reject/send-back reason requirements

**Question:** Prompt requires reject reason; is send-back reason also mandatory?

**My Understanding/Hypothesis:** Reject reason must be mandatory; send-back should also require reason for actionable edits and traceability.

**Solution:** Backend enforces non-empty reason for both `reject` and `send_back`; request fails with `BadRequest` if missing.

---

## 6) Withdraw boundary conditions

**Question:** Can requestors withdraw only while pending, or also while draft/sent_back?

**My Understanding/Hypothesis:** Business intent is “withdraw while pending,” but allowing pre-final states (draft/sent_back/pending) reduces operational friction.

**Solution:** State machine controls withdrawability (`ReqStatus::is_withdrawable`). Current implementation permits withdraw only in explicitly allowed non-terminal states and blocks after terminal/final states.

---

## 7) Audit trail immutability

**Question:** Should audit comments be editable/deletable by admins for corrections?

**My Understanding/Hypothesis:** No. For accountability/compliance, audit must be append-only.

**Solution:** Decisions append records into `requisition_audit`; UI presents “Audit timeline (immutable)” read-only. No edit/delete endpoint exists for audit rows.

---

## 8) Final approval + inventory deduction atomicity

**Question:** If stock deduction fails during final approval, should approval still be recorded?

**My Understanding/Hypothesis:** No partial success; approval, issue creation, and stock deduction must commit/rollback together.

**Solution:** `finalize_in_transaction` performs final step decision, requisition state transition, stock checks/deduction (`FOR UPDATE`), and issue record creation in one transaction. Any failure rolls back all.

---

## 9) Cart merge semantics for same-store vs cross-store

**Question:** Should separate carts exist per store, or one unified cart that can include cross-store lines?

**My Understanding/Hypothesis:** One active cart per user is simpler offline; mark cross-store for fulfillment messaging.

**Solution:** Current implementation uses one active cart (`active_cart_id`) and sets `has_cross_store=true` when mixed stores are detected. UI warns that cross-store items are fulfilled separately.

---

## 10) Bundle warning behavior (“close to threshold” vs “already reached”)

**Question:** Prompt example says “Add $15.00 more…” (near-threshold), but should warnings appear before threshold is hit?

**My Understanding/Hypothesis:** Near-threshold nudges are ideal UX, but minimum viable rule can warn once threshold is reached.

**Solution:** Current engine warns when bundle threshold quantity is met (`total_qty >= threshold_qty`). “Amount-away” messaging is not yet implemented; future enhancement can compute remaining quantity/value.

---

## 11) Promotion stacking conflict rules

**Question:** How do we resolve conflicts among member pricing, threshold discount, and coupon stacking flags?

**My Understanding/Hypothesis:** First choose best per-line price (member vs threshold), then apply coupon only if stacking rules allow.

**Solution:** Pricing engine picks lower effective line price between member/threshold, then applies one coupon if active/valid and stackability flags permit (`stackable_with_member_price`, `stackable_with_threshold`). Otherwise coupon is blocked with explicit reason.

---

## 12) Daily purchase limit scope

**Question:** Prompt says max 5 units per SKU per day; does this apply per order, per cart line, or across all orders that day?

**My Understanding/Hypothesis:** Must be cumulative per user+SKU+day across all orders.

**Solution:** Enforced in three places: set line hard cap, verify warnings converted to blocking errors, and confirm-time transactional check against `daily_purchase_tracking` before updating quantity.

---

## 13) Verify-before-confirm freshness window

**Question:** How long should verify results remain valid before confirm must re-verify?

**My Understanding/Hypothesis:** Short lock window balances UX with stale-price/stock risk; 10 minutes is reasonable offline LAN default.

**Solution:** Verification snapshot TTL is 600 seconds. Confirm requires an unexpired snapshot and re-prices inside transaction; if drift detected, user must verify again.

---

## 14) Master-data import whitelist behavior

**Question:** Should unknown columns fail import, or be ignored while processing known columns?

**My Understanding/Hypothesis:** Unknown columns should be ignored (for forward compatibility), but missing required columns should fail the job.

**Solution:** Import pipeline normalizes headers, processes only whitelisted columns, rejects job on missing required columns, and gives line-level rejection reasons for row-level validation failures.

---

## 15) File fingerprint dedup boundary

**Question:** Should duplicate import detection be global or scoped?

**My Understanding/Hypothesis:** Scope dedup by `(fingerprint, entity_type, institution)` to avoid false positives across different entities/institutions.

**Solution:** Duplicate check is scoped exactly to fingerprint + entity_type + institution_id via `file_fingerprints`.

---

## 16) Master-data delete strategy (hard vs soft delete)

**Question:** Prompt does not explicitly require soft delete for master data; should deletions be logical?

**My Understanding/Hypothesis:** For reference tables with strict FK integrity and operational cleanup needs, guarded hard delete is acceptable; audit-sensitive domains use append-only logs instead.

**Solution:** Current CRUD uses hard deletes with referential guards (e.g., prevent deleting semester/class when referenced). No universal `deleted_at` pattern is used in master-data tables.

---

## 17) Account recovery channel in offline deployment

**Question:** Without email/SMS, how is recovery initiated and completed securely?

**My Understanding/Hypothesis:** User can submit recovery request, then admin performs offline identity verification and resets credentials through admin workflow.

**Solution:** Public `POST /auth/recovery/request` stores recovery request without user-enumeration leakage (`ok` response regardless). Fulfillment remains admin-mediated per offline policy.

---

## 18) Signed request rollout compatibility

**Question:** Should strict body-hash signing be enforced immediately, or phased for older clients?

**My Understanding/Hypothesis:** Need phased migration to avoid breaking existing clients.

**Solution:** Implemented signing modes: `compat`, `dual`, `strict`. `dual` supports fallback with warning; strict requires `x-silveroak-body-hash`. Replay protection and timestamp window still enforced in all modes.

---

## 19) Timestamp anti-replay tolerance

**Question:** Prompt specifies ±60 seconds; what if local nodes have slight clock drift?

**My Understanding/Hypothesis:** Keep ±60s as security baseline and require NTP alignment operationally.

**Solution:** Middleware enforces replay window check (`within_replay_window`) and stores signatures (`used_signatures`) to reject replay within window. Operational note: keep host clocks synchronized.

---

## 20) Family-portal privacy boundary for supply summaries

**Question:** Should family supply summary be institution-wide or only resident-linked department data?

**My Understanding/Hypothesis:** Must be resident-scoped only; institution-wide exposure is a privacy leak.

**Solution:** Current query scopes requisitions through resident’s class department and requires explicit consent (`supply_usage`) before access. Response is minimized (no internal notes, approver identities, or dollar amounts).

---

## 21) Requisition audit/read access policy for non-requestors

**Question:** Can any approver read any requisition audit, or only those in their department scope?

**My Understanding/Hypothesis:** Read policy should mirror action scope: owner, in-scope approver, or `scope:any`.

**Solution:** Shared `can_read_requisition` guard is used for requisition detail and issue-record reads: requester OR (`requisitions:approve` + dept scope) OR `scope:any`.

---

## 22) Prompt UI expectation: side-by-side prior comments

**Question:** Prompt mentions side-by-side request details and prior comments in approver inbox; should that be in inbox list view or detail view?

**My Understanding/Hypothesis:** Keep inbox lightweight and show full side-by-side context in detail view to reduce list clutter.

**Solution:** Current UI uses list-style inbox and detailed requisition page with route + audit timeline + action panel. This satisfies access to prior comments, though literal side-by-side inbox rendering can be a UX enhancement.
