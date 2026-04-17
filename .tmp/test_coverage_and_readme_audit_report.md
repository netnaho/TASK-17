# Test Coverage Audit# Test Coverage Audit

## Scope, Method, and Project Type## Scope, Method, and Project Type

- **Audit mode:** static inspection only (no execution of code/tests/scripts/containers).- **Audit mode:** static inspection only (no test execution, no build, no containers run).

- **Scope inspected (targeted):**- **Inspected areas only:** backend route declarations, API test scripts, frontend/backend unit test files, `run_tests.sh`, and `repo/README.md`.
  - Backend route declarations (`repo/apps/backend-api/src/routes/*.rs`)- **README-declared project type:** `fullstack` (`repo/README.md:3`).

  - API test suites (`repo/API_tests/*.sh`)- **Resolved API base prefix:** `/api/v1` from router scope (`repo/apps/backend-api/src/routes/mod.rs:18`).

  - Frontend unit tests (`repo/apps/frontend-yew/tests/*`)

  - Fullstack E2E tests (`repo/e2e/tests/*`, runner wiring)---

  - Test orchestrator (`repo/run_tests.sh`)

  - README (`repo/README.md`)## Backend Endpoint Inventory (strict `METHOD + fully resolved PATH`)

- **Project type declaration:** `fullstack` found at top of README (`repo/README.md:3`).

- **Resolved API prefix/version:** `/api/v1` (`repo/apps/backend-api/src/routes/mod.rs:18`).> Resolved as `/api/v1` + route attributes from `repo/apps/backend-api/src/routes/*.rs`.

---1. `GET /api/v1/health`

2. `POST /api/v1/auth/login`

## Backend Endpoint Inventory3. `POST /api/v1/auth/logout`

4. `GET /api/v1/auth/session`

> Endpoint = unique `METHOD + fully resolved PATH` under `/api/v1`.5. `POST /api/v1/auth/recovery/request`

6. `GET /api/v1/admin/blacklist`

1. `GET /api/v1/health`7. `POST /api/v1/admin/blacklist`

1. `POST /api/v1/auth/login`8. `DELETE /api/v1/admin/blacklist/{ip}`

1. `POST /api/v1/auth/logout`9. `POST /api/v1/admin/users/grant-role`

1. `GET /api/v1/auth/session`10. `GET /api/v1/departments/{dept_id}/residents`

1. `POST /api/v1/auth/recovery/request`11. `GET /api/v1/me`

1. `GET /api/v1/admin/blacklist`12. `GET /api/v1/inventory/catalog`

1. `POST /api/v1/admin/blacklist`13. `POST /api/v1/requisitions`

1. `DELETE /api/v1/admin/blacklist/{ip}`14. `POST /api/v1/requisitions/{id}/submit`

1. `POST /api/v1/admin/users/grant-role`15. `POST /api/v1/requisitions/{id}/withdraw`

1. `GET /api/v1/departments/{dept_id}/residents`16. `POST /api/v1/requisitions/{id}/approve`

1. `GET /api/v1/me`17. `POST /api/v1/requisitions/{id}/reject`

1. `GET /api/v1/inventory/catalog`18. `POST /api/v1/requisitions/{id}/send-back`

1. `POST /api/v1/requisitions`19. `GET /api/v1/requisitions/{id}`

1. `POST /api/v1/requisitions/{id}/submit`20. `GET /api/v1/requisitions/mine/list`

1. `POST /api/v1/requisitions/{id}/withdraw`21. `GET /api/v1/approvals/inbox`

1. `POST /api/v1/requisitions/{id}/approve`22. `GET /api/v1/requisitions/{id}/audit`

1. `POST /api/v1/requisitions/{id}/reject`23. `GET /api/v1/issue-records/{req_id}`

1. `POST /api/v1/requisitions/{id}/send-back`24. `GET /api/v1/orders/products`

1. `GET /api/v1/requisitions/{id}`25. `GET /api/v1/orders/delivery-methods`

1. `GET /api/v1/requisitions/mine/list`26. `GET /api/v1/orders/cart`

1. `GET /api/v1/approvals/inbox`27. `POST /api/v1/orders/cart/lines`

1. `GET /api/v1/requisitions/{id}/audit`28. `POST /api/v1/orders/cart/coupon`

1. `GET /api/v1/issue-records/{req_id}`29. `POST /api/v1/orders/cart/coupon-remove`

1. `GET /api/v1/orders/products`30. `POST /api/v1/orders/cart/delivery`

1. `GET /api/v1/orders/delivery-methods`31. `POST /api/v1/orders/verify`

1. `GET /api/v1/orders/cart`32. `POST /api/v1/orders/confirm`

1. `POST /api/v1/orders/cart/lines`33. `GET /api/v1/orders/mine`

1. `POST /api/v1/orders/cart/coupon`34. `GET /api/v1/orders/{id}`

1. `POST /api/v1/orders/cart/coupon-remove`35. `GET /api/v1/master-data/institutions`

1. `POST /api/v1/orders/cart/delivery`36. `GET /api/v1/master-data/institutions/{iid}/semesters`

1. `POST /api/v1/orders/verify`37. `GET /api/v1/master-data/semesters/{id}`

1. `POST /api/v1/orders/confirm`38. `POST /api/v1/master-data/institutions/{iid}/semesters`

1. `GET /api/v1/orders/mine`39. `POST /api/v1/master-data/semesters/{id}/update`

1. `GET /api/v1/orders/{id}`40. `POST /api/v1/master-data/semesters/{id}/delete`

1. `GET /api/v1/master-data/institutions`41. `GET /api/v1/master-data/institutions/{iid}/classes`

1. `GET /api/v1/master-data/institutions/{iid}/semesters`42. `GET /api/v1/master-data/classes/{id}`

1. `GET /api/v1/master-data/semesters/{id}`43. `POST /api/v1/master-data/institutions/{iid}/classes`

1. `POST /api/v1/master-data/institutions/{iid}/semesters`44. `POST /api/v1/master-data/classes/{id}/update`

1. `POST /api/v1/master-data/semesters/{id}/update`45. `POST /api/v1/master-data/classes/{id}/delete`

1. `POST /api/v1/master-data/semesters/{id}/delete`46. `GET /api/v1/master-data/institutions/{iid}/courses`

1. `GET /api/v1/master-data/institutions/{iid}/classes`47. `GET /api/v1/master-data/courses/{id}`

1. `GET /api/v1/master-data/classes/{id}`48. `POST /api/v1/master-data/institutions/{iid}/courses`

1. `POST /api/v1/master-data/institutions/{iid}/classes`49. `POST /api/v1/master-data/courses/{id}/update`

1. `POST /api/v1/master-data/classes/{id}/update`50. `POST /api/v1/master-data/courses/{id}/delete`

1. `POST /api/v1/master-data/classes/{id}/delete`51. `GET /api/v1/master-data/institutions/{iid}/students`

1. `GET /api/v1/master-data/institutions/{iid}/courses`52. `GET /api/v1/master-data/students/{id}`

1. `GET /api/v1/master-data/courses/{id}`53. `POST /api/v1/master-data/institutions/{iid}/students`

1. `POST /api/v1/master-data/institutions/{iid}/courses`54. `POST /api/v1/master-data/students/{id}/update`

1. `POST /api/v1/master-data/courses/{id}/update`55. `POST /api/v1/master-data/students/{id}/delete`

1. `POST /api/v1/master-data/courses/{id}/delete`56. `GET /api/v1/master-data/institutions/{iid}/departments`

1. `GET /api/v1/master-data/institutions/{iid}/students`57. `GET /api/v1/master-data/departments/{id}`

1. `GET /api/v1/master-data/students/{id}`58. `POST /api/v1/master-data/institutions/{iid}/departments`

1. `POST /api/v1/master-data/institutions/{iid}/students`59. `POST /api/v1/master-data/departments/{id}/update`

1. `POST /api/v1/master-data/students/{id}/update`60. `POST /api/v1/master-data/departments/{id}/delete`

1. `POST /api/v1/master-data/students/{id}/delete`61. `POST /api/v1/master-data/institutions/{iid}/import/{entity_type}`

1. `GET /api/v1/master-data/institutions/{iid}/departments`62. `GET /api/v1/master-data/import-jobs/{job_id}`

1. `GET /api/v1/master-data/departments/{id}`63. `GET /api/v1/master-data/institutions/{iid}/export/{entity_type}`

1. `POST /api/v1/master-data/institutions/{iid}/departments`64. `GET /api/v1/family/groups`

1. `POST /api/v1/master-data/departments/{id}/update`65. `POST /api/v1/family/groups`

1. `POST /api/v1/master-data/departments/{id}/delete`66. `POST /api/v1/family/groups/{id}/members`

1. `POST /api/v1/master-data/institutions/{iid}/import/{entity_type}`67. `POST /api/v1/family/groups/{id}/residents`

1. `GET /api/v1/master-data/import-jobs/{job_id}`68. `POST /api/v1/family/groups/{id}/consent`

1. `GET /api/v1/master-data/institutions/{iid}/export/{entity_type}`69. `GET /api/v1/family/groups/{id}/supply-summary`

1. `GET /api/v1/family/groups`70. `GET /api/v1/family/groups/{id}/wellness-summary`

1. `POST /api/v1/family/groups`71. `GET /api/v1/family/groups/{id}`

1. `POST /api/v1/family/groups/{id}/members`72. `GET /api/v1/family/groups/{id}/activity-log`

1. `POST /api/v1/family/groups/{id}/residents`73. `POST /api/v1/family/wellness`

1. `POST /api/v1/family/groups/{id}/consent`74. `GET /api/v1/analytics/dashboard`

1. `GET /api/v1/family/groups/{id}/supply-summary`75. `GET /api/v1/analytics/daily`

1. `GET /api/v1/family/groups/{id}/wellness-summary`76. `GET /api/v1/analytics/weekly`

1. `GET /api/v1/family/groups/{id}`77. `GET /api/v1/analytics/monthly`

1. `GET /api/v1/family/groups/{id}/activity-log`78. `GET /api/v1/moderation/queue`

1. `POST /api/v1/family/wellness`79. `POST /api/v1/moderation/queue/{id}/review`

1. `GET /api/v1/analytics/dashboard`80. `GET /api/v1/moderation/policies`

1. `GET /api/v1/analytics/daily`81. `POST /api/v1/moderation/policies`

1. `GET /api/v1/analytics/weekly`82. `POST /api/v1/moderation/policies/{id}/delete`

1. `GET /api/v1/analytics/monthly`83. `GET /api/v1/anomaly/events`

1. `GET /api/v1/moderation/queue`84. `POST /api/v1/anomaly/events/{id}/acknowledge`

1. `POST /api/v1/moderation/queue/{id}/review`85. `GET /api/v1/anomaly/rules`

1. `GET /api/v1/moderation/policies`

1. `POST /api/v1/moderation/policies`---

1. `POST /api/v1/moderation/policies/{id}/delete`

1. `GET /api/v1/anomaly/events`## API Test Classification

1. `POST /api/v1/anomaly/events/{id}/acknowledge`

1. `GET /api/v1/anomaly/rules`### 1) True No-Mock HTTP tests

Evidence: route attributes count is **85** (`grep` over `repo/apps/backend-api/src/routes/**/*.rs`) and router prefix is `/api/v1` (`routes/mod.rs:18`).- `repo/API_tests/auth.sh`

- `repo/API_tests/requisitions.sh`

---- `repo/API_tests/orders.sh`

- `repo/API_tests/master_data.sh`

## API Test Classification- `repo/API_tests/phase6.sh`

- `repo/API_tests/coverage_extra.sh`

### 1) True No-Mock HTTP

**Evidence:** each script issues real `curl` calls to live HTTP endpoints and signed headers (`auth.sh:52`, `requisitions.sh:64`, `orders.sh:70`, `master_data.sh:71`, `phase6.sh:67`, `coverage_extra.sh:64`) and `run_tests.sh` orchestrates these via Dockerized stack (`repo/run_tests.sh:33-42`, `204-227`).

- `repo/API_tests/auth.sh`

- `repo/API_tests/requisitions.sh`### 2) HTTP with mocking

- `repo/API_tests/orders.sh`

- `repo/API_tests/master_data.sh`- **None found** (static scan).

- `repo/API_tests/phase6.sh`

- `repo/API_tests/coverage_extra.sh`### 3) Non-HTTP tests (unit/integration without HTTP)

Evidence:- Frontend wasm unit tests:

- Real HTTP transport via `curl` helpers and signed headers in each suite. - `repo/apps/frontend-yew/tests/app_render.test.rs`

- Real app wiring via Docker stack and runner orchestration (`repo/run_tests.sh`, steps for stack boot + API suites). - `repo/apps/frontend-yew/tests/components_utils.test.rs`
  - `repo/apps/frontend-yew/tests/router_routes.test.rs`

### 2) HTTP with mocking- Cross-crate Rust unit tests:

- `repo/unit_tests/tests/cross_crate.rs`

- **None found** by static inspection.- Many backend source-level unit tests (`#[cfg(test)]`) in modules such as:
  - `security/signing.rs`, `security/rate_limit.rs`, `security/password.rs`, `orders/pricing.rs`, `requisitions/engine.rs`, `requisitions/state.rs`, `family/service.rs`, `analytics/service.rs`, `anomaly/service.rs`, `moderation/service.rs`.

### 3) Non-HTTP (unit/integration without HTTP)

---

- Backend Rust unit tests in source modules (`#[cfg(test)]` across security, orders, requisitions, family, analytics, anomaly, moderation, master_data import, etc.).

- Cross-crate tests: `repo/unit_tests/tests/cross_crate.rs`.## Mock Detection (strict)

- Frontend wasm unit tests in `repo/apps/frontend-yew/tests/*.test.rs`.

- Playwright E2E is HTTP-driven browser testing, not unit-only.Searched patterns: `jest.mock`, `vi.mock`, `sinon.stub`, `mockall`, `stub`, `mock`.

---- **No mocking/stubbing evidence found in test code paths**.

- Only textual claims (not mocks) found:

## Mock Detection (strict) - `repo/README.md:97`

- `repo/API_tests/coverage_extra.sh:7`

Searched patterns in code/tests:

- `jest.mock`Classification outcome: API suites remain **True No-Mock HTTP** by static evidence.

- `vi.mock`

- `sinon.stub`---

- `mockall`

- `mockito`## API Test Mapping Table (per endpoint)

- `spyOn(`

| Endpoint | Covered | Test type | Test files | Evidence |

Result:| ---------------------------------------------------------------- | ------- | ----------------- | --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |

- **No matches found** in `repo/**/*.{rs,ts,js,sh}`.| GET /api/v1/health | yes | true no-mock HTTP | auth.sh | route `routes/health.rs:4`; call `API_tests/auth.sh:74` |

| POST /api/v1/auth/login | yes | true no-mock HTTP | auth.sh, requisitions.sh, orders.sh, master_data.sh, phase6.sh, coverage_extra.sh | route `routes/auth.rs:48`; calls e.g. `API_tests/auth.sh:62` |

Verdict:| POST /api/v1/auth/logout | yes | true no-mock HTTP | auth.sh | route `routes/auth.rs:172`; call `API_tests/auth.sh:166` |

- No static evidence of transport/controller/service mocking in API tests.| GET /api/v1/auth/session | yes | true no-mock HTTP | auth.sh | route `routes/auth.rs:197`; call `API_tests/auth.sh:167` |

| POST /api/v1/auth/recovery/request | yes | true no-mock HTTP | coverage_extra.sh | route `routes/auth.rs:232`; calls `API_tests/coverage_extra.sh` (section 3) |

---| GET /api/v1/admin/blacklist | yes | true no-mock HTTP | coverage_extra.sh | route `routes/admin.rs:16`; call section 1 |

| POST /api/v1/admin/blacklist | yes | true no-mock HTTP | coverage_extra.sh | route `routes/admin.rs:27`; call section 1 |

## API Test Mapping Table| DELETE /api/v1/admin/blacklist/{ip} | yes | true no-mock HTTP | coverage_extra.sh | route `routes/admin.rs:39`; call section 1 |

| POST /api/v1/admin/users/grant-role | yes | true no-mock HTTP | coverage_extra.sh | route `routes/admin.rs:56`; call section 2 |

> Per-endpoint coverage mapping (strict METHOD+PATH). All coverage evidence is static (callsites in API scripts + route declaration).| GET /api/v1/departments/{dept_id}/residents | yes | true no-mock HTTP | auth.sh | route `routes/admin.rs:134`; calls `API_tests/auth.sh:143,161` |

| GET /api/v1/me | yes | true no-mock HTTP | auth.sh, requisitions.sh, coverage_extra.sh | route `routes/admin.rs:148`; calls `auth.sh:91`, `requisitions.sh:81` |

| Endpoint | Covered | Test type | Test files | Evidence || GET /api/v1/inventory/catalog | yes | true no-mock HTTP | requisitions.sh, coverage_extra.sh | route `routes/requisitions.rs:32`; calls `requisitions.sh:88`, `coverage_extra.sh` section 4 |

|---|---|---|---|---|| POST /api/v1/requisitions | yes | true no-mock HTTP | requisitions.sh, coverage_extra.sh | route `routes/requisitions.rs:66`; calls `requisitions.sh:97`, `coverage_extra.sh` section 4 |

| GET /api/v1/health | yes | true no-mock HTTP | auth.sh | `routes/health.rs:4`; `API_tests/auth.sh` health check block || POST /api/v1/requisitions/{id}/submit | yes | true no-mock HTTP | requisitions.sh | route `routes/requisitions.rs:77`; call `requisitions.sh:112` |

| POST /api/v1/auth/login | yes | true no-mock HTTP | auth.sh, requisitions.sh, orders.sh, master_data.sh, phase6.sh, coverage_extra.sh | `routes/auth.rs:48`; login helper/calls across suites || POST /api/v1/requisitions/{id}/withdraw | yes | true no-mock HTTP | requisitions.sh | route `routes/requisitions.rs:88`; calls `requisitions.sh:153,156` |

| POST /api/v1/auth/logout | yes | true no-mock HTTP | auth.sh | `routes/auth.rs:172`; logout block || POST /api/v1/requisitions/{id}/approve | yes | true no-mock HTTP | requisitions.sh | route `routes/requisitions.rs:99`; calls `requisitions.sh:161,173,189,206` |

| GET /api/v1/auth/session | yes | true no-mock HTTP | auth.sh | `routes/auth.rs:197`; session-invalidated check || POST /api/v1/requisitions/{id}/reject | yes | true no-mock HTTP | requisitions.sh | route `routes/requisitions.rs:111`; call `requisitions.sh:138` |

| POST /api/v1/auth/recovery/request | yes | true no-mock HTTP | coverage_extra.sh | `routes/auth.rs:232`; coverage-extra recovery section || POST /api/v1/requisitions/{id}/send-back | yes | true no-mock HTTP | requisitions.sh | route `routes/requisitions.rs:123`; call `requisitions.sh:143` |

| GET /api/v1/admin/blacklist | yes | true no-mock HTTP | coverage_extra.sh | `routes/admin.rs:16`; coverage-extra admin blacklist section || GET /api/v1/requisitions/{id} | yes | true no-mock HTTP | requisitions.sh | route `routes/requisitions.rs:135`; calls `requisitions.sh:226,234` |

| POST /api/v1/admin/blacklist | yes | true no-mock HTTP | coverage_extra.sh | `routes/admin.rs:27`; coverage-extra admin blacklist section || GET /api/v1/requisitions/mine/list | yes | true no-mock HTTP | coverage_extra.sh | route `routes/requisitions.rs:146`; call section 4 |

| DELETE /api/v1/admin/blacklist/{ip} | yes | true no-mock HTTP | coverage_extra.sh | `routes/admin.rs:39`; coverage-extra admin blacklist section || GET /api/v1/approvals/inbox | yes | true no-mock HTTP | requisitions.sh | route `routes/requisitions.rs:152`; call `requisitions.sh:131` |

| POST /api/v1/admin/users/grant-role | yes | true no-mock HTTP | coverage_extra.sh | `routes/admin.rs:56`; coverage-extra grant-role section || GET /api/v1/requisitions/{id}/audit | yes | true no-mock HTTP | coverage_extra.sh | route `routes/requisitions.rs:158`; call section 4 |

| GET /api/v1/departments/{dept_id}/residents | yes | true no-mock HTTP | auth.sh | `routes/admin.rs:134`; scope isolation checks || GET /api/v1/issue-records/{req_id} | yes | true no-mock HTTP | requisitions.sh | route `routes/requisitions.rs:169`; call `requisitions.sh:179` |

| GET /api/v1/me | yes | true no-mock HTTP | auth.sh, requisitions.sh, coverage_extra.sh | `routes/admin.rs:148`; signed `/me` calls || GET /api/v1/orders/products | yes | true no-mock HTTP | orders.sh | route `routes/orders.rs:9`; call `orders.sh:92` |

| GET /api/v1/inventory/catalog | yes | true no-mock HTTP | requisitions.sh, coverage_extra.sh | `routes/requisitions.rs:32`; catalog calls || GET /api/v1/orders/delivery-methods | yes | true no-mock HTTP | orders.sh | route `routes/orders.rs:18`; call `orders.sh:101` |

| POST /api/v1/requisitions | yes | true no-mock HTTP | requisitions.sh, coverage_extra.sh | `routes/requisitions.rs:66`; create calls || GET /api/v1/orders/cart | yes | true no-mock HTTP | orders.sh | route `routes/orders.rs:27`; call `orders.sh:111` |

| POST /api/v1/requisitions/{id}/submit | yes | true no-mock HTTP | requisitions.sh | `routes/requisitions.rs:77`; submit calls || POST /api/v1/orders/cart/lines | yes | true no-mock HTTP | orders.sh | route `routes/orders.rs:39`; calls e.g. `orders.sh:118` |

| POST /api/v1/requisitions/{id}/withdraw | yes | true no-mock HTTP | requisitions.sh | `routes/requisitions.rs:88`; withdraw calls || POST /api/v1/orders/cart/coupon | yes | true no-mock HTTP | orders.sh | route `routes/orders.rs:52`; call `orders.sh:176` |

| POST /api/v1/requisitions/{id}/approve | yes | true no-mock HTTP | requisitions.sh | `routes/requisitions.rs:99`; approve calls || POST /api/v1/orders/cart/coupon-remove | yes | true no-mock HTTP | coverage_extra.sh | route `routes/orders.rs:65`; call section 5 |

| POST /api/v1/requisitions/{id}/reject | yes | true no-mock HTTP | requisitions.sh | `routes/requisitions.rs:111`; reject calls || POST /api/v1/orders/cart/delivery | yes | true no-mock HTTP | orders.sh | route `routes/orders.rs:77`; call `orders.sh:132` |

| POST /api/v1/requisitions/{id}/send-back | yes | true no-mock HTTP | requisitions.sh | `routes/requisitions.rs:123`; send-back calls || POST /api/v1/orders/verify | yes | true no-mock HTTP | orders.sh | route `routes/orders.rs:90`; calls `orders.sh:270,275,349` |

| GET /api/v1/requisitions/{id} | yes | true no-mock HTTP | requisitions.sh | `routes/requisitions.rs:135`; get-one calls || POST /api/v1/orders/confirm | yes | true no-mock HTTP | orders.sh | route `routes/orders.rs:102`; calls `orders.sh:289,298,330` |

| GET /api/v1/requisitions/mine/list | yes | true no-mock HTTP | coverage_extra.sh | `routes/requisitions.rs:146`; mine-list coverage block || GET /api/v1/orders/mine | yes | true no-mock HTTP | orders.sh | route `routes/orders.rs:114`; call `orders.sh:317` |

| GET /api/v1/approvals/inbox | yes | true no-mock HTTP | requisitions.sh | `routes/requisitions.rs:152`; inbox call || GET /api/v1/orders/{id} | yes | true no-mock HTTP | orders.sh | route `routes/orders.rs:126`; call `orders.sh:359` |

| GET /api/v1/requisitions/{id}/audit | yes | true no-mock HTTP | coverage_extra.sh | `routes/requisitions.rs:158`; audit coverage block || GET /api/v1/master-data/institutions | yes | true no-mock HTTP | master_data.sh, phase6.sh, coverage_extra.sh | route `routes/master_data.rs:19`; calls `master_data.sh:112`, `phase6.sh:100` |

| GET /api/v1/issue-records/{req_id} | yes | true no-mock HTTP | requisitions.sh | `routes/requisitions.rs:169`; issue-record call || GET /api/v1/master-data/institutions/{iid}/semesters | yes | true no-mock HTTP | master_data.sh | route `routes/master_data.rs:32`; calls `master_data.sh:123,153` |

| GET /api/v1/orders/products | yes | true no-mock HTTP | orders.sh | `routes/orders.rs:9`; products call || GET /api/v1/master-data/semesters/{id} | yes | true no-mock HTTP | master_data.sh | route `routes/master_data.rs:44`; calls `master_data.sh:149,421` |

| GET /api/v1/orders/delivery-methods | yes | true no-mock HTTP | orders.sh | `routes/orders.rs:18`; delivery-methods call || POST /api/v1/master-data/institutions/{iid}/semesters | yes | true no-mock HTTP | master_data.sh | route `routes/master_data.rs:55`; calls `master_data.sh:131,139,144` |

| GET /api/v1/orders/cart | yes | true no-mock HTTP | orders.sh | `routes/orders.rs:27`; cart reads || POST /api/v1/master-data/semesters/{id}/update | yes | true no-mock HTTP | coverage_extra.sh | route `routes/master_data.rs:67`; call section 6c |

| POST /api/v1/orders/cart/lines | yes | true no-mock HTTP | orders.sh | `routes/orders.rs:39`; line set/remove calls || POST /api/v1/master-data/semesters/{id}/delete | yes | true no-mock HTTP | master_data.sh, coverage_extra.sh | route `routes/master_data.rs:79`; calls `master_data.sh:392,411` |

| POST /api/v1/orders/cart/coupon | yes | true no-mock HTTP | orders.sh | `routes/orders.rs:52`; coupon apply calls || GET /api/v1/master-data/institutions/{iid}/classes | yes | true no-mock HTTP | master_data.sh | route `routes/master_data.rs:93`; call `master_data.sh:213` |

| POST /api/v1/orders/cart/coupon-remove | yes | true no-mock HTTP | coverage_extra.sh | `routes/orders.rs:65`; coupon-remove coverage block || GET /api/v1/master-data/classes/{id} | yes | true no-mock HTTP | coverage_extra.sh | route `routes/master_data.rs:105`; call section 6b |

| POST /api/v1/orders/cart/delivery | yes | true no-mock HTTP | orders.sh | `routes/orders.rs:77`; delivery set calls || POST /api/v1/master-data/institutions/{iid}/classes | yes | true no-mock HTTP | master_data.sh | route `routes/master_data.rs:116`; calls `master_data.sh:195,201,208` |

| POST /api/v1/orders/verify | yes | true no-mock HTTP | orders.sh | `routes/orders.rs:90`; verify calls || POST /api/v1/master-data/classes/{id}/update | yes | true no-mock HTTP | coverage_extra.sh | route `routes/master_data.rs:128`; call section 6c |

| POST /api/v1/orders/confirm | yes | true no-mock HTTP | orders.sh | `routes/orders.rs:102`; confirm calls || POST /api/v1/master-data/classes/{id}/delete | yes | true no-mock HTTP | master_data.sh, coverage_extra.sh | route `routes/master_data.rs:140`; calls `master_data.sh:407`, cleanup in coverage_extra |

| GET /api/v1/orders/mine | yes | true no-mock HTTP | orders.sh | `routes/orders.rs:114`; list mine call || GET /api/v1/master-data/institutions/{iid}/courses | yes | true no-mock HTTP | master_data.sh | route `routes/master_data.rs:154`; call `master_data.sh:228` |

| GET /api/v1/orders/{id} | yes | true no-mock HTTP | orders.sh | `routes/orders.rs:126`; get-order call || GET /api/v1/master-data/courses/{id} | yes | true no-mock HTTP | coverage_extra.sh | route `routes/master_data.rs:166`; call section 6b |

| GET /api/v1/master-data/institutions | yes | true no-mock HTTP | master_data.sh, phase6.sh, coverage_extra.sh | `routes/master_data.rs:19`; institutions calls || POST /api/v1/master-data/institutions/{iid}/courses | yes | true no-mock HTTP | master_data.sh | route `routes/master_data.rs:177`; calls `master_data.sh:218,224` |

| GET /api/v1/master-data/institutions/{iid}/semesters | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:32`; semesters list call || POST /api/v1/master-data/courses/{id}/update | yes | true no-mock HTTP | coverage_extra.sh | route `routes/master_data.rs:189`; call section 6c |

| GET /api/v1/master-data/semesters/{id} | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:44`; get semester call || POST /api/v1/master-data/courses/{id}/delete | yes | true no-mock HTTP | master_data.sh, coverage_extra.sh | route `routes/master_data.rs:201`; call `master_data.sh:403` |

| POST /api/v1/master-data/institutions/{iid}/semesters | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:55`; create semester calls || GET /api/v1/master-data/institutions/{iid}/students | yes | true no-mock HTTP | master_data.sh, phase6.sh | route `routes/master_data.rs:215`; calls `master_data.sh:243`, `phase6.sh:127` |

| POST /api/v1/master-data/semesters/{id}/update | yes | true no-mock HTTP | coverage_extra.sh | `routes/master_data.rs:67`; update coverage block || GET /api/v1/master-data/students/{id} | yes | true no-mock HTTP | coverage_extra.sh | route `routes/master_data.rs:227`; call section 6b |

| POST /api/v1/master-data/semesters/{id}/delete | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:79`; delete calls || POST /api/v1/master-data/institutions/{iid}/students | yes | true no-mock HTTP | master_data.sh, coverage_extra.sh | route `routes/master_data.rs:238`; calls `master_data.sh:233,239` |

| GET /api/v1/master-data/institutions/{iid}/classes | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:93`; classes list call || POST /api/v1/master-data/students/{id}/update | yes | true no-mock HTTP | coverage_extra.sh | route `routes/master_data.rs:250`; call section 6c |

| GET /api/v1/master-data/classes/{id} | yes | true no-mock HTTP | coverage_extra.sh | `routes/master_data.rs:105`; get class coverage block || POST /api/v1/master-data/students/{id}/delete | yes | true no-mock HTTP | master_data.sh, coverage_extra.sh | route `routes/master_data.rs:262`; call `master_data.sh:399` |

| POST /api/v1/master-data/institutions/{iid}/classes | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:116`; class create calls || GET /api/v1/master-data/institutions/{iid}/departments | yes | true no-mock HTTP | master_data.sh, coverage_extra.sh | route `routes/master_data.rs:276`; calls `master_data.sh:165`, fixture discovery in coverage_extra |

| POST /api/v1/master-data/classes/{id}/update | yes | true no-mock HTTP | coverage_extra.sh | `routes/master_data.rs:128`; class update coverage block || GET /api/v1/master-data/departments/{id} | yes | true no-mock HTTP | coverage_extra.sh | route `routes/master_data.rs:288`; call section 6b |

| POST /api/v1/master-data/classes/{id}/delete | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:140`; class delete calls || POST /api/v1/master-data/institutions/{iid}/departments | yes | true no-mock HTTP | master_data.sh, coverage_extra.sh | route `routes/master_data.rs:299`; calls `master_data.sh:172,178`, fixture create in coverage_extra |

| GET /api/v1/master-data/institutions/{iid}/courses | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:154`; courses list call || POST /api/v1/master-data/departments/{id}/update | yes | true no-mock HTTP | coverage_extra.sh | route `routes/master_data.rs:311`; call section 6c |

| GET /api/v1/master-data/courses/{id} | yes | true no-mock HTTP | coverage_extra.sh | `routes/master_data.rs:166`; get course coverage block || POST /api/v1/master-data/departments/{id}/delete | yes | true no-mock HTTP | master_data.sh, coverage_extra.sh | route `routes/master_data.rs:323`; calls `master_data.sh:385,416` |

| POST /api/v1/master-data/institutions/{iid}/courses | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:177`; course create calls || POST /api/v1/master-data/institutions/{iid}/import/{entity_type} | yes | true no-mock HTTP | master_data.sh | route `routes/master_data.rs:337`; calls `master_data.sh:255,265,276,297,354` |

| POST /api/v1/master-data/courses/{id}/update | yes | true no-mock HTTP | coverage_extra.sh | `routes/master_data.rs:189`; course update coverage block || GET /api/v1/master-data/import-jobs/{job_id} | yes | true no-mock HTTP | master_data.sh | route `routes/master_data.rs:356`; calls `master_data.sh:282,340` |

| POST /api/v1/master-data/courses/{id}/delete | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:201`; course delete calls || GET /api/v1/master-data/institutions/{iid}/export/{entity_type} | yes | true no-mock HTTP | master_data.sh | route `routes/master_data.rs:375`; calls `master_data.sh:308,323,361,368` |

| GET /api/v1/master-data/institutions/{iid}/students | yes | true no-mock HTTP | master_data.sh, phase6.sh | `routes/master_data.rs:215`; students list calls || GET /api/v1/family/groups | yes | true no-mock HTTP | phase6.sh | route `routes/family.rs:24`; call `phase6.sh:371` |

| GET /api/v1/master-data/students/{id} | yes | true no-mock HTTP | coverage_extra.sh | `routes/master_data.rs:227`; get student coverage block || POST /api/v1/family/groups | yes | true no-mock HTTP | phase6.sh | route `routes/family.rs:35`; call `phase6.sh:107` |

| POST /api/v1/master-data/institutions/{iid}/students | yes | true no-mock HTTP | master_data.sh, phase6.sh | `routes/master_data.rs:238`; student create calls || POST /api/v1/family/groups/{id}/members | yes | true no-mock HTTP | phase6.sh | route `routes/family.rs:47`; call `phase6.sh:382` |

| POST /api/v1/master-data/students/{id}/update | yes | true no-mock HTTP | coverage_extra.sh | `routes/master_data.rs:250`; student update coverage block || POST /api/v1/family/groups/{id}/residents | yes | true no-mock HTTP | phase6.sh | route `routes/family.rs:60`; call `phase6.sh:130` |

| POST /api/v1/master-data/students/{id}/delete | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:262`; student delete calls || POST /api/v1/family/groups/{id}/consent | yes | true no-mock HTTP | phase6.sh | route `routes/family.rs:73`; calls `phase6.sh:155,188,399` |

| GET /api/v1/master-data/institutions/{iid}/departments | yes | true no-mock HTTP | master_data.sh, coverage_extra.sh | `routes/master_data.rs:276`; departments list calls || GET /api/v1/family/groups/{id}/supply-summary | yes | true no-mock HTTP | phase6.sh | route `routes/family.rs:86`; calls `phase6.sh:145,165` |

| GET /api/v1/master-data/departments/{id} | yes | true no-mock HTTP | coverage_extra.sh | `routes/master_data.rs:288`; get dept coverage block || GET /api/v1/family/groups/{id}/wellness-summary | yes | true no-mock HTTP | phase6.sh | route `routes/family.rs:98`; calls `phase6.sh:179,198,223` |

| POST /api/v1/master-data/institutions/{iid}/departments | yes | true no-mock HTTP | master_data.sh, coverage_extra.sh | `routes/master_data.rs:299`; create dept calls || GET /api/v1/family/groups/{id} | yes | true no-mock HTTP | phase6.sh | route `routes/family.rs:110`; call `phase6.sh:117` |

| POST /api/v1/master-data/departments/{id}/update | yes | true no-mock HTTP | coverage_extra.sh | `routes/master_data.rs:311`; dept update coverage block || GET /api/v1/family/groups/{id}/activity-log | yes | true no-mock HTTP | phase6.sh | route `routes/family.rs:122`; calls `phase6.sh:394,403` |

| POST /api/v1/master-data/departments/{id}/delete | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:323`; dept delete calls || POST /api/v1/family/wellness | yes | true no-mock HTTP | phase6.sh | route `routes/family.rs:134`; call `phase6.sh:211` |

| POST /api/v1/master-data/institutions/{iid}/import/{entity_type} | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:337`; upload import calls || GET /api/v1/analytics/dashboard | yes | true no-mock HTTP | phase6.sh | route `routes/analytics.rs:40`; calls `phase6.sh:236,268` |

| GET /api/v1/master-data/import-jobs/{job_id} | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:356`; import-job status calls || GET /api/v1/analytics/daily | yes | true no-mock HTTP | phase6.sh | route `routes/analytics.rs:52`; call `phase6.sh:247` |

| GET /api/v1/master-data/institutions/{iid}/export/{entity_type} | yes | true no-mock HTTP | master_data.sh | `routes/master_data.rs:375`; export calls || GET /api/v1/analytics/weekly | yes | true no-mock HTTP | phase6.sh | route `routes/analytics.rs:64`; call `phase6.sh:257` |

| GET /api/v1/family/groups | yes | true no-mock HTTP | phase6.sh | `routes/family.rs:24`; list groups calls || GET /api/v1/analytics/monthly | yes | true no-mock HTTP | phase6.sh | route `routes/analytics.rs:77`; call `phase6.sh:263` |

| POST /api/v1/family/groups | yes | true no-mock HTTP | phase6.sh | `routes/family.rs:35`; create group step || GET /api/v1/moderation/queue | yes | true no-mock HTTP | phase6.sh | route `routes/moderation.rs:22`; calls `phase6.sh:273,283` |

| POST /api/v1/family/groups/{id}/members | yes | true no-mock HTTP | phase6.sh | `routes/family.rs:47`; add member step || POST /api/v1/moderation/queue/{id}/review | yes | true no-mock HTTP | phase6.sh | route `routes/moderation.rs:34`; call `phase6.sh:316` |

| POST /api/v1/family/groups/{id}/residents | yes | true no-mock HTTP | phase6.sh | `routes/family.rs:60`; add resident step || GET /api/v1/moderation/policies | yes | true no-mock HTTP | phase6.sh | route `routes/moderation.rs:47`; call `phase6.sh:288` |

| POST /api/v1/family/groups/{id}/consent | yes | true no-mock HTTP | phase6.sh | `routes/family.rs:73`; consent grant steps || POST /api/v1/moderation/policies | yes | true no-mock HTTP | phase6.sh | route `routes/moderation.rs:58`; call `phase6.sh:299` |

| GET /api/v1/family/groups/{id}/supply-summary | yes | true no-mock HTTP | phase6.sh | `routes/family.rs:86`; strict 403→200 sequence || POST /api/v1/moderation/policies/{id}/delete | yes | true no-mock HTTP | phase6.sh | route `routes/moderation.rs:70`; call `phase6.sh:307` |

| GET /api/v1/family/groups/{id}/wellness-summary | yes | true no-mock HTTP | phase6.sh | `routes/family.rs:98`; strict 403→200 sequence || GET /api/v1/anomaly/events | yes | true no-mock HTTP | phase6.sh | route `routes/anomaly.rs:22`; calls `phase6.sh:326,336,361` |

| GET /api/v1/family/groups/{id} | yes | true no-mock HTTP | phase6.sh | `routes/family.rs:110`; non-member/member checks || POST /api/v1/anomaly/events/{id}/acknowledge | yes | true no-mock HTTP | phase6.sh | route `routes/anomaly.rs:35`; call `phase6.sh:352` |

| GET /api/v1/family/groups/{id}/activity-log | yes | true no-mock HTTP | phase6.sh | `routes/family.rs:122`; strict consent gate checks || GET /api/v1/anomaly/rules | yes | true no-mock HTTP | phase6.sh | route `routes/anomaly.rs:47`; call `phase6.sh:341` |

| POST /api/v1/family/wellness | yes | true no-mock HTTP | phase6.sh | `routes/family.rs:134`; log wellness step |

| GET /api/v1/analytics/dashboard | yes | true no-mock HTTP | phase6.sh | `routes/analytics.rs:40`; dashboard + non-admin denial |---

| GET /api/v1/analytics/daily | yes | true no-mock HTTP | phase6.sh | `routes/analytics.rs:52`; daily stats step |

| GET /api/v1/analytics/weekly | yes | true no-mock HTTP | phase6.sh | `routes/analytics.rs:64`; weekly stats step |## Coverage Summary

| GET /api/v1/analytics/monthly | yes | true no-mock HTTP | phase6.sh | `routes/analytics.rs:77`; monthly stats step |

| GET /api/v1/moderation/queue | yes | true no-mock HTTP | phase6.sh | `routes/moderation.rs:22`; queue + non-admin denial |- **Total endpoints:** 85

| POST /api/v1/moderation/queue/{id}/review | yes | true no-mock HTTP | phase6.sh | `routes/moderation.rs:34`; review-item step |- **Endpoints with HTTP tests:** 85

| GET /api/v1/moderation/policies | yes | true no-mock HTTP | phase6.sh | `routes/moderation.rs:47`; list policies step |- **Endpoints with true no-mock HTTP tests:** 85

| POST /api/v1/moderation/policies | yes | true no-mock HTTP | phase6.sh | `routes/moderation.rs:58`; create policy step |

| POST /api/v1/moderation/policies/{id}/delete | yes | true no-mock HTTP | phase6.sh | `routes/moderation.rs:70`; delete policy step |Computed metrics:

| GET /api/v1/anomaly/events | yes | true no-mock HTTP | phase6.sh | `routes/anomaly.rs:22`; list events + unacked filter |

| POST /api/v1/anomaly/events/{id}/acknowledge | yes | true no-mock HTTP | phase6.sh | `routes/anomaly.rs:35`; acknowledge step |- **HTTP coverage:** $85/85 = 100\%$

| GET /api/v1/anomaly/rules | yes | true no-mock HTTP | phase6.sh | `routes/anomaly.rs:47`; list rules step |- **True API coverage:** $85/85 = 100\%$

---**Strict caveat:** coverage here is endpoint-hit coverage by static script evidence, not branch/path coverage.

## Coverage Summary---

- **Total endpoints:** 85## Unit Test Analysis

- **Endpoints with HTTP tests:** 85

- **Endpoints with TRUE no-mock tests:** 85### Backend unit tests

Computed:**Test files / modules evidenced (non-HTTP):**

- **HTTP coverage:** $85/85 = 100\%$- `repo/apps/backend-api/src/security/signing.rs` (`#[cfg(test)]` around line 162)

- **True API coverage:** $85/85 = 100\%$- `repo/apps/backend-api/src/security/rate_limit.rs` (`#[cfg(test)]` around line 269)

- `repo/apps/backend-api/src/security/password.rs` (`#[cfg(test)]` around line 40)

Static caveat:- `repo/apps/backend-api/src/security/middleware.rs` (`#[cfg(test)]` around line 402)

- This is endpoint-hit sufficiency by code evidence, not runtime branch coverage.- `repo/apps/backend-api/src/security/signing_mode.rs` (`#[cfg(test)]` around line 78)

- `repo/apps/backend-api/src/orders/pricing.rs` (`#[cfg(test)]` around line 354)

---- `repo/apps/backend-api/src/requisitions/state.rs` (`#[cfg(test)]` around line 83)

- `repo/apps/backend-api/src/requisitions/engine.rs` (`#[cfg(test)]` around line 88)

## Unit Test Analysis- `repo/apps/backend-api/src/requisitions/service.rs` (`#[cfg(test)]` around line 743)

- `repo/apps/backend-api/src/family/service.rs` (`#[cfg(test)]` around line 652)

### Backend Unit Tests- `repo/apps/backend-api/src/analytics/service.rs` (`#[cfg(test)]` around line 340)

- `repo/apps/backend-api/src/moderation/service.rs` (`#[cfg(test)]` around line 378)

Evidence (representative non-HTTP unit modules):- `repo/apps/backend-api/src/anomaly/service.rs` (`#[cfg(test)]` around line 413)

- `repo/apps/backend-api/src/master_data/import.rs` (`#[cfg(test)]` around line 623)

- `repo/apps/backend-api/src/security/signing.rs`- plus cross-crate tests in `repo/unit_tests/tests/cross_crate.rs`.

- `repo/apps/backend-api/src/security/rate_limit.rs`

- `repo/apps/backend-api/src/security/password.rs`**Backend modules covered:**

- `repo/apps/backend-api/src/security/middleware.rs`

- `repo/apps/backend-api/src/security/signing_mode.rs`- Controllers/routes: light unit checks in route modules (e.g., `routes/auth.rs` helper tests).

- `repo/apps/backend-api/src/orders/pricing.rs`- Services/use-cases: requisitions, pricing/orders, family, analytics, moderation, anomaly.

- `repo/apps/backend-api/src/requisitions/state.rs`- Auth/guards/middleware: signing, password, signed middleware, rate limit, signing mode.

- `repo/apps/backend-api/src/requisitions/engine.rs`- Repository/data boundaries: exercised primarily through no-mock HTTP API tests, not isolated repository unit suites.

- `repo/apps/backend-api/src/requisitions/service.rs`

- `repo/apps/backend-api/src/family/service.rs`**Important backend modules not clearly unit-tested in isolation (by static evidence):**

- `repo/apps/backend-api/src/analytics/service.rs`

- `repo/apps/backend-api/src/moderation/service.rs`- `routes/master_data.rs` handlers (mostly covered via HTTP scripts only).

- `repo/apps/backend-api/src/anomaly/service.rs`- `routes/orders.rs`, `routes/requisitions.rs` handler-level pure unit tests (coverage comes via API scripts).

- `repo/apps/backend-api/src/master_data/import.rs`- DB adapter internals in `crates/infrastructure/src/db.rs`, migration/seed helpers (`migrations.rs`, `seed.rs`) as direct unit tests.

- `repo/unit_tests/tests/cross_crate.rs`

### Frontend unit tests (STRICT)

Backend modules covered:

- Controllers/routes: helper-level route tests (limited)Detection criteria check:

- Services: requisitions/orders/family/analytics/moderation/anomaly

- Security/auth/middleware: strong coverage- Identifiable frontend test files: **yes** (`apps/frontend-yew/tests/*.test.rs`).

- Repositories/adapters: more via API integration than isolated repository tests- Tests target frontend components/logic: **yes** (`App`, `Card`, `DataTable`, `Route`, `fmt_date`, `fmt_datetime`).

- Framework evident: **yes** (`wasm_bindgen_test`, Yew HTML/component usage).

Important backend modules not clearly unit-tested in isolation:- Real component/module imports: **yes**.

- Route handler behavior in `routes/master_data.rs`, `routes/orders.rs`, `routes/requisitions.rs` (covered via API/E2E, not deep unit isolation)

- `crates/infrastructure/src/db.rs` and migration/seed adapter internals as direct unit suites**Frontend test files:**

### Frontend Unit Tests (STRICT)- `repo/apps/frontend-yew/tests/app_render.test.rs`

- `repo/apps/frontend-yew/tests/components_utils.test.rs`

Detection rules check (all required):- `repo/apps/frontend-yew/tests/router_routes.test.rs`

- Identifiable frontend test files: **yes** (`*.test.rs` files in `apps/frontend-yew/tests/`)**Frameworks/tools detected:**

- Tests target frontend logic/components: **yes**

- Framework evident: **yes** (`wasm-bindgen-test`, Yew modules/components)- Rust test harness + `wasm-bindgen-test` + Yew component rendering/types.

- Real frontend imports/render/modules: **yes**

**Frontend modules covered:**

Frontend test files:

- `frontend_yew::App`

- `app_render.test.rs`- `frontend_yew::components::card::Card`

- `components_utils.test.rs`- `frontend_yew::components::table::DataTable`

- `router_routes.test.rs`- `frontend_yew::router::Route`

- `auth_state_behavior.test.rs`- `frontend_yew::components::utils::{fmt_date, fmt_datetime}`

- `router_guard.test.rs`

- `state_components.test.rs`**Important frontend modules not tested:**

- `auth_client_signing.test.rs`

- Auth flows in `apps/frontend-yew/src/auth/*`

Framework/tools detected:- Page-level module behavior in `apps/frontend-yew/src/pages/*`

- Rust + `wasm-bindgen-test` + Yew component/state/router modules- API client/signing behavior (if implemented in frontend modules)

- End-to-end UI interactions across route transitions and API responses

Components/modules covered:

- App shell and shared components### Mandatory verdict

- Router labels + guard decision behavior

- Auth state reducer behavior**Frontend unit tests: PRESENT**

- API signing client invariants

- UI state component prop contracts### Cross-layer observation

Important frontend modules not tested (remaining):- Both backend and frontend have tests, but **backend is substantially deeper**.

- Most page-level workflow modules in `apps/frontend-yew/src/pages/*` as isolated unit tests- Frontend tests are present but mostly smoke/utility-level; no true FE↔BE integration evidence.

- Layout/menu rendering behavior at per-role granularity in direct unit tests (partially covered by E2E)

---

### Mandatory verdict

## API Observability Check

**Frontend unit tests: PRESENT**

Strengths:

### Strict failure rule check

- Endpoint method+path are explicit in scripts (e.g., `call GET /api/v1/...`).

- Project type is `fullstack`, but frontend unit tests are present and materially expanded.- Inputs are explicit for many POSTs (JSON payloads inline).

- **No CRITICAL GAP** for missing frontend unit tests.- Status assertions are explicit and frequent.

### Cross-layer observationWeaknesses (observability quality is mixed):

- Testing is now substantially more balanced:- Several checks accept broad status alternatives (`200/403/404`) in `phase6.sh` consent-gate flows, reducing precision.
  - backend: deep API + unit- Some assertions are shallow existence checks (e.g., “is array”) without contract-level field validation.

  - frontend: expanded behavioral unit tests- Some scenario comments claim behaviors that are only partially asserted.

  - fullstack: added browser-driven FE↔BE E2E.

Verdict: **Moderate observability; not weak overall, but uneven across modules.**

---

---

## API Observability Check

## Test Quality & Sufficiency

Checks now generally explicit and strong:

- Endpoint METHOD+PATH visible in call wrappers (`call GET/POST /api/v1/...`).- **Success paths:** strong coverage across auth, requisitions lifecycle, orders flow, master-data CRUD/import/export, phase6 modules.

- Request inputs explicit (JSON body/query/path params).- **Failure paths:** good (auth failures, invalid signatures, validation failures, permissions, FK guards, unknown IDs/formats).

- Response contract assertions improved (status + key field/value/type assertions), especially in `phase6.sh`.- **Edge cases:** present (replay, lockout, coupon interactions, duplicate fingerprint, stale snapshot).

- **Auth/permissions:** strong across scripts (`auth.sh`, `phase6.sh`, `master_data.sh`, `coverage_extra.sh`).

Weaknesses still present (minor):- **Integration boundaries:** strong API-level integration (real HTTP + real handlers), but no browser-level integration.

- Some branches are conditionally skipped when dynamic data is absent (e.g., moderation queue review), which can reduce deterministic depth in one run context.

`run_tests.sh` check:

Verdict:

- **Not weak**; observability is **strong** overall after phase6 tightening.- **Docker-based:** yes (`repo/run_tests.sh` clearly containerized pipeline).

- **Local dependency requirement:** script enforces Docker on host; API scripts themselves require tools (`curl/jq/openssl/python3`) but are executed in test container per runner design.

---

---

## Test Quality & Sufficiency

## End-to-End Expectations (fullstack)

- Success paths: strong and broad across all functional domains.

- Failure/negative paths: strong (auth failures, permission denials, validation errors, missing IDs, format errors).- Expected: real FE↔BE end-to-end tests.

- Edge cases: present (replay, lockout, stale snapshot, duplicate fingerprints, consent gates).- Found: backend real HTTP suites + frontend unit tests, but **no explicit real browser-driven FE↔BE E2E suite** in audited files.

- Auth/permission boundaries: strong across API and E2E.- Compensation: strong backend API integration coverage partially compensates; does not fully replace fullstack E2E confidence.

- Integration boundaries:
  - API integration: strong true no-mock HTTP---

  - FE unit behavior: improved materially

  - FE↔BE E2E: now present (Playwright via proxy)## Unit Test Summary

`run_tests.sh` check:- Backend non-HTTP unit testing: **present and broad**.

- Docker-based orchestration: **OK** (`repo/run_tests.sh`).- Frontend unit testing: **present** (strictly verified).

- Local dependency posture: host requires Docker only; test/runtime toolchains are containerized.- Fullstack E2E FE↔BE: **not evidenced**.

---

## End-to-End Expectations (fullstack)## Tests Check

Expectation:- API suites are true HTTP (real endpoints) by static evidence.

- Fullstack should include real FE ↔ BE tests.- No mock framework usage found in test paths.

- Route-level endpoint hit coverage appears complete (85/85).

Evidence now present:

- `repo/e2e/tests/01-auth-dashboard.spec.ts`---

- `repo/e2e/tests/02-requisition-create.spec.ts`

- `repo/e2e/tests/03-orders-cart.spec.ts`## Test Coverage Score (0–100)

- Runner wiring in `repo/run_tests.sh` (E2E step) and `repo/docker-compose.tests.yml` (`e2e` service)

**Score: 88/100**

Verdict:

- Fullstack FE↔BE E2E expectation is **satisfied** by static evidence.### Score rationale

---- - Full endpoint hit coverage by no-mock HTTP scripts.

- - Strong auth/permission/negative-path coverage.

## Unit Test Summary- - Backend unit depth is substantial.

- - Frontend unit tests exist and are real module tests.

- Backend unit tests: **PRESENT (broad)**- − No explicit fullstack FE↔BE E2E tests.

- Frontend unit tests: **PRESENT (expanded behavioral scope)**- − Some assertions are permissive/weak in phase6 checks.

- Fullstack E2E tests: **PRESENT**

### Key gaps

---

1. Missing explicit FE↔BE E2E suite for fullstack confidence.

## Tests Check2. Frontend tests mostly smoke-level; limited behavior/state coverage.

3. Some API assertions tolerate multiple statuses and may hide regressions.

- API suites are true HTTP and no-mock by static evidence.

- Mock patterns (`jest.mock` / `vi.mock` / `sinon.stub` etc.) not found.### Confidence & assumptions

- Endpoint hit coverage remains complete (85/85).

- E2E layer now added for FE↔BE confidence.- **Confidence:** high for endpoint inventory and script-to-endpoint mapping; medium for behavioral sufficiency depth.

- **Assumptions:** API scripts are intended to run against real stack as documented; this audit did not execute them.

---

### Test Coverage Final Verdict

## Test Coverage Score (0–100)

**PASS with quality gaps** (coverage breadth is excellent; depth still improvable in FE and strictness).

**Score: 95/100**

---

## Score Rationale

# README Audit

- - 100% endpoint hit coverage with true no-mock API suites.

- - Strong backend unit depth.## Target README

- - Expanded frontend behavioral unit tests (auth state, guard logic, signing invariants, state components).

- - Real FE↔BE browser E2E coverage added.- Required location: `repo/README.md`

- − Minor non-deterministic branches remain in some scenario setup paths.- File exists: **yes**

- − Route-handler-specific unit isolation is still lighter than service/security layers.

## Hard Gate Evaluation

## Key Gaps

### 1) Formatting/readability

1. Some scenario blocks still allow skip behavior when specific seeded dynamic entities are absent (moderation queue item review path).

2. Handler-level direct unit isolation in select route modules remains limited.- Structured markdown with clear headings, tables, fenced commands.

3. Page-level frontend unit coverage can still deepen for more module-specific behavior.- **Result: PASS**

## Confidence & Assumptions### 2) Startup instructions (fullstack/backend requires `docker-compose up`)

- **Confidence:** high for endpoint inventory, test-file detection, and README compliance gates; medium-high for sufficiency depth judgment.- Explicit command present (`repo/README.md:24`).

- **Assumptions:** static evidence reflects intended run path; no runtime execution was performed by design.- Also provides `docker compose` and overlays.

- **Result: PASS**

### Test Coverage Audit Verdict

### 3) Access method (URL + port for web/backend)

**PASS**

- Explicit URL/port table provided (`README` access section around lines 59-74).

---- **Result: PASS**

# README Audit### 4) Verification method

## README Location Check- API verification via curl smoke (`README` around lines 83-101).

- Web UI verification flow with expected outcomes (`README` around lines 106-125).

- Required: `repo/README.md`- **Result: PASS**

- Exists: **yes**

### 5) Environment rules (Docker-contained, no runtime/manual host installs)

---

Findings:

## Hard Gates

- README strongly claims Docker-only for normal flow.

### Formatting- However, it includes a manual test command that runs `cargo install wasm-pack` at runtime inside test container (`repo/README.md:432`).

- Clean markdown structure, headings, tables, and command blocks.Strict interpretation of this audit’s gate (“DO NOT allow runtime installs”):

- **Result: PASS**

- **Result: FAIL (strict)** due to runtime install instruction.

### Startup Instructions (fullstack/backend must include `docker-compose up`)

### 6) Demo credentials (auth exists)

- `docker-compose up` explicitly present in quick-start section (`repo/README.md`, early quick-start block).

- **Result: PASS**- Auth clearly exists and demo credentials are provided with role table (`README` section “Demo credentials”, lines ~127+).

- Roles listed: admin, medical, approver, finance, senior, family.

### Access Method- **Result: PASS**

- URL and ports clearly documented (web/API/frontend/postgres access table).---

- **Result: PASS**

## Engineering Quality

### Verification Method

- Tech stack clarity: strong.

- API verification via curl plus full test runner command.- Architecture explanation: strong.

- UI verification workflow documented.- Testing instructions: extensive.

- **Result: PASS**- Security/roles/workflows: clearly described.

- Presentation quality: high.

### Environment Rules (STRICT)

---

Disallowed in README instructions:

- npm install## High Priority Issues

- pip install

- apt-get1. **Strict hard-gate conflict:** runtime install command (`cargo install wasm-pack`) is present in README manual testing path (`README.md:432`).

- runtime installs

- manual DB setup## Medium Priority Issues

Findings:1. README claims “full API regression — every signed route” and this is broadly true by script inventory, but does not explicitly disclose that some phase6 assertions are permissive (multi-status acceptance).

- README explicitly states Docker-contained tests and no host runtime installs.2. FE↔BE E2E path is described as manual UI flow, but no automated fullstack E2E command is documented.

- Manual run commands use pre-baked `tests`/`e2e` containers (`docker compose run ... tests/e2e ...`), no install command in README paths.

- No forbidden runtime install command found in README.## Low Priority Issues

**Result: PASS**1. README is long; critical quick-start and hard constraints could be summarized in a stricter checklist at top.

### Demo Credentials (auth conditional)## Hard Gate Failures

- Auth exists and demo credentials include username/email + password + roles.- **Environment Rules (STRICT): FAIL** due to runtime install instruction (`cargo install wasm-pack`) in README manual command block.

- **Result: PASS**

## README Verdict

---

**PARTIAL PASS**

## Engineering Quality

- Strong operational documentation and onboarding quality.

- Tech stack clarity: strong- One strict-compliance hard-gate failure remains.

- Architecture explanation: strong

- Testing instructions: strong and now includes FE↔BE E2E---

- Security/roles/workflows: detailed

- Presentation quality: high## Combined Final Verdicts

---- **Test Coverage Audit Verdict:** **PASS with quality gaps**

- **README Audit Verdict:** **PARTIAL PASS** (strict hard-gate failure on runtime install instruction)

## High Priority Issues

- None identified under strict gate criteria.

## Medium Priority Issues

1. README is extensive; quick-start critical path could be surfaced with a compact “strict checklist” near top for faster operator onboarding.

## Low Priority Issues

1. Some sections are highly detailed and could be split into docs files for maintainability, while retaining top-level summary.

## Hard Gate Failures

- **None**

## README Verdict

**PASS**

---

## Combined Final Verdicts

- **Test Coverage Audit:** **PASS** (score 95/100)
- **README Audit:** **PASS**
