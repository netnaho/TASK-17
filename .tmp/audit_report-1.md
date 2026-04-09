# SilverOak TASK-17 — Delivery Acceptance + Project Architecture Audit (Static Only)

## 1) Audit Context, Scope, and Method

- **Mode:** Static-only code/document review.
- **Constraints honored:** No project startup, no Docker run, no test execution, no code edits.
- **Repository root:** `repo/`
- **Evidence standard:** Strong claims are backed with `file:line` citations.
- **What “cannot be proven statically” means:** Runtime behavior, integration success, performance, and operational characteristics are marked as **Cannot Confirm Statically**.

---

## 2) Executive Acceptance Verdict

**Overall verdict: PARTIAL ACCEPTANCE (Not production-ready).**

The codebase demonstrates strong architecture and many implemented controls (transactional issuance, immutable requisition audit table trigger, consent gates, signed middleware, replay window, rate limiting, encryption helpers). However, there are **material security and delivery risks** that block full acceptance:

- **High:** Signature canonicalization mismatch risk between frontend and backend on query-bearing routes.
- **High:** Approval action authorization gap (role/permission check without explicit department scope enforcement in approve/reject/send-back paths).
- **High:** Issue-record endpoint exposes requisition-linked issuance data with only “principal exists” check.
- **High:** Analytics read paths accept arbitrary `institution_id` with permission check only (no explicit scope binding).
- **High:** Seed SQL for requisitions uses stale column names vs migration schema; with seeds enabled in compose, this can break environment bootstrap.
- **High:** Reverse proxy is HTTP-only (no TLS termination in provided nginx config).

---

## 3) Acceptance Section A — Requisition / Approval / Issue Flow

### Decision: **PARTIAL PASS**

### What is implemented well

- Multi-step approval model exists with current-step resolution and role-based step checks.
  - `apps/backend-api/src/requisitions/service.rs:302`
  - `apps/backend-api/src/requisitions/service.rs:310`
- Final approval path is wrapped in a **single DB transaction**, with stock row locking, stock deduction, issue record creation, and audit writes before commit.
  - `apps/backend-api/src/requisitions/service.rs:343`
  - `apps/backend-api/src/requisitions/service.rs:370`
  - `apps/backend-api/src/requisitions/service.rs:380`
  - `apps/backend-api/src/requisitions/service.rs:401`
  - `apps/backend-api/src/requisitions/service.rs:413`
- Immutable audit timeline is enforced at DB level via trigger blocking `UPDATE/DELETE`.
  - `migrations/0003_requisitions.sql:149`
  - `migrations/0003_requisitions.sql:152`
  - `migrations/0003_requisitions.sql:157`

### Material gaps

1. **High — Approval action scope gap**
   - Approve/reject/send-back enforce permission + required role at step but do not explicitly enforce department scope for the target requisition in those mutating endpoints.
   - Evidence:
     - Action checks: `apps/backend-api/src/requisitions/service.rs:308`, `:310`, `:423`, `:433`, `:458`, `:467`
     - Inbox has department scope filter (contrast): `apps/backend-api/src/requisitions/service.rs:520-521`
   - Impact: A principal with requisition-approve permission and matching role may act beyond intended department scope depending on role/scopes assignment.
   - Minimum fix: For approve/reject/send-back, fetch requisition `department_id` and enforce same scope predicate used by inbox/read path before mutation.

2. **High — Issue record endpoint authorization too weak**
   - Endpoint only validates principal presence and then returns issue record + lines.
   - Evidence:
     - `apps/backend-api/src/routes/requisitions.rs:170` (route)
     - `apps/backend-api/src/routes/requisitions.rs:175` (`let _p = principal(&req)?;`)
   - Impact: Potential disclosure of issue records across unauthorized users if requisition IDs are discovered.
   - Minimum fix: Enforce owner/approver/admin + scope authorization equivalent to `load_detail` before returning issue records.

---

## 4) Acceptance Section B — Orders / Cart / Pricing / Confirmation

### Decision: **PASS (with caveats)**

### Strengths

- Verification snapshot model implemented with TTL and confirm-time re-price guard.
  - `apps/backend-api/src/orders/service.rs:12`
  - `apps/backend-api/src/orders/service.rs:363`
  - `apps/backend-api/src/orders/service.rs:421`
  - `apps/backend-api/src/orders/service.rs:446`
  - `apps/backend-api/src/orders/service.rs:490`
- Confirm path uses transactional row locks for cart lines/stock/daily limits.
  - `apps/backend-api/src/orders/service.rs:468`
  - `apps/backend-api/src/orders/service.rs:498`
  - `apps/backend-api/src/orders/service.rs:509`
- Object-level order read check exists (owner or broad scope).
  - `apps/backend-api/src/orders/service.rs:624`
  - `apps/backend-api/src/orders/service.rs:639-646`

### Caveat

- Runtime correctness/performance under concurrent load remains **Cannot Confirm Statically**.

---

## 5) Acceptance Section C — Master Data / Import-Export

### Decision: **PARTIAL PASS**

### What is implemented well

- Import fingerprint de-duplication exists.
  - `apps/backend-api/src/master_data/import.rs:173`
  - `apps/backend-api/src/master_data/import.rs:181`
  - `apps/backend-api/src/master_data/import.rs:200`
- Required-column validation and accepted/rejected row accounting exist.
  - `apps/backend-api/src/master_data/import.rs:240`
  - `apps/backend-api/src/master_data/import.rs:245`
  - `apps/backend-api/src/master_data/import.rs:310`

### Material gap

1. **High — Seed/migration requisition schema mismatch (bootstrap risk)**
   - Seed uses `requisition_audit(action, note)` and requisition/requisition_line insert assumptions that conflict with migration schema (`event_kind`, `comment`, `quantity`, etc.).
   - Evidence:
     - Seed inserts: `crates/infrastructure/src/seed.rs:566`, `:574`, `:598`, `:606`, `:670`
     - Migration columns: `migrations/0003_requisitions.sql:102`, `:140`, `:141`
   - Additional risk amplifier: compose enables seeds by default.
     - `docker-compose.yml:32`
   - Impact: Seed phase can fail or drift from intended schema, reducing environment reliability.
   - Minimum fix: Align seed SQL exactly to migrated schema and add compile-time/query tests for seed statements.

---

## 6) Acceptance Section D — Family / Consent / Sensitive Data Handling

### Decision: **PASS (static design), with runtime caveat**

### Strengths

- Consent checks are enforced before summary/log endpoints.
  - `apps/backend-api/src/family/service.rs:188`
  - `apps/backend-api/src/family/service.rs:385`
  - `apps/backend-api/src/family/service.rs:458`
  - `apps/backend-api/src/family/service.rs:615`
- Sensitive notes are encrypted at write path and intentionally excluded from response DTOs.
  - `apps/backend-api/src/family/service.rs:101`
  - `apps/backend-api/src/family/service.rs:451`
  - `apps/backend-api/src/family/service.rs:523`
- Field-level encryption helper is documented/implemented as AES-256-GCM with key from env.
  - `crates/infrastructure/src/crypto.rs:1`
  - `crates/infrastructure/src/crypto.rs:4`
  - `crates/infrastructure/src/crypto.rs:124`

### Caveat

- Cryptographic key management quality in real deployment (rotation/KMS/HSM) is **Cannot Confirm Statically**.

---

## 7) Acceptance Section E — Analytics / Moderation / Anomaly

### Decision: **PARTIAL PASS**

### Strengths

- Analytics daily/weekly/monthly/dashboard service surface is implemented.
  - `apps/backend-api/src/analytics/service.rs:60`
  - `apps/backend-api/src/analytics/service.rs:89`
  - `apps/backend-api/src/analytics/service.rs:118`
  - `apps/backend-api/src/analytics/service.rs:153`
- Moderation + anomaly include audit-oriented writes.
  - `apps/backend-api/src/moderation/service.rs:136`
  - `apps/backend-api/src/anomaly/service.rs:185`

### Material gap

1. **High — Analytics institution-level access control gap**
   - Methods require `analytics:read` but accept caller-provided `institution_id` without explicit scope enforcement.
   - Evidence:
     - Permission checks: `apps/backend-api/src/analytics/service.rs:63`, `:92`, `:121`, `:155`
     - Queries filter by passed `institution_id`: `apps/backend-api/src/analytics/service.rs:68`, `:97`, `:132`, `:161`
   - Impact: Cross-institution analytics data exposure risk if token has read permission but insufficient institution scoping.
   - Minimum fix: Enforce institution/site/department scope binding before executing queries.

---

## 8) Acceptance Section F — Security Architecture, Config, and Prompt-Fit

### Decision: **PARTIAL PASS**

### Implemented controls (static)

- Signed-auth middleware with timestamp/nonce/signature checks and replay recording.
  - `apps/backend-api/src/security/middleware.rs:205`
  - `apps/backend-api/src/security/middleware.rs:212`
- Middleware wiring present for protected routes.
  - `apps/backend-api/src/routes/mod.rs:34`

### Material gaps

1. **High — Frontend/backend signature canonicalization mismatch risk**
   - Frontend signs full path variable as provided.
     - `apps/frontend-yew/src/auth/client.rs:37`, `:53`
   - Backend verifies canonical path via `req.uri().path()` (query stripped).
     - `apps/backend-api/src/security/middleware.rs:205`, `:207`
   - API test scripts explicitly strip query before signing (indicates expected canonical behavior differs from generic frontend call pattern).
     - `API_tests/orders.sh:46-47`
     - `API_tests/requisitions.sh:40-41`
     - `API_tests/phase6.sh:54,58`
   - Impact: Query-bearing frontend calls may fail signature verification intermittently/consistently depending on caller path usage.
   - Minimum fix: Normalize canonicalization consistently on both client and server (strip query before signing and document contract).

2. **High — HTTP-only reverse proxy in provided config**
   - Evidence:
     - `proxy/nginx.conf:14` (`listen 80;`)
     - `proxy/nginx.conf:18` (`proxy_pass http://silveroak_api;`)
   - Impact: No TLS termination in supplied deployment artifact; data in transit protection not met unless externally terminated.
   - Minimum fix: Add TLS server block/certs handling or document required upstream TLS termination and enforce secure headers.

3. **Medium — Security-relevant default relaxation in compose**
   - Evidence:
     - `docker-compose.yml:38` (`RATE_LIMIT_PER_MIN: "600"`)
     - `docker-compose.yml:39` (`DAILY_LIMIT_PER_SKU: "500"`)
     - `docker-compose.yml:32` (`APP__RUN_SEEDS: "true"`)
   - Impact: Non-production-safe defaults may mask abuse/limit behavior during acceptance and increase accidental exposure.
   - Minimum fix: Provide explicit secure/dev profile split and production-default overrides.

4. **Medium — Documentation/config drift**
   - README lists postgres host port as 5432 while compose maps 5433:5432; README says migrations `0001–0006` though repo has `0007`.
   - Evidence:
     - `README.md:43`
     - `docker-compose.yml:13`
     - `README.md:61`
     - `migrations/0007_moderation_audit_compat.sql:1`
   - Impact: Operator confusion and onboarding errors.
   - Minimum fix: Update README to current ports/migration range.

---

## 9) Risk Register, Static Test Coverage Mapping, and Final Recommendation

## 9.1 Severity-ranked issue list

| Severity   | Issue                                                   | Evidence                                                                                                                         | Acceptance impact                      |
| ---------- | ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| **High**   | Signature canonical mismatch risk (query path handling) | `apps/frontend-yew/src/auth/client.rs:37,53`; `apps/backend-api/src/security/middleware.rs:205,207`; `API_tests/orders.sh:46-47` | Security/auth reliability blocker      |
| **High**   | Requisition action scope enforcement gap                | `apps/backend-api/src/requisitions/service.rs:308,310,423,433,458,467`; contrast `:520-521`                                      | Object-level authz risk                |
| **High**   | Issue record endpoint lacks granular authz              | `apps/backend-api/src/routes/requisitions.rs:170,175`                                                                            | Data isolation risk                    |
| **High**   | Analytics scope gap on institution_id                   | `apps/backend-api/src/analytics/service.rs:63,68,92,97,121,132,155,161`                                                          | Cross-tenant data risk                 |
| **High**   | Seed SQL vs migration schema mismatch                   | `crates/infrastructure/src/seed.rs:566,574,598,606,670`; `migrations/0003_requisitions.sql:102,140,141`                          | Delivery/bootstrap reliability blocker |
| **High**   | HTTP-only proxy (no TLS in supplied config)             | `proxy/nginx.conf:14,18`                                                                                                         | Transport security gap                 |
| **Medium** | Security defaults loosened in compose                   | `docker-compose.yml:32,38,39`                                                                                                    | Hardening gap                          |
| **Medium** | README drift vs actual config/schema                    | `README.md:43,61`; `docker-compose.yml:13`; `migrations/0007_moderation_audit_compat.sql:1`                                      | Ops/documentation quality              |

## 9.2 Static test coverage mapping

| Area                                | Static evidence of test assets                                                                                                                      | Coverage judgment (static)                                         |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Auth + signed requests              | Canonical signing helper + login/bootstrap in scripts (`API_tests/orders.sh:43-50`, `API_tests/requisitions.sh:37-44`, `API_tests/phase6.sh:53-59`) | **Present**, runtime results not verified                          |
| Requisitions/approvals              | Scenario list in script headers (`API_tests/requisitions.sh:5-18`)                                                                                  | **Present**, strong intent coverage                                |
| Orders flow                         | Scenario list incl verify/confirm/limits (`API_tests/orders.sh:5-24`)                                                                               | **Present**, runtime not confirmed                                 |
| Family/analytics/moderation/anomaly | Phase 6 matrix and routes (`API_tests/README.md`, section “Phase 6”)                                                                                | **Present**, runtime not confirmed                                 |
| Unit tests                          | DTO/domain/application shape tests (`unit_tests/tests/cross_crate.rs`)                                                                              | **Limited** (mostly model/serialization, not business integration) |

## 9.3 Final recommendation

- **Acceptance status:** **Not accepted for production as-is**.
- **Required before acceptance:** Resolve all **High** findings above, then run targeted integration/security tests and re-audit.
- **Static-only note:** No runtime assertions were made beyond direct code/document evidence.

---

### Requirements coverage (audit task)

- Static-only review with no startup/tests/code edits: **Done**
- Evidence-backed findings with `file:line`: **Done**
- Security-first review: **Done**
- Severity-ranked issues: **Done**
- Test/logging static mapping: **Done**
- Deliver report under `./.tmp/**.md`: **Done** (`repo/.tmp/delivery_acceptance_project_architecture_audit.md`)
