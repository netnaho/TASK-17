# Delivery Acceptance & Project Architecture Audit (Static-Only)

- Project: `SilverOak` (Week-2 / TASK-17)
- Audit mode: **Static-only** (no runtime, no Docker, no tests executed, no code modification)
- Auditor: GitHub Copilot
- Timestamp (UTC): 2026-04-09

---

## 1) Final Delivery Verdict

**Verdict: PARTIAL PASS**

The codebase has substantial implementation coverage for core requirements (auth, signed requests, requisitions, orders, family portal, moderation/anomaly endpoints, analytics worker, migrations). However, there is at least one **High-severity acceptance risk** that blocks full confidence:

- **High:** anomaly/moderation trigger functions are present but appear **not wired into runtime request flows** (dead/inert detection paths), so promised proactive detection behavior is likely incomplete in live operation.

Given the above, this delivery is a **Partial Pass** pending targeted remediation + verification.

---

## 2) Scope Boundary & Audit Method

### Included (static evidence only)

- Backend API route wiring, middleware, security primitives, business services.
- Worker scheduler/jobs.
- SQL migrations/schema artifacts.
- Frontend route/pages and API usage patterns.
- Test assets and documentation mapping.

### Excluded (per instruction)

- No application startup.
- No Docker/Compose operations.
- No test execution.
- No DB/runtime behavior validation.

### Confidence labels used

- **Verified Statistically:** directly evidenced in source/docs.
- **Cannot Confirm Statistically:** runtime behavior required to prove claim.
- **Manual Verification Required:** needs end-to-end or operational testing.

---

## 3) Requirement Mapping (High-Level)

| Requirement Theme                                     | Static Status          | Evidence                                                                                                                                                                  |
| ----------------------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Auth + session + signed API model                     | Verified Statistically | `repo/apps/backend-api/src/routes/auth.rs:136-141`, `repo/apps/backend-api/src/routes/mod.rs:18,34`, `repo/apps/backend-api/src/security/middleware.rs:182-184`           |
| Requisition lifecycle + transactional finalization    | Verified Statistically | `repo/apps/backend-api/src/requisitions/service.rs:345,389,397,447,459`                                                                                                   |
| Orders verify/confirm + stock + coupon + daily limits | Verified Statistically | `repo/apps/backend-api/src/orders/service.rs:363,439,495-503,521,536-539`, `repo/apps/backend-api/src/orders/pricing.rs:204-216,324,340`                                  |
| Family consent gates + encrypted wellness notes       | Verified Statistically | `repo/apps/backend-api/src/family/service.rs:458,500,520-523,540,608`                                                                                                     |
| Moderation/anomaly APIs and data model                | Partially Verified     | Endpoint presence: `repo/apps/backend-api/src/routes/moderation.rs:22,34`, `repo/apps/backend-api/src/routes/anomaly.rs:22,35,47`; trigger usage gap: see High issue H-01 |
| Worker analytics/anomaly sweep scheduling             | Verified Statistically | `repo/apps/backend-worker/src/scheduler.rs:11`, `repo/apps/backend-worker/src/jobs/mod.rs:33,41`                                                                          |
| Test artifact availability                            | Verified Statistically | `repo/README.md:255-268`, `repo/API_tests/auth.sh:9-15`, `repo/API_tests/phase6.sh:21-31`                                                                                 |

---

## 4) Section-by-Section Acceptance Judgments

Judgment scale used in this report: **Pass / Partial Pass / Fail**.

## 4.1 Security & Access Control

**Judgment: PASS**

- Signed auth applied to protected API scope (`/api/v1`) and wrapped with `SignedAuth` middleware: `repo/apps/backend-api/src/routes/mod.rs:18,34`.
- Replay/timestamp window enforced (`timestamp outside replay window`): `repo/apps/backend-api/src/security/middleware.rs:182-184`.
- Lockout threshold and login failure handling present: `repo/apps/backend-api/src/security/constants.rs:10`, `repo/apps/backend-api/src/routes/auth.rs:100`.
- Session cookie + token issuance in login response: `repo/apps/backend-api/src/routes/auth.rs:129-132`, `136-141`.

Caveats moved to issue list (query/body signing surface, in-memory rate limiting).

## 4.2 Requisition Workflow & Stock-Issue Control

**Judgment: PASS**

- Approval path + finalization path are explicit and transactional: `repo/apps/backend-api/src/requisitions/service.rs:345,381,389,397,459`.
- Issue records inserted during finalization: `repo/apps/backend-api/src/requisitions/service.rs:447`.

Runtime business-correctness still requires live tests (not executed here).

## 4.3 Orders/Pricing/Checkout Integrity

**Judgment: PASS**

- Verify-before-confirm flow exists: `repo/apps/backend-api/src/orders/service.rs:363,439`.
- Confirm enforces stock checks and `FOR UPDATE` locking before deduction: `repo/apps/backend-api/src/orders/service.rs:495-503,521`.
- Coupon + bundle + daily limit calculations implemented: `repo/apps/backend-api/src/orders/pricing.rs:204-216,324,340`.

## 4.4 Family Portal & Privacy/Consent Controls

**Judgment: PASS**

- Consent gate for wellness summary: `repo/apps/backend-api/src/family/service.rs:458`.
- Notes encrypted at rest before persistence: `repo/apps/backend-api/src/family/service.rs:520-523,540`.
- DTO intentionally excludes encrypted notes from responses: `repo/apps/backend-api/src/family/service.rs:608`.

## 4.5 Moderation/Anomaly Detection & Compliance

**Judgment: PARTIAL PASS (High-risk gap)**

- Endpoints and services exist (`/moderation/*`, `/anomaly/*`): `repo/apps/backend-api/src/routes/moderation.rs:22,34`, `repo/apps/backend-api/src/routes/anomaly.rs:22,35,47`.
- But critical trigger functions appear unreferenced outside their own definitions:
  - `record_auth_failure`: `repo/apps/backend-api/src/anomaly/service.rs:122`
  - `record_permission_denial`: `repo/apps/backend-api/src/anomaly/service.rs:175`
  - `record_large_upload`: `repo/apps/backend-api/src/anomaly/service.rs:239`
  - `check_and_flag`: `repo/apps/backend-api/src/moderation/service.rs:75`
  - Global search found only these definitions (no call sites): `repo/**/*.rs` search result.

This materially weakens acceptance confidence for proactive detection behavior.

## 4.6 Architecture, Deployability, and Operational Readiness

**Judgment: PARTIAL PASS**

- Compose profiles and runbook docs exist: `repo/README.md:13,23,31,255-268`.
- Worker schedule mechanism exists (`60s` tick): `repo/apps/backend-worker/src/scheduler.rs:11`.
- Static concern: `.env.example` defaults to running migrations and seeds (`true`) which can be risky if copied unchanged outside dev: `repo/.env.example:7-8`.

---

## 5) Material Issues (Severity-Rated)

## H-01 (High) — Anomaly/Moderation trigger logic appears not wired into runtime paths

- **Impact:** Detection/flagging may not execute in real request flows despite endpoints/UI existing.
- **Evidence:** Only function definitions found for critical triggers:
  - `repo/apps/backend-api/src/anomaly/service.rs:122,175,239`
  - `repo/apps/backend-api/src/moderation/service.rs:75`
  - Global source scan for `record_auth_failure(` / `record_permission_denial(` / `record_large_upload(` / `check_and_flag(` returned only definition hits.
- **Acceptance risk:** High risk of “feature present in codebase but inert in practice.”
- **Needed verification:** Manual + runtime tracing to confirm invocation points.

## M-01 (Medium) — Frontend can panic on fixed string slicing for timestamps

- **Impact:** UI crash risk if backend returns unexpected datetime string length/format.
- **Evidence:** direct slicing without guards:
  - `repo/apps/frontend-yew/src/pages/moderation.rs:308,372`
  - `repo/apps/frontend-yew/src/pages/security_events.rs:221`
  - `repo/apps/frontend-yew/src/pages/orders.rs:75`
  - `repo/apps/frontend-yew/src/pages/order_detail.rs:87`

## M-02 (Medium) — Moderation status filter may fetch stale value (state update race)

- **Impact:** queue may reload using previous filter value right after dropdown change.
- **Evidence:** immediate emit after state set:
  - `repo/apps/frontend-yew/src/pages/moderation.rs:230-231`
- **Note:** This is a UX/consistency bug, not a direct security blocker.

## M-03 (Medium) — Rate limiter is in-memory/process-local

- **Impact:** horizontal scale can bypass effective global throttling; restarts reset counters.
- **Evidence:** `HashMap + Mutex` in memory store:
  - `repo/apps/backend-api/src/security/rate_limit.rs:1,5,17,25`

## M-04 (Medium) — Signature design excludes query/body from signed surface

- **Impact:** request integrity guarantees are narrower than many HMAC API designs.
- **Evidence:**
  - canonical/body note: `repo/apps/backend-api/src/security/signing.rs:37`
  - replay test comment explicitly notes query not part of signature: `repo/apps/backend-api/src/security/signing.rs:235-237,244`
- **Classification:** security design debt; may be intentional but should be explicitly risk-accepted.

## L-01 (Low) — `.env.example` enables migrations/seeds by default

- **Impact:** accidental seed/migration execution risk if copied to non-dev usage.
- **Evidence:** `repo/.env.example:7-8`.

---

## 6) Security Summary

### Strengths (static)

- Signed request middleware and replay-window checks in protected scope (`/api/v1`): `repo/apps/backend-api/src/routes/mod.rs:18,34`, `repo/apps/backend-api/src/security/middleware.rs:182-184`.
- Account lockout threshold constants and lockout path present: `repo/apps/backend-api/src/security/constants.rs:10`, `repo/apps/backend-api/src/routes/auth.rs:100`.
- Session cookie issuance with HTTP-only and SameSite flags: `repo/apps/backend-api/src/routes/auth.rs:129-132`.

### Security caveats

- Detection triggers likely unwired (H-01).
- In-memory rate limiting (M-03).
- Signed surface limitations (M-04).

### Static-only confidence

- **Cannot Confirm Statistically:** effective replay resistance under production proxy topology.
- **Manual Verification Required:** lockout edge cases, signature tamper tests, moderation/anomaly event generation under abusive traffic.

---

## 7) Tests & Logging Review (Static)

### Available test assets (not executed)

- Test orchestration documented: `repo/README.md:255-268`.
- Auth/security API test intentions include replay/lockout/scope: `repo/API_tests/auth.sh:9-15,121,147`.
- Phase-6 test intentions include family/moderation/anomaly: `repo/API_tests/phase6.sh:21-31,271-342`.

### Logging/audit evidence

- Moderation audit inserts exist: `repo/apps/backend-api/src/moderation/service.rs:136,264`.
- Anomaly permission-denial signal writes to `audit_log`: `repo/apps/backend-api/src/anomaly/service.rs:185,196`.

### Gap callout

- Presence of test scripts does not prove pass/fail outcomes in this audit (not run by requirement).

---

## 8) Static Test Coverage Mapping Table

| Risk Point                                  | Static Tests/Artifacts Present                                 | Coverage Status       | Notes                                                     |
| ------------------------------------------- | -------------------------------------------------------------- | --------------------- | --------------------------------------------------------- |
| Signed requests, replay, lockout, scope     | `API_tests/auth.sh` (`9-15`, `121`, `147`)                     | Partial (static-only) | Script exists; execution result unavailable.              |
| Requisition lifecycle + approval + rollback | `API_tests/requisitions.sh` (header intent and scenario steps) | Partial (static-only) | Runtime DB checks not performed.                          |
| Orders verify/confirm + pricing + limits    | `API_tests/orders.sh`; README matrix `repo/README.md:266`      | Partial (static-only) | Assertions not executed here.                             |
| Family consent + wellness privacy           | `API_tests/phase6.sh:8-15,369-403`                             | Partial (static-only) | Strong scenario intent.                                   |
| Moderation queue/policies                   | `API_tests/phase6.sh:271-316`                                  | Partial (static-only) | Endpoint tests exist.                                     |
| Anomaly list/rules/ack                      | `API_tests/phase6.sh:324-361`                                  | Partial (static-only) | Tests focus on API behavior, not trigger generation path. |
| Cross-crate Rust unit scaffold              | `repo/unit_tests/` + README mention `repo/README.md:93`        | Low assurance         | Minimal evidence of broad coverage depth.                 |

---

## 9) Final Notes & Required Follow-Ups

1. **Fix before full sign-off:** wire anomaly/moderation trigger functions into concrete runtime points (auth failures, permission denials, content ingress paths), then prove with targeted integration tests and trace logs.
2. Replace unsafe frontend timestamp slicing with parsing/formatting guards.
3. Decide and document explicit risk acceptance (or redesign) for signature coverage (query/body).
4. Introduce distributed/global throttling if multi-instance deployment is expected.
5. Keep this audit classified as static-only; perform a separate runtime acceptance pass for final release.

---

## Requirements Coverage (Audit Task)

- Static-only audit, no runtime execution: **Done**.
- No code modifications: **Done**.
- Material issue discovery with severity and evidence: **Done**.
- Security, tests/logging, and acceptance mapping sections: **Done**.
- Uncertain/runtime claims marked appropriately: **Done**.
- Report written to `./.tmp/**.md`: **Done** (`.tmp/delivery_acceptance_architecture_audit_static.md`).
