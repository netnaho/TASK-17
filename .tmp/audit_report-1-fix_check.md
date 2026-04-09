# SilverOak TASK-17 — Issue Revalidation Report (Round 2, Static)

Date: 2026-04-09  
Mode: Static source/config review only (no runtime execution)

This round re-checks the same previously reported issues after the latest repo updates (including new `docker-compose.dev.yml`, `docker-compose.prod.yml`, `proxy/nginx.dev.conf`, and `proxy/nginx.prod.conf`).

---

## Summary

- **Fixed:** 8
- **Partially Fixed:** 0
- **Not Fixed:** 0

---

## Detailed Revalidation

| #   | Previously Reported Issue                                        | Current Status | Evidence (current code)                                                                                                                                                                                                                                                                    | Notes                                                                           |
| --- | ---------------------------------------------------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| 1   | Signature canonicalization mismatch (frontend vs backend)        | **Fixed**      | Frontend strips query before signing: `apps/frontend-yew/src/auth/client.rs:43-44`, `:61-62`. Backend verifies normalized canonical path and keeps transition fallback for legacy query-including signatures: `apps/backend-api/src/security/middleware.rs:209-211`, `:217`, `:228-229`.   | Contract is now aligned and backward-compatible during transition.              |
| 2   | Requisition approval action scope gap (approve/reject/send-back) | **Fixed**      | Shared dept guard added: `apps/backend-api/src/requisitions/service.rs:157`. Applied in `approve`: `:354`; `reject`: `:471`; `send_back`: `:508`.                                                                                                                                          | Mutation paths now enforce object-level department scope.                       |
| 3   | Issue-record endpoint auth too weak                              | **Fixed**      | Route delegates to authorization-aware service call: `apps/backend-api/src/routes/requisitions.rs:179`. Service checks same read policy via `can_read_requisition`: `apps/backend-api/src/requisitions/service.rs:665`, `:679`.                                                            | No longer principal-only read of issue records.                                 |
| 4   | Analytics institution scope gap                                  | **Fixed**      | Institution-scope guard exists: `apps/backend-api/src/analytics/service.rs:70-71`. Enforced in analytics entry points (examples): `:80-81`, `:110-111`, `:140-141`, `:175-176`.                                                                                                            | Caller-supplied `institution_id` is now scope-bound.                            |
| 5   | Seed SQL vs migration schema mismatch                            | **Fixed**      | Seed now uses `event_kind/comment` and line totals with `quantity`: `crates/infrastructure/src/seed.rs:566`, `:574`, `:600`, `:608`, `:677`. Migration contract remains `quantity`, `event_kind`, `comment`: `migrations/0003_requisitions.sql:102`, `:140`, `:141`.                       | Requisition seed statements align with migration schema.                        |
| 6   | HTTP-only proxy / no TLS termination in provided config          | **Fixed**      | Production proxy config now has HTTP→HTTPS redirect and TLS termination with cert/key and security headers: `proxy/nginx.prod.conf:22`, `:24`, `:33`, `:36-37`, `:55`, `:57`, `:59`. Prod compose mounts `nginx.prod.conf` and certs, and exposes 80/443: `docker-compose.prod.yml:37-40`. | Dev remains intentionally HTTP via `nginx.dev.conf`; prod path is TLS-hardened. |
| 7   | Security-default relaxation in compose (seeds/rate limits)       | **Fixed**      | Base compose now secure-by-default: `APP__RUN_SEEDS=${...:-false}` at `docker-compose.yml:34`, `RATE_LIMIT_PER_MIN=${...:-120}` at `:42`, `DAILY_LIMIT_PER_SKU=${...:-50}` at `:43`. Dev-only permissive overrides are isolated in `docker-compose.dev.yml:18-20`.                         | Hardening gap closed via base defaults + explicit dev override split.           |
| 8   | README drift vs config/schema                                    | **Fixed**      | README now documents dev/prod compose flows and TLS setup: `README.md:10`, `:13`, `:26`, `:31`, `:284`, `:325-326`, `:332-334`. Port/migration references are aligned with current setup (e.g., postgres 5433 and 0001–0007 in current README).                                            | Documentation now reflects actual deployment/config model.                      |

---

## Final Revalidation Verdict

All previously reported issues are now **resolved in static review**.

- Security/authz/data-isolation issues: **resolved**.
- Deployment hardening/config hygiene issues: **resolved** via dev/prod split and TLS production config.

### Static boundary

This report does **not** assert runtime correctness of TLS cert mounting, redirect behavior, or compose merge behavior in a live environment, since no runtime commands were executed.
