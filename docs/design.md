# SilverOak Operations Suite — Design Document

## 1. Purpose and Scope

This document describes the current system design of the SilverOak Operations Suite as implemented in the `repo/` workspace.

The platform supports offline-first local-network operations for a multi-site care institution, with these core domains:

- Procurement requisitions and approval routing
- Consumable ordering and checkout validation
- Inventory governance and outbound issue records
- Master-data CRUD + import/export with validation
- Family visibility (consent-gated)
- Security, moderation, anomaly detection, and analytics

This design is implementation-first: it reflects existing Rust/Yew/PostgreSQL behavior in the repository, including known constraints and deliberate trade-offs.

---

## 2. Product Roles and Responsibility Model

### 2.1 Primary user roles

- **Institution Admin**
  - Manages master data, users/roles/scope, blacklist, and operational settings
  - Reviews analytics and platform-wide security/admin tools
- **Medical Staff / Requester**
  - Creates requisitions and places consumable orders
  - Submits/withdraws requisitions and tracks status
- **Approver (Department / Finance)**
  - Works approval inbox
  - Approves/rejects/send-back with audit comments
- **Senior / Family Member**
  - Uses shopping flow where permitted
  - Views family portal summaries under explicit consent controls

### 2.2 Access-control model

Authorization is a combination of:

- **RBAC permissions** (menu/API capability), and
- **Data scopes** (`institution`, `site`, `department`, `family_group`)

Access is denied unless both capability and scope constraints are satisfied (except `scope:any` and wildcard admin paths).

---

## 3. High-Level Architecture

## 3.1 Runtime topology

The system is deployed as local-network containers via Docker Compose:

- `proxy` (nginx) — unified entrypoint, TLS termination in prod profile
- `backend-api` (Actix-web) — REST APIs and domain logic
- `backend-worker` (Tokio scheduler) — recurring aggregation/validation jobs
- `frontend-yew` (WASM + nginx) — SPA UI
- `postgres` — durable persistence

## 3.2 Codebase structure

- `apps/backend-api/` — API, business logic, auth middleware, routes
- `apps/backend-worker/` — scheduled jobs
- `apps/frontend-yew/` — Yew pages/components/auth client
- `crates/domain/` — domain-level primitives
- `crates/application/` — application interfaces/use-case contracts
- `crates/infrastructure/` — DB, crypto, migrations, seeding
- `crates/shared/` — cross-boundary shared DTO/util types
- `migrations/` — additive SQL migrations
- `API_tests/` + `unit_tests/` — integration and unit test suites

---

## 4. Backend Design (Actix-web)

## 4.1 API composition

Main bootstrap lives in `apps/backend-api/src/main.rs`.

Request handling chain:

1. `IpGuard` middleware: blacklist + per-IP rate limit
2. `SignedAuth` middleware (for protected scope): token + signature + replay checks
3. Route handlers under `/api/v1/*`

Public endpoints (e.g., login/health/recovery request) are intentionally outside signed-auth scope.

## 4.2 Module boundaries

Backend modules are organized by domain:

- `requisitions/`
- `orders/`
- `master_data/`
- `family/`
- `analytics/`
- `moderation/`
- `anomaly/`
- `security/`
- `routes/` (HTTP wiring)

Business rules are concentrated in `service.rs` files; route files are thin transport layers.

## 4.3 Error model

`ApiAppError` provides structured HTTP failure modes (`BadRequest`, `Unauthorized`, `Forbidden`, `Conflict`, `NotFound`, `RateLimited`, `Internal`).

This keeps domain/service failures consistent across all route handlers.

---

## 5. Frontend Design (Yew)

## 5.1 UI architecture

Yew SPA located in `apps/frontend-yew/src/`:

- `pages/` — feature pages (requisitions, approvals, cart, checkout, master data, etc.)
- `auth/` — session/token state and signed request client
- `layout/` — shell and navigation
- `components/` — reusable UI primitives

## 5.2 API consumption strategy

Frontend uses a signed API client (`auth/client.rs`) for protected endpoints.

Each page uses async fetch + local state pattern:

- load initial data
- call mutation endpoint
- reload view model from server

## 5.3 Notable UX behavior (current)

- Requisition form creates draft then auto-submits
- Approval inbox is list-centric; full route/comments shown in detail page
- Cart supports cross-store line mixing with warning
- Checkout requires verify before confirm

---

## 6. Data and Persistence Design (PostgreSQL)

## 6.1 Persistence principles

- PostgreSQL is system of record
- Migrations are versioned and additive (`migrations/0001` ... `0008`)
- Sensitive fields use field-level encryption where required (e.g., wellness notes)

## 6.2 Core entity groups

### Procurement / inventory

- `requisitions`
- `requisition_lines`
- `requisition_approval_instances`
- `requisition_approval_steps`
- `requisition_audit`
- `issue_records`
- `issue_record_lines`
- `inventory_items`, `inventory_categories`, `inventory_stock_by_site`

### Ordering / checkout

- `carts`, `cart_lines`
- `orders`, `order_lines`, `order_adjustments`
- `product_catalog`, `product_pricing`, `product_threshold_discounts`
- `coupons`, `delivery_methods`
- `order_verification_snapshots`
- `daily_purchase_tracking`

### Master data

- `students`, `classes`, `courses`, `semesters`, `departments`
- Import tracking: `import_jobs`, `import_job_rows`, `file_fingerprints`

### Security / access

- `users`, `roles`, `permissions`, `role_permissions`, scopes mappings
- `sessions`, `api_tokens`, `login_attempts`, `account_lockouts`
- `used_signatures`, `ip_blacklist`

### Family / compliance / moderation / analytics

- `family_groups`, `family_group_members`, `family_group_residents`, `consent_records`
- `wellness_activities`, `wellness_personal_bests`
- moderation queue/policies/audit tables
- anomaly event/rule tables
- `analytics_daily`, `analytics_weekly`, `analytics_monthly`, consistency reports

---

## 7. Key Business Workflow Designs

## 7.1 Requisition lifecycle

State machine (implemented in requisition domain logic):

`draft -> pending_approval -> approved_final -> issued`

Alternative transitions:

- `pending_approval -> rejected`
- `pending_approval -> sent_back`
- `draft/pending_approval/sent_back -> withdrawn` (per allowed transition rules)

### Invariants

- `needed_by` cannot be in the past
- non-empty justification required
- at least one line with quantity > 0
- approver action requires permission + step role + department scope
- reject/send-back require non-empty reason
- audit entries are append-only

### Finalization atomicity

Final approval executes in one DB transaction:

- lock requisition + stock rows
- verify stock
- deduct stock
- create issue record + lines
- transition status and write audit entries

On any failure, entire transaction is rolled back.

## 7.2 Approval routing engine

Routing is table-driven:

- workflow + nodes + conditional rules
- conditions include spend threshold and controlled-category flags

Routing snapshot from requisition (`total_amount_cents`, `contains_controlled`) decides step chain at submit time.

## 7.3 Consumable ordering and checkout

### Pricing pipeline

Per line:

1. Base price
2. Member price candidate
3. Threshold discount candidate
4. Choose best effective line price

Order level:

5. Apply coupon if valid and stackable
6. Add delivery fee
7. Return warnings (bundle/daily-limit)

### Rule enforcement

- one coupon per cart
- stacking flags enforced
- daily limit enforced cumulatively per user+SKU+day
- cross-store cart represented with `has_cross_store`

### Verify/confirm flow

- `verify`: re-price + stock checks + limit checks, persist snapshot (TTL 10 min)
- `confirm`: requires valid snapshot, re-price in transaction, reject on drift, then deduct stock and create order

## 7.4 Master data import/export

### Import flow

- multipart file upload
- MIME/extension whitelist
- size limit
- SHA-256 fingerprint duplicate detection per entity+institution
- header validation (required columns)
- row-level validation with partial success behavior
- per-row rejection reasons persisted and returned via job tracking

### Export flow

- entity export to CSV/XLSX
- content disposition for attachment downloads

## 7.5 Family visibility and consent

Data access is consent-gated by category:

- `supply_usage`
- `wellness_summary`
- `activity_log`

Privacy model returns minimized views (no encrypted raw notes, no internal-only sensitive data).

---

## 8. Security and Compliance Design

## 8.1 Authentication/session model

- login via username/email + Argon2 hash verification
- lockout: 5 failed attempts within window -> temporary lock
- API token issuance with short TTL (~15 min)
- session cookie for session-aware endpoints
- logout revokes both session and active API token linkage

## 8.2 Signed request model

Protected endpoints require:

- `x-silveroak-token`
- `x-silveroak-timestamp`
- `x-silveroak-nonce`
- `x-silveroak-signature`
- optional/required `x-silveroak-body-hash` depending on signing mode

Replay controls:

- timestamp window enforcement (±60s)
- used-signature recording to block duplicate signed requests

Compatibility rollout modes:

- `compat`
- `dual`
- `strict`

## 8.3 Perimeter controls

- per-IP rate limiting (configurable backend)
- IP blacklist check prior to auth logic
- CORS rules for local deployment origins
- TLS termination in production compose profile

## 8.4 Sensitive-data controls

- field-level encryption for wellness notes (AES-256-GCM via infra crypto)
- consent checks for family-facing data exposure
- moderation queue for flagged content
- anomaly event recording for auth bursts/permission denials/large uploads

---

## 9. Worker and Analytics Design

`apps/backend-worker` runs scheduled jobs (Tokio scheduler), including:

- daily/weekly/monthly stats rollups
- personal bests calculations
- consistency validation jobs
- anomaly sweep/maintenance jobs

No external SaaS dependency is required for scheduler execution.

---

## 10. Deployment and Environment Strategy

## 10.1 Environment profiles

- Dev profile: local HTTP, migrations + seeds enabled
- Prod-like profile: TLS through proxy, seeds disabled

## 10.2 Operational configuration

Config includes:

- DB URL and migration toggles
- signing mode
- rate-limit backend mode
- cookie security flag
- encryption keys and security constants

Compose files in root:

- `docker-compose.yml`
- `docker-compose.dev.yml`
- `docker-compose.prod.yml`

---

## 11. Observability and Quality Gates

## 11.1 Logging

- structured JSON logs from backend startup/runtime
- warnings for fallback/compat security paths and async side-channel failures

## 11.2 Test strategy

- unit tests in module-level `#[cfg(test)]` blocks
- API integration scripts under `API_tests/`
- orchestration script: `run_tests.sh`

Coverage focuses on:

- state transitions
- pricing and stacking rules
- security signing/replay/rate limits
- import validation and family consent paths

---

## 12. Explicit Design Decisions and Known Boundaries

These are intentional current-state decisions (many captured in `docs/questions.md`):

1. API date contracts use ISO format; display format can differ in UI.
2. Requisition flow remains draft-first internally, even when UI feels one-step.
3. Approval decisions require both role and scope checks.
4. Audit timeline is append-only and read-only.
5. Bundle warnings currently trigger at threshold reached (not near-threshold deltas).
6. Master-data deletes are guarded hard deletes, not global soft deletes.
7. Verify-before-confirm uses a short-lived snapshot with confirm-time revalidation.

---

## 13. Future Enhancements (Design Backlog)

Potential next design increments:

- Near-threshold pricing prompts (e.g., “add $X more”)
- Richer side-by-side approver inbox layout
- Stronger distributed rate-limit defaults in multi-instance deployments
- Optional soft-delete strategy for selected admin entities
- Expanded conflict-resolution telemetry for ID mapping and nightly consistency jobs
