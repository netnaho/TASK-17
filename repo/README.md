# SilverOak Operations Suite

> **Project Type: fullstack**
> (Rust Actix-web backend API + Yew/WASM frontend, served behind an nginx proxy,
> backed by PostgreSQL, orchestrated with Docker Compose.)

Operations platform for assisted-living institutions: requisitions, approvals,
inventory, consumable ordering, master data, analytics, family portal, content
moderation, and anomaly detection. Designed for single-command startup and
offline-first, local-network deployment.

## Quick start

The whole platform is Docker-only — you do not need Rust, Node, `jq`, `curl`,
`openssl`, or Postgres installed on the host.  Docker (with the Compose plugin)
is the single dependency.

### Fastest path — one command

Set the default compose overlay once, then use the plain verb:

```bash
export COMPOSE_FILE=docker-compose.yml:docker-compose.dev.yml
docker-compose up
```

The literal `docker-compose up` command starts Postgres, the backend API, the
worker, the Yew/WASM frontend, and the nginx proxy.  The modern CLI syntax
works identically:

```bash
docker compose up
```

### Explicit overlay (no shell exports)

```bash
docker compose -f docker-compose.yml -f docker-compose.dev.yml up --build
```

Migrations run automatically on first boot; demo seed data is loaded because
`APP__RUN_SEEDS=true` is set in the dev override.

### Prod-like mode (TLS termination, seeds disabled)

Provide TLS cert/key files before starting (see [TLS certificates](#tls-certificates)):

```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml up --build
```

Traffic flows: `HTTPS :443 → proxy → backend-api / frontend`.
HTTP `:80` is accepted only to redirect to HTTPS.  Seeds are **off** by default.

---

## How to access the running system

Once `docker-compose up` reports every container healthy, reach the suite at:

| What | URL | Port |
|---|---|---|
| **Web UI (SPA)** | <http://localhost:8000> | **8000** (unified proxy) |
| **Direct backend API** | <http://localhost:8080/api/v1> | **8080** |
| **Direct frontend bundle** | <http://localhost:8081> | **8081** |
| **Postgres** | `postgres://silveroak:silveroak_dev@localhost:5433/silveroak` | **5433** |

The **primary entry point is <http://localhost:8000>** — the proxy serves the
Yew SPA from `/` and transparently forwards `/api/*` to the backend-api
container.  Use the other ports only when debugging a specific service.

Sign in at <http://localhost:8000/login> with any
[demo credential](#demo-credentials) below.

---

## How to verify the system works

After `docker-compose up` returns (or `docker compose ps` reports `healthy`),
run this checklist.  The API verification is a no-install curl smoke test; the
UI verification walks through the happy path in the browser.

### 1 · API verification (curl smoke test)

```bash
# 1. Liveness — must return HTTP 200 with status "ok"
curl -s http://localhost:8000/api/v1/health
# → {"status":"ok"}

# 2. Login — must return an api_token + signing_key
curl -s -X POST http://localhost:8000/api/v1/auth/login \
  -H 'content-type: application/json' \
  -d '{"email":"admin@silveroak.local","password":"ChangeMeNow!2025"}' \
  | python3 -m json.tool
# → {"user_id":"…","roles":["admin"],"api_token":"…","signing_key":"…", …}

# 3. Full API regression — every signed route, true no-mock HTTP
./run_tests.sh
# → "ALL SUITES PASSED" on success
```

A Postman collection lives at `API_tests/README.md`; import any of the
`API_tests/*.sh` scripts as a curl script for manual exploration.

### 2 · UI verification flow (Web)

Expected outcome at each step:

| Step | Action | Expected result |
|---|---|---|
| 1 | Open <http://localhost:8000> | Redirected to `/login` (since no session) |
| 2 | Sign in as `admin@silveroak.local` / `ChangeMeNow!2025` | Landed on dashboard, nav shows all modules |
| 3 | Click **Inventory** | Catalog renders with ≥ 7 SKUs (PPE-001, MED-001, …) |
| 4 | Click **Requisitions → New** → add a PPE row → Submit | Status chip transitions `draft → pending_approval` |
| 5 | Sign out, sign in as `approver@silveroak.local` | Approvals inbox shows the pending requisition |
| 6 | Approve it | Status transitions to `approved_final` (or continues to finance) |
| 7 | Sign in as `senior@silveroak.local` → **Orders** | Catalog shows reduced member prices (lower than the admin view) |
| 8 | Add `CAN-001`, pick pickup delivery, verify, confirm | Order confirmation page shows a `ref_code` |
| 9 | Reload — the new order is visible under **Orders → My orders** | ✓ |

If any step diverges, capture `docker compose logs backend-api` and
`docker compose logs frontend`; a failure at step 1 or 2 usually means the
stack is still coming up (wait 15 s) or seeds did not run.

---

## Demo credentials

All accounts share the password **`ChangeMeNow!2025`** (Argon2id-hashed).

| Email | Role | Access |
|---|---|---|
| `admin@silveroak.local` | Institution Admin | All routes + admin tools |
| `medical@silveroak.local` | Medical Staff | Requisitions, orders, inventory catalog |
| `approver@silveroak.local` | Department Approver | Approval inbox, requisitions |
| `finance@silveroak.local` | Finance Approver | Approval inbox, analytics (read) |
| `senior@silveroak.local` | Senior | Orders (with member pricing), family portal |
| `family@silveroak.local` | Family Member | Orders, family portal |

---

## Services and ports

### Dev mode

| Service | Host port | Description |
|---|---|---|
| `proxy` | **8000** | Unified entrypoint (UI + `/api/`) — HTTP |
| `frontend` | 8081 | Direct Yew/WASM bundle (nginx) |
| `backend-api` | 8080 | Actix-web REST API, `/api/v1/health` |
| `postgres` | 5433 | `silveroak / silveroak_dev / silveroak` |

### Prod mode (with `docker-compose.prod.yml`)

| Service | Host port | Description |
|---|---|---|
| `proxy` | **80** | HTTP → HTTPS redirect |
| `proxy` | **443** | TLS termination, unified entrypoint |
| `frontend` | 8081 | Direct bundle (internal use / debugging) |
| `postgres` | 5433 | Database (restrict with firewall in real prod) |

---

## Architecture

### Folder structure

```
apps/
  backend-api/      Actix-web 4 REST API, signed-auth middleware, all business logic
  backend-worker/   Tokio background scheduler (6 recurring jobs)
  frontend-yew/     Yew 0.21 WASM SPA, signed-request client, role-aware nav
crates/
  domain/           Pure domain types and invariants
  application/      Use-case ports (traits)
  infrastructure/   sqlx adapters, migrations (embedded), seed, field encryption
  shared/           DTOs shared across HTTP and WASM boundaries
migrations/         0001–0007 additive SQL migrations (embedded at compile time)
proxy/              nginx reverse-proxy configs (nginx.conf, nginx.dev.conf, nginx.prod.conf)
API_tests/          Live bash/curl integration test suites
unit_tests/         Cross-crate unit test scaffold
```

### Tech stack

| Layer | Choice |
|---|---|
| Language | Rust 2021 edition workspace |
| Backend | Actix-web 4, sqlx 0.7, PostgreSQL 16 |
| Frontend | Yew 0.21 (WASM), yew-router 0.18, Trunk |
| Worker | Tokio background scheduler (no external cron) |
| Proxy | nginx 1.27 |
| Containers | Docker Compose multi-stage builds |

---

## Feature modules

| Module | Description |
|---|---|
| **Auth & security** | Argon2 passwords, short-lived API tokens, HMAC-signed requests, replay protection, IP rate limiting, IP blacklist, RBAC + data scopes |
| **Requisitions & approvals** | 7-state lifecycle, table-driven approval routing (dept + finance), transactional finalization with stock deduction, immutable audit trail |
| **Consumable ordering** | Product catalog across 3 stores, member pricing, threshold discounts, coupons (stackable rules), bundle warnings, daily purchase limits, verify-before-confirm checkout |
| **Master data management** | CRUD for students, classes, courses, semesters, departments; multipart CSV/XLSX import with SHA-256 fingerprint dedup; per-row rejection feedback; CSV/XLSX export |
| **Family visibility portal** | Family groups, residents, consent-gated supply/wellness summaries, encrypted wellness notes, personal bests |
| **Analytics** | 6 background worker jobs (daily/weekly/monthly stats, personal bests, consistency checks, anomaly sweep); admin dashboard with live counts + trend tables |
| **Content moderation** | Keyword policies, automatic content flagging (`check_and_flag`), approve/reject/escalate queue, audit log |
| **Anomaly detection** | 3 built-in rules (auth failure burst, privilege escalation, large upload), threshold-based event creation, acknowledge + auto-sweep |

---

## Security architecture

### Authentication flow

1. `POST /api/v1/auth/login` — Argon2 hash verification; 5 failures in 15 min → 15-min lockout.
2. On success: session row created (only SHA-256 of the token stored) + short-lived API token (15-min TTL) + HMAC signing key returned once to the client.

### Signed requests

Every authenticated endpoint is wrapped by `SignedAuth` middleware. Each request must include:

| Header | Value |
|---|---|
| `x-silveroak-token` | API token from login |
| `x-silveroak-timestamp` | Unix seconds or RFC3339; must be within ±60 s |
| `x-silveroak-nonce` | Per-request unique string |
| `x-silveroak-signature` | `hex(HMAC-SHA256(signing_key, canonical))` |

Canonical string (body intentionally excluded — integrity via TLS):

```
METHOD\n
PATH\n
TIMESTAMP\n
NONCE
```

Each signature is stored in `used_signatures` (TTL = 2× window) — replaying an
identical request within the window returns HTTP 400.

### Rate limiting and blacklist

- **120 requests / 60 s** per client IP (in-process sliding window; single-container trade-off documented in constraints).
- `ip_blacklist` table checked before any auth — blocked IPs return 403.

### RBAC and data scopes

Permissions live in `permissions`, mapped to roles via `role_permissions`.
Each request builds a `Principal` from `api_tokens` + role lookups. Scope kinds:
`institution`, `site`, `department`, `family_group`. The `scope:any` permission
(admin only) bypasses all scope checks.

---

## Pricing engine

Rules applied in order per cart line:

1. **Base price** — `product_catalog.base_price_cents`
2. **Member price** — role-specific override; chosen if ≤ base price
3. **Threshold discount** — best `product_threshold_discounts` row for quantity; competes with member price, picks lower
4. **Coupon** — one per order; `WELCOME10` (10%, no-stack) · `LOYAL5` (5%, stacks with member); stacking controlled by flags
5. **Bundle warning** — cross-product quantity warning when threshold met

**Daily purchase limit:** 5 units / SKU / user / calendar day (enforced at set_line, verify, and confirm).

---

## Requisition lifecycle

```
draft ──submit──▶ pending_approval ──approve(final)──▶ approved_final ──▶ issued
  │                     │                                                    ▲
  │                     ├──reject(reason)──▶ rejected (terminal)             │
  │                     ├──send_back(reason)──▶ sent_back ──submit──┐        │
  │                     └──withdraw──▶ withdrawn (terminal)         │        │
  └──withdraw──▶ withdrawn                                          └────────┘
```

All transitions are validated in `requisitions::state::Transition::check` before
any DB write. The finalization (issue) runs inside a single transaction with
`SELECT … FOR UPDATE` on all stock rows; any failure rolls back completely.

---

## Worker job schedule

The background worker (`apps/backend-worker`) runs 6 jobs on fixed intervals:

| Job | Interval | Description |
|---|---|---|
| `DailyStatsJob` | 24 h | Aggregates yesterday's requisition/order/spend stats → `analytics_daily` |
| `WeeklyStatsJob` | 7 d | Sums daily stats + wellness sessions for the week → `analytics_weekly` |
| `MonthlyStatsJob` | 30 d | Monthly rollup from daily stats → `analytics_monthly` |
| `PersonalBestsJob` | 6 h | `DISTINCT ON` best-duration upsert → `wellness_personal_bests` |
| `ConsistencyValidationJob` | 24 h | Orphan checks, stale ID-map entries, unresolved conflicts → `consistency_reports` |
| `AnomalySweepJob` | 1 h | Auto-acknowledges stale low-severity anomaly events; logs unacknowledged count summary |

---

## Field-level encryption

Wellness activity notes are encrypted at rest with **AES-256-GCM**:

- Key: `SILVEROAK_FIELD_KEY` env var (base64-encoded 32 bytes). A loud warning
  is emitted at startup if the variable is absent; a dev fallback key is used.
  **Never use the dev fallback in production.**
- Stored format: `base64(nonce):base64(ciphertext)` in `wellness_activities.notes_encrypted`.
- Family-facing and admin-facing endpoints never return the raw encrypted column.
- Generate a production key: `openssl rand -base64 32`

---

## Consent model

Family groups gate data access via `consent_records`. Three categories:

| Category | Data exposed |
|---|---|
| `supply_usage` | Summarised requisitions per resident |
| `wellness_summary` | Aggregated wellness activities and personal bests |
| `activity_log` | Individual activity log entries |

Accessing a category without an active consent record returns **403 Forbidden**.

---

## Import/export

- **Import**: multipart `POST /api/v1/master-data/{entity_type}/import`.
  SHA-256 fingerprint of the file is stored; re-uploading the same file returns
  **409 Conflict**. Per-row validation feedback is returned in the job result.
  Supported formats: CSV, XLSX (calamine).
- **Export**: `GET /api/v1/master-data/{entity_type}/export?format=csv|xlsx`.
  Returns appropriate `Content-Type` and `Content-Disposition: attachment` headers.

---

## Running tests  (Docker-contained)

`run_tests.sh` is fully Docker-contained.  The host needs **only Docker** —
no local `cargo`, `rustc`, `node`, `npx`, `jq`, `curl`, `openssl`, or Postgres
toolchain.  The `tests` image bakes in cargo, wasm-pack, and the wasm32
target; the `e2e` image bakes in Playwright + headless chromium.  Nothing is
installed at test time.

```bash
# Bring up the stack, build the tests-runner image, execute every suite.
./run_tests.sh

# Skip API integration scripts (unit tests still run inside the container).
SKIP_API_TESTS=1 ./run_tests.sh

# Skip the Playwright E2E suite (backend unit + API tests still run).
SKIP_E2E_TESTS=1 ./run_tests.sh

# Keep the stack up after tests finish (default: containers keep running).
KEEP_STACK=1 ./run_tests.sh

# Force rebuild of the tests-runner image.
REBUILD_TESTS=1 ./run_tests.sh
```

### How the Docker-only pipeline works

| Step | What runs | Where |
|---|---|---|
| 1 | `docker compose up -d` (postgres, backend-api, worker, frontend, proxy) | `docker-compose.yml` + `docker-compose.dev.yml` |
| 2 | Wait for `backend-api` health endpoint | `silveroak_api` container |
| 3 | Seed verification (auto-reseeds if demo users are missing) | `silveroak_api` |
| 4 | `cargo check --workspace --exclude frontend-yew` | **`tests` service** (rust:1.89-slim + cargo + wasm-pack) |
| 5 | `cargo test --workspace --exclude frontend-yew --no-fail-fast` | **`tests` service** |
| 6 | API scripts `auth.sh`, `requisitions.sh`, `orders.sh`, `master_data.sh`, `phase6.sh`, `coverage_extra.sh` | **`tests` service** (curl + jq + openssl + python3) |
| 7 | Playwright E2E specs (`e2e/tests/*.spec.ts`) driving real chromium through the nginx proxy | **`e2e` service** (Playwright v1.47 + chromium + firefox + webkit) |
| 8 | Aggregate results and exit non-zero on any failure | `run_tests.sh` |

The tests-runner image is defined in `Dockerfile.tests`, wired in through
`docker-compose.tests.yml` (profile `tests`), and mounts the repository as
`/workspace` with a named volume for `cargo`'s target directory.

### Manual invocation (without the wrapper)

```bash
docker compose -f docker-compose.yml \
               -f docker-compose.dev.yml \
               -f docker-compose.tests.yml \
               run --rm tests \
               "cargo test --workspace --exclude frontend-yew"
```

### API test suites

| File | Coverage |
|---|---|
| `API_tests/auth.sh` | Login, lockout, signed-request validation, replay protection, scope enforcement, logout |
| `API_tests/requisitions.sh` | Full requisition lifecycle, approval routing, controlled-category path, stock rollback, audit immutability |
| `API_tests/orders.sh` | Cart operations, member/threshold pricing, coupons, bundle warnings, daily limits, verify/confirm, stale-snapshot rejection |
| `API_tests/master_data.sh` | CRUD for all 5 entity types, CSV/XLSX import (fingerprint, partial success), export, FK guards |
| `API_tests/phase6.sh` | Family groups, consent gates, encrypted-note exclusion, analytics, moderation queue + policies, anomaly events + rules |
| `API_tests/coverage_extra.sh` | **Blacklist CRUD, grant-role, auth recovery, requisitions mine/audit, cart coupon-remove, master-data get/update** — closes every endpoint the audit flagged as uncovered (raises HTTP coverage to 100%) |

### Unit tests

#### Backend (`cargo test`, runs in tests-runner container)

Unit tests live alongside backend source files as `#[cfg(test)]` modules.
Covered:

- `security/password.rs` — policy enforcement, Argon2 round-trip, malformed hash
- `security/signing.rs` — canonical string, HMAC sign/verify, timestamp parsing, replay window
- `security/rate_limit.rs` — limit enforcement, multi-IP independence, retry-after
- `requisitions/state.rs` — all status transitions, terminal/editable/withdrawable flags, parse round-trip
- `requisitions/engine.rs` — `Condition::matches` for all condition kinds, edge cases
- `orders/pricing.rs` — threshold formula, member vs threshold selection, coupon stacking, daily limit
- `infrastructure/crypto.rs` — AES-256-GCM round-trip, nonce uniqueness, tamper detection, Unicode

#### Frontend (`wasm-pack test`, runs in tests-runner container)

Dedicated frontend test files live at `apps/frontend-yew/tests/` and are named
with the strict `*.test.rs` / `*.spec.rs` suffix:

| File | Target |
|---|---|
| `apps/frontend-yew/tests/app_render.test.rs` | `App` root + `Card` + `DataTable` components (smoke render) |
| `apps/frontend-yew/tests/router_routes.test.rs` | `router::Route` — nav labels + parameterised route round-trip |
| `apps/frontend-yew/tests/components_utils.test.rs` | `components::utils::fmt_date` / `fmt_datetime` formatters |
| `apps/frontend-yew/tests/auth_state_behavior.test.rs` | `auth::state` reducers — login/logout/reset/role predicates |
| `apps/frontend-yew/tests/state_components.test.rs` | `LoadingState` / `EmptyState` / `ErrorState` render decisions |
| `apps/frontend-yew/tests/auth_client_signing.test.rs` | `ApiClient` signing payload shape + body-hash invariant |

Tests use `wasm-bindgen-test` (dev-dependency in
`apps/frontend-yew/Cargo.toml`) and are gated to `target_arch = "wasm32"` so a
plain host `cargo check` ignores them.  **`wasm-pack` + the `wasm32-unknown-unknown`
target are pre-baked into the tests-runner image** (see `Dockerfile.tests`),
so nothing is installed at test time:

```bash
# Execute the frontend unit tests inside the pre-baked tests-runner container.
docker compose -f docker-compose.yml \
               -f docker-compose.dev.yml \
               -f docker-compose.tests.yml \
               run --rm -w /workspace/apps/frontend-yew tests \
               "wasm-pack test --node"
```

The E2E suite (`e2e/tests/*.spec.ts`) runs the same way — see
[End-to-end tests](#end-to-end-tests-febe) below.

### End-to-end tests (FE↔BE)

Fullstack E2E specs live in `e2e/tests/` and are executed by Playwright
inside the `e2e` container.  Every request drives the real nginx proxy
(`http://proxy:80`) — the proxy forwards `/` to the Yew SPA and `/api/*` to
the Actix backend, so every spec exercises the complete FE↔BE path.

| File | Scenario |
|---|---|
| `e2e/tests/01-auth-dashboard.spec.ts` | Unauthenticated `/` → redirect to `/login`; admin and medical logins land on a non-login page and render role-appropriate nav; wrong password keeps user on `/login`. |
| `e2e/tests/02-requisition-create.spec.ts` | Medical user opens the requisition form, submits a draft, and a signed API probe confirms `/requisitions/mine/list` length grew by ≥1. |
| `e2e/tests/03-orders-cart.spec.ts` | Senior user seeds a cart via signed API, clicks **Verify → Confirm** in the Checkout UI, then the signed API reports the order under `/orders/mine` with status `confirmed` and the same `ref_code` visible through `/orders/{id}`. |

The `e2e` container ships with Playwright + chromium pre-installed
(`Dockerfile.e2e` uses `mcr.microsoft.com/playwright:v1.47.2-jammy`).
Nothing is installed on the host or at test time.

```bash
# Run the whole E2E suite (part of ./run_tests.sh step 7).
docker compose -f docker-compose.yml \
               -f docker-compose.dev.yml \
               -f docker-compose.tests.yml \
               run --rm e2e playwright test

# Run a single spec or grep by title.
docker compose -f docker-compose.yml -f docker-compose.dev.yml -f docker-compose.tests.yml \
    run --rm e2e playwright test 02-requisition-create.spec.ts

# Open the HTML report after a run (report is mounted into ./e2e/playwright-report).
xdg-open e2e/playwright-report/index.html
```

Each spec asserts **both UI and backend outcomes** — never just "the page
loaded".  If a UI selector drifts, the spec's accompanying signed-API probe
fails with a precise message, so regressions surface as actionable errors.

---

## TLS certificates

`docker-compose.prod.yml` mounts a local directory into the proxy container at
`/etc/nginx/certs/`.  The directory is controlled by the `TLS_CERT_DIR` env var
(default: `./proxy/certs`).

Expected files:

| File | Description |
|---|---|
| `fullchain.pem` | Certificate + intermediate chain |
| `privkey.pem` | Private key — chmod 0600 |

**Self-signed cert for smoke-testing** (do not use in real production):

```bash
mkdir -p proxy/certs
openssl req -x509 -newkey rsa:4096 -days 365 -nodes \
  -keyout proxy/certs/privkey.pem \
  -out    proxy/certs/fullchain.pem \
  -subj   "/CN=localhost"
chmod 0600 proxy/certs/privkey.pem
```

**Let's Encrypt / Certbot** (standard production path):

```bash
# After obtaining certs with certbot or equivalent:
export TLS_CERT_DIR=/etc/letsencrypt/live/yourdomain.com
docker compose -f docker-compose.yml -f docker-compose.prod.yml up --build
```

---

## Configuration reference

### Compose file matrix

| File | Purpose |
|---|---|
| `docker-compose.yml` | Base — secure-by-default; usable alone for HTTP-only deploys |
| `docker-compose.dev.yml` | Dev override — seeds on, permissive limits, HTTP proxy |
| `docker-compose.prod.yml` | Prod override — TLS proxy, `COOKIE_SECURE=true`, API port unexposed |

### Key environment variables

| Variable | Base default | Dev override | Prod override | Notes |
|---|---|---|---|---|
| `APP__RUN_SEEDS` | `false` | `true` | — (stays false) | Enable only in dev/demo environments |
| `RATE_LIMIT_PER_MIN` | `120` | `600` | — (stays 120) | Requests per IP per 60 s |
| `DAILY_LIMIT_PER_SKU` | `50` | `500` | — (stays 50) | Per-user daily purchase cap per SKU |
| `COOKIE_SECURE` | `false` | `false` | `true` | Must be true when served over HTTPS |
| `SILVEROAK_FIELD_KEY` | dev placeholder | dev placeholder | **must override** | `openssl rand -base64 32` |
| `TLS_CERT_DIR` | `./proxy/certs` | — | override with real cert dir | Mounted into proxy container |

### Config sanity checklist

Before any non-local deployment, verify:

- [ ] `SILVEROAK_FIELD_KEY` is set to a real 32-byte base64 secret (not the dev placeholder)
- [ ] `APP__RUN_SEEDS` is `false` (default) — never seed a production database
- [ ] `COOKIE_SECURE` is `true` (set automatically by `docker-compose.prod.yml`)
- [ ] TLS cert files exist at `$TLS_CERT_DIR` with correct permissions
- [ ] `RATE_LIMIT_PER_MIN` is ≤ 120 and `DAILY_LIMIT_PER_SKU` is ≤ 50 (base defaults)
- [ ] Postgres port 5433 is firewalled from external traffic

---

## Constraints and notes

- **Rate limiter** is in-process (single API container). Upgrading to a shared
  store (Redis) is a clean swap behind `RateLimiter`.
- **Account recovery** is admin-assisted. `POST /api/v1/auth/recovery/request`
  records a row; no email/SMS is sent (intentional for offline-first deployments).
- **TLS** is handled by `proxy/nginx.prod.conf` (see
  [TLS certificates](#tls-certificates)).  Use `docker-compose.prod.yml` to
  enable it.  The base `docker-compose.yml` and dev override use HTTP for
  zero-setup local development.
- **SILVEROAK_FIELD_KEY** must be set to a real 32-byte secret in production.
  The docker-compose.yml ships a dev placeholder; change it before any
  non-local deployment.
- **Worker jobs** run on timed intervals within the worker process, not via an
  external cron scheduler. The intervals are configured as constants in
  `apps/backend-worker/src/jobs/`.

---

## Security audit follow-up

### TASK-17 — Anomaly/moderation trigger wiring (High severity)

**Finding**: `record_auth_failure`, `record_permission_denial`, `record_large_upload`
(anomaly service) and `check_and_flag` (moderation service) were defined but not
invoked from production code paths.

**Fix** (non-breaking, fail-safe):

| Trigger function | Call site | File |
|---|---|---|
| `record_auth_failure` | Failed login branch | `apps/backend-api/src/routes/auth.rs` |
| `record_permission_denial` | Post-403 response hook in `SignedAuth` middleware | `apps/backend-api/src/security/middleware.rs` |
| `check_and_flag` | After requisition commit in `create_draft` | `apps/backend-api/src/requisitions/service.rs` |
| `record_large_upload` | After multipart read in `run_import` | `apps/backend-api/src/master_data/import.rs` |

**Design decisions**:

- All recording calls are **fire-and-forget**: errors are logged at `WARN` level
  via `tracing::warn!` but never propagated to the caller.  A transient DB issue
  in the side channel must not degrade the primary user operation.
- `record_permission_denial` is wired in the `SignedAuth` middleware so it covers
  **every authenticated route** without touching individual handlers.  When a
  handler returns HTTP 403, the middleware records `("forbidden_request", path)`.
- `check_and_flag` fires **after** `tx.commit()` so a moderation DB error cannot
  roll back an accepted requisition.
- `record_large_upload` fires immediately after the multipart read, before MIME
  validation, so uploads that are later rejected still produce an anomaly signal.

**Tests added**: inline `#[cfg(test)]` tests in each modified module verify
threshold conditions, status-code constants, and content-type labels used at the
call sites.  Full live-DB integration tests belong in `apps/backend-api/tests/`
and require a running Postgres instance (`API_tests/README.md`).

---

### TASK-17 — Backend security hardening v2: M-03 and M-04 fully operationalized

#### M-03 — Distributed rate limiting (now Fixed)

**Previous state**: `RateLimiter` supported two backends but defaulted to
`memory`, leaving multi-replica deployments unprotected.

**Fix**: `docker-compose.yml` (the base / production-capable profile) now sets
`APP__RATE_LIMIT_BACKEND=postgres` by default.  `docker-compose.dev.yml`
overrides this to `memory` for local development.

| Backend | Config | Where set | Behavior |
|---|---|---|---|
| `memory` | `APP__RATE_LIMIT_BACKEND=memory` | docker-compose.dev.yml | Process-local. Zero extra DB load. Correct for single-container dev. |
| `postgres` | `APP__RATE_LIMIT_BACKEND=postgres` | docker-compose.yml (default) | Shared window via `rate_limit_log` (migration 0008). Globally correct under horizontal scaling. ~1 extra DB round-trip per request. |

**Postgres failure policy (documented)**: If the DB is unreachable during a rate-limit
check, the request is **allowed through** (fail-open) and a structured `WARN` log
is emitted with `rate_limit_db_error=true`.  This prevents a transient DB issue
from causing a self-inflicted denial-of-service.  Sustained DB failures appear in
logs and can trigger alerts; operators should monitor `rate_limit_db_error` tag
counts.

**Retry-after is now bounded**: `[1, window_secs]` seconds — deterministic and
safe for client retry loops.

**Tests added**:
- `allows_up_to_limit_then_denies` / `multiple_ips_are_independent` (async)
- `retry_after_is_at_least_one` / `retry_after_is_at_most_window` (bounds)
- `from_config_*` tests (backend selection, fallback, case-insensitivity)
- `outcome_type` tests (`Allow` vs `Deny` comparison, field preservation)

#### M-04 — Three-mode strict signing (now Fixed)

**Previous state**: Binary `APP__STRICT_SIGNING=true/false` — no transition
path; any cut-over from false→true was a hard break for existing clients.

**Fix**: Replaced the boolean with a three-mode `SigningMode` enum and a
corresponding `APP__SIGNING_MODE` config variable:

| Mode | Config value | Accepts legacy? | WARN logged? | Notes |
|---|---|---|---|---|
| Compat | `APP__SIGNING_MODE=compat` | yes | no | Default for dev. 4-field canonical only. |
| Dual | `APP__SIGNING_MODE=dual` | yes (with WARN) | yes, per request | Production-capable default. Accepts strict and compat; WARN on compat fallback. |
| Strict | `APP__SIGNING_MODE=strict` | no | n/a | Final production target. Rejects missing body-hash with 401. |

**Deploy profile defaults**:

| Compose file | Rate limiter | Signing mode |
|---|---|---|
| `docker-compose.yml` (base) | `postgres` | `dual` |
| `docker-compose.dev.yml` (overlay) | `memory` | `compat` |
| `docker-compose.prod.yml` (overlay) | `postgres` | `strict` |

**Frontend updated**: The Yew `ApiClient` now always sends `x-silveroak-body-hash`
(SHA-256 of the request body, or SHA-256 of empty bytes for GET/DELETE).  The
server ignores this header in `compat` mode and uses it in `dual`/`strict` mode.
No change to the canonical string the client signs (4-field), so `compat` and
`dual` remain backward-compatible.

**Migration cutover checklist**:

```text
1. Deploy with APP__SIGNING_MODE=dual (already set in docker-compose.yml).
2. Deploy updated Yew frontend (now sends x-silveroak-body-hash).
3. Monitor WARN logs for "dual-mode compat fallback" entries.
4. Once WARN rate is zero for ≥ 24 h, all in-flight clients are strict-capable.
5. Set APP__SIGNING_MODE=strict in docker-compose.prod.yml and redeploy.

Rollback at any step: set APP__SIGNING_MODE=dual (env change only, no build).
```

**Multipart uploads**: `post_multipart` sends SHA-256 of empty bytes as the body
hash (limitation: `FormData` cannot be streamed through SHA-256 in the browser
without a `ReadableStream` transform).  In `strict` mode the server accepts this
convention for multipart content.  Body integrity for uploads is provided by TLS.

**Tests added** (all in `security/middleware.rs`):
- `signed_auth_defaults_to_compat_mode`
- `strict_mode_tampered_query_fails`
- `legacy_compat_request_fails_strict_canonical` (mode isolation)
- `dual_mode_with_body_hash_uses_strict_canonical`
- `signing_modes_are_distinct_values`
- `strict_mode_signature_verifies_with_correct_body_hash`

Additional tests in `security/signing_mode.rs`:
- `from_config_parses_all_variants`
- `from_config_is_case_insensitive`
- `from_config_unknown_falls_back_to_compat`
- `labels_are_distinct`

#### L-01 — `.env.example` safe defaults (updated)

`.env.example` now documents the full three-mode signing system and both
rate-limit backends with clear `[DEV ONLY]` annotations, safe production
defaults, and inline descriptions of each option.
