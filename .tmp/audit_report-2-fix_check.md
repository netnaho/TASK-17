# SilverOak TASK-17 — Issue Reinspection Status (Round 2, Static)

- Date: 2026-04-10
- Baseline issue list: `.tmp/delivery_acceptance_architecture_audit_static.md` (H-01, M-01, M-02, M-03, M-04, L-01)
- Validation mode: **Static-only** (no runtime execution)

---

## Summary

- **Fixed:** 6
- **Partially Fixed:** 0
- **Not Fixed:** 0

---

## Reinspection Results

| ID   | Prior Issue                                                   | Current Status | Evidence (current code)                                                                                                                                                                                                                                                                                                                                                                           | Conclusion                                                                                                                  |
| ---- | ------------------------------------------------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| H-01 | Anomaly/moderation trigger logic not wired into runtime paths | **Fixed**      | Runtime invocations now present: `repo/apps/backend-api/src/routes/auth.rs:119` (`record_auth_failure`), `repo/apps/backend-api/src/security/middleware.rs:338` (`record_permission_denial`), `repo/apps/backend-api/src/master_data/import.rs:146` (`record_large_upload`), `repo/apps/backend-api/src/requisitions/service.rs:255` (`check_and_flag`)                                           | Previously inert detection hooks are now wired into real request flows.                                                     |
| M-01 | Frontend panic risk from fixed timestamp slicing              | **Fixed**      | No `[..N]` slicing remains in frontend page files; safe format helpers in use: `repo/apps/frontend-yew/src/pages/moderation.rs:322,386`, `repo/apps/frontend-yew/src/pages/orders.rs:75`, `repo/apps/frontend-yew/src/pages/order_detail.rs:87`, `repo/apps/frontend-yew/src/pages/security_events.rs:221`                                                                                        | Timestamp rendering path is now guarded/format-based rather than brittle slicing.                                           |
| M-02 | Moderation status filter stale-state race                     | **Fixed**      | Queue reload keyed via effect: `repo/apps/frontend-yew/src/pages/moderation.rs:137`; change handler sets state only: `repo/apps/frontend-yew/src/pages/moderation.rs:245`                                                                                                                                                                                                                         | Fetch now occurs after state update/re-render, removing stale closure fetch.                                                |
| M-03 | Rate limiter process-local by design                          | **Fixed**      | Distributed backend exists and is configured as base default: `repo/apps/backend-api/src/security/rate_limit.rs:1-19,98-106,121-139,182`, `repo/docker-compose.yml` (`APP__RATE_LIMIT_BACKEND=${APP__RATE_LIMIT_BACKEND:-postgres}`), prod restated: `repo/docker-compose.prod.yml` (`APP__RATE_LIMIT_BACKEND: "postgres"`)                                                                       | Horizontal-scale correctness path is implemented and set as production-default; dev memory mode is an intentional override. |
| M-04 | Signed surface excluded query/body in default posture         | **Fixed**      | Full rollout modes implemented (`compat/dual/strict`): `repo/apps/backend-api/src/security/signing_mode.rs:9-10,53`; strict canonical support: `repo/apps/backend-api/src/security/signing.rs:67`; middleware strict/dual handling: `repo/apps/backend-api/src/security/middleware.rs:253,272,297`; base default `dual`: `repo/docker-compose.yml`; prod `strict`: `repo/docker-compose.prod.yml` | Query/body signing is operationalized with non-breaking migration path and production strict mode target.                   |
| L-01 | `.env.example` seeds-enabled default risk                     | **Fixed**      | `.env.example` now sets `APP__RUN_SEEDS=false`; base compose defaults seeds off: `repo/docker-compose.yml` (`APP__RUN_SEEDS=${APP__RUN_SEEDS:-false}`)                                                                                                                                                                                                                                            | Accidental seeding-by-default risk is closed.                                                                               |

---

## Final Reinspection Verdict

All issues from the prior inspection list are now **Fixed** under static review.

The remaining caveat is operational (not code-completeness): strict signing and distributed rate limiting should still be validated in a live environment for client compatibility and behavior under load.

---

## Static Boundary Note

This report is source/config validation only. Runtime assertions (traffic behavior, rollout safety under real clients, performance under concurrency) require live integration testing.
