# SilverOak Operations Suite — API Specification

## 1. Overview

This document specifies the REST-style API exposed by `apps/backend-api` under `/api/v1`.

- **Protocol:** HTTP in dev, HTTPS via proxy in prod-like profile
- **Payload format:** JSON unless explicitly noted (multipart upload / file download)
- **Primary server implementation:** Actix-web
- **Persistence backend:** PostgreSQL

This spec is implementation-aligned with current route wiring in `apps/backend-api/src/routes/`.

---

## 2. Base URL and Versioning

- **Base path:** `/api/v1`
- **Health path (public):** `/api/v1/health`

Versioning is path-based (`v1`).

---

## 3. Authentication and Request Signing

## 3.1 Endpoint security groups

### Public endpoints (no signed auth)

- `POST /auth/login`
- `POST /auth/recovery/request`
- `POST /auth/logout` (session cookie based)
- `GET /auth/session` (session cookie based)
- `GET /health`

### Protected endpoints (signed auth required)

All other `/api/v1/*` endpoints are protected by signed-auth middleware.

## 3.2 Signed request headers

Protected endpoints require:

- `x-silveroak-token` — API token
- `x-silveroak-timestamp` — unix seconds or RFC3339
- `x-silveroak-nonce` — unique per request
- `x-silveroak-signature` — HMAC signature
- `x-silveroak-body-hash` — required only in strict mode

## 3.3 Replay and freshness controls

- Timestamp window: ±60 seconds
- Signature replay prevention via `used_signatures` store

## 3.4 Signing modes

- `compat` — legacy canonical format
- `dual` — strict preferred, compat fallback
- `strict` — strict canonical + body hash required

---

## 4. Common Request/Response Conventions

## 4.1 Content types

- JSON requests: `Content-Type: application/json`
- Import upload: `multipart/form-data`
- Export download: CSV/XLSX content type with `Content-Disposition: attachment`

## 4.2 Error response envelope

All typed API errors return:

```json
{
  "code": "bad_request|unauthorized|forbidden|not_found|conflict|rate_limited|internal",
  "message": "human-readable message"
}
```

## 4.3 Common status codes

- `200 OK` — successful read/update action
- `201 Created` — successful creation (used in master-data create)
- `204 No Content` — delete success
- `207 Multi-Status` — import completed with rejected rows
- `400 Bad Request` — validation/business rule violation
- `401 Unauthorized` — missing/invalid auth/signature/session
- `403 Forbidden` — permission/scope/consent denial
- `404 Not Found` — entity not found
- `409 Conflict` — duplicate or conflict (e.g., import fingerprint)
- `429 Too Many Requests` — rate limited (`Retry-After` header)

---

## 5. Auth APIs

## 5.1 Login

### `POST /auth/login`

Authenticate user and issue short-lived API token + signing key.

**Request body**

```json
{
  "email": "user@silveroak.local",
  "password": "plain-text-input"
}
```

**Response 200**

```json
{
  "user_id": "uuid",
  "email": "user@silveroak.local",
  "roles": ["institution_admin"],
  "api_token": "token-string",
  "signing_key": "hmac-key",
  "token_expires_at": "2026-04-10T12:00:00Z"
}
```

Also sets session cookie (HTTP-only).

## 5.2 Logout

### `POST /auth/logout`

Revokes current session and active API token for that user.

**Response 200**

```json
{ "ok": true }
```

## 5.3 Current session

### `GET /auth/session`

Returns principal and optional active token expiry for current session cookie.

**Response 200**

```json
{
  "principal": {
    "user_id": "uuid",
    "email": "...",
    "roles": ["..."],
    "permissions": ["..."],
    "scopes": [{ "kind": "department", "reference": "uuid-or-key" }]
  },
  "token_expires_at": "2026-04-10T12:00:00Z"
}
```

## 5.4 Recovery request (offline admin-mediated)

### `POST /auth/recovery/request`

**Request body**

```json
{ "email": "user@silveroak.local" }
```

**Response 200**

```json
{ "ok": true }
```

Always returns OK to reduce user enumeration risk.

---

## 6. Health API

### `GET /health`

**Response 200**

```json
{
  "status": "ok",
  "version": "<cargo package version>"
}
```

---

## 7. Admin and Identity APIs

## 7.1 Principal

### `GET /me`

Returns authenticated principal object (roles, permissions, scopes).

## 7.2 IP blacklist

- `GET /admin/blacklist`
- `POST /admin/blacklist`
- `DELETE /admin/blacklist/{ip}`

`POST /admin/blacklist` body:

```json
{
  "ip": "203.0.113.7",
  "reason": "suspicious activity"
}
```

## 7.3 Role assignment

### `POST /admin/users/grant-role`

```json
{
  "user_id": "uuid",
  "role_key": "finance_approver"
}
```

Audited via permission-change audit mechanism.

## 7.4 Department residents scoped probe

### `GET /departments/{dept_id}/residents`

Requires `residents:read` + matching department scope.

Current response is intentionally minimal:

```json
{
  "department_id": "...",
  "residents": []
}
```

---

## 8. Requisitions and Approvals APIs

## 8.1 Inventory catalog for requisitioning

### `GET /inventory/catalog`

Returns requisition catalog items with stock/price/category/controlled flags.

## 8.2 Requisition CRUD/state transitions

- `POST /requisitions` — create draft
- `POST /requisitions/{id}/submit`
- `POST /requisitions/{id}/withdraw`
- `POST /requisitions/{id}/approve`
- `POST /requisitions/{id}/reject`
- `POST /requisitions/{id}/send-back`
- `GET /requisitions/{id}`
- `GET /requisitions/mine/list`
- `GET /approvals/inbox`
- `GET /requisitions/{id}/audit`
- `GET /issue-records/{req_id}`

### Create draft request body

```json
{
  "department_id": "uuid",
  "needed_by": "YYYY-MM-DD",
  "justification": "clinical and operational rationale",
  "lines": [{ "item_id": "uuid", "quantity": 2 }]
}
```

### Decision body (`approve/reject/send-back`)

```json
{
  "reason": "required for reject/send-back",
  "comment": "optional comment"
}
```

### Requisition detail response shape (high-level)

```json
{
	"id": "uuid",
	"ref_code": "REQ-000001",
	"status": "pending_approval",
	"requester_id": "uuid",
	"requester_email": "...",
	"site_id": "uuid",
	"department_id": "uuid",
	"needed_by": "YYYY-MM-DD",
	"justification": "...",
	"total_amount_cents": 12345,
	"contains_controlled": false,
	"workflow_id": "uuid-or-null",
	"lines": [ ... ],
	"route": [ ... ],
	"audit": [ ... ]
}
```

---

## 9. Orders / Cart / Checkout APIs

## 9.1 Catalog and delivery methods

- `GET /orders/products`
- `GET /orders/delivery-methods`

## 9.2 Cart operations

- `GET /orders/cart`
- `POST /orders/cart/lines`
- `POST /orders/cart/coupon`
- `POST /orders/cart/coupon-remove`
- `POST /orders/cart/delivery`

### Set line body

```json
{
  "product_id": "uuid",
  "quantity": 3
}
```

Quantity `0` removes a line.

### Apply coupon body

```json
{ "code": "WELCOME10" }
```

### Set delivery body

```json
{ "delivery_method_id": "uuid" }
```

## 9.3 Verify/confirm checkout

- `POST /orders/verify`
- `POST /orders/confirm`

`verify` re-checks pricing + inventory + daily limits and stores a short-lived snapshot.

`confirm` requires a valid snapshot and re-checks for price drift/stock in transaction.

## 9.4 Order history

- `GET /orders/mine`
- `GET /orders/{id}`

---

## 10. Master Data APIs

All master-data endpoints are under `/master-data/*` and require admin + institution scope.

## 10.1 Institutions

- `GET /master-data/institutions`

## 10.2 Semesters

- `GET /master-data/institutions/{iid}/semesters`
- `GET /master-data/semesters/{id}`
- `POST /master-data/institutions/{iid}/semesters`
- `POST /master-data/semesters/{id}/update`
- `POST /master-data/semesters/{id}/delete`

Input shape:

```json
{
  "code": "2026-S1",
  "label": "Semester 1",
  "starts_on": "YYYY-MM-DD",
  "ends_on": "YYYY-MM-DD",
  "is_active": true
}
```

## 10.3 Classes

- `GET /master-data/institutions/{iid}/classes`
- `GET /master-data/classes/{id}`
- `POST /master-data/institutions/{iid}/classes`
- `POST /master-data/classes/{id}/update`
- `POST /master-data/classes/{id}/delete`

Input shape:

```json
{
  "code": "CLS-01",
  "label": "Class A",
  "semester_id": "uuid-or-null",
  "department_id": "uuid-or-null",
  "is_active": true
}
```

## 10.4 Courses

- `GET /master-data/institutions/{iid}/courses`
- `GET /master-data/courses/{id}`
- `POST /master-data/institutions/{iid}/courses`
- `POST /master-data/courses/{id}/update`
- `POST /master-data/courses/{id}/delete`

Input shape:

```json
{
  "code": "CRS-101",
  "title": "Basic Care",
  "department_id": "uuid-or-null",
  "credits": 2,
  "is_active": true
}
```

## 10.5 Students

- `GET /master-data/institutions/{iid}/students`
- `GET /master-data/students/{id}`
- `POST /master-data/institutions/{iid}/students`
- `POST /master-data/students/{id}/update`
- `POST /master-data/students/{id}/delete`

Input shape:

```json
{
  "student_number": "STU-0001",
  "first_name": "Jane",
  "last_name": "Doe",
  "email": "jane@example.local",
  "date_of_birth": "YYYY-MM-DD",
  "class_id": "uuid-or-null",
  "is_active": true
}
```

## 10.6 Departments

- `GET /master-data/institutions/{iid}/departments`
- `GET /master-data/departments/{id}`
- `POST /master-data/institutions/{iid}/departments`
- `POST /master-data/departments/{id}/update`
- `POST /master-data/departments/{id}/delete`

Input shape:

```json
{
  "name": "Pharmacy",
  "site_id": "uuid",
  "is_active": true
}
```

## 10.7 Import/export

### Import

- `POST /master-data/institutions/{iid}/import/{entity_type}`
- Content type: `multipart/form-data`
- Multipart field name: `file`
- Entity types: `students|classes|courses|semesters|departments`

Response:

- `200` if all rows accepted
- `207` if partial success (some rows rejected)

Response shape:

```json
{
  "job_id": "uuid",
  "status": "done|failed",
  "total_rows": 120,
  "accepted_rows": 118,
  "rejected_rows": 2,
  "error_message": null
}
```

### Import job details

- `GET /master-data/import-jobs/{job_id}`

### Export

- `GET /master-data/institutions/{iid}/export/{entity_type}?format=csv|xlsx`
- Returns binary file body with headers:
  - `Content-Disposition: attachment; filename=...`
  - `X-Row-Count: <n>`

---

## 11. Family APIs

- `GET /family/groups`
- `POST /family/groups`
- `GET /family/groups/{id}`
- `POST /family/groups/{id}/members`
- `POST /family/groups/{id}/residents`
- `POST /family/groups/{id}/consent`
- `GET /family/groups/{id}/supply-summary`
- `GET /family/groups/{id}/wellness-summary`
- `GET /family/groups/{id}/activity-log`
- `POST /family/wellness?institution_id={uuid}`

### Create family group body

```json
{
  "institution_id": "uuid",
  "label": "Doe Family Group"
}
```

### Add member body

```json
{
  "user_id": "uuid",
  "role": "guardian"
}
```

### Add resident body

```json
{
  "student_id": "uuid"
}
```

### Set consent body

```json
{
  "data_category": "supply_usage|wellness_summary|activity_log",
  "consented": true
}
```

### Wellness log body

```json
{
  "student_id": "uuid-or-null",
  "activity_type": "exercise|therapy|social|nutrition",
  "duration_minutes": 30,
  "notes": "optional plain text (stored encrypted)",
  "activity_date": "YYYY-MM-DD"
}
```

---

## 12. Analytics APIs

- `GET /analytics/dashboard?institution_id={uuid}`
- `GET /analytics/daily?institution_id={uuid}&date=YYYY-MM-DD`
- `GET /analytics/weekly?institution_id={uuid}&week_start=YYYY-MM-DD`
- `GET /analytics/monthly?institution_id={uuid}&year_month=YYYY-MM`

All endpoints return JSON aggregates for the requested period/scope.

---

## 13. Moderation APIs

- `GET /moderation/queue?status={optional}`
- `POST /moderation/queue/{id}/review`
- `GET /moderation/policies`
- `POST /moderation/policies`
- `POST /moderation/policies/{id}/delete`

### Review body (service-aligned)

```json
{
  "decision": "approved|rejected|escalated",
  "comment": "optional moderator note"
}
```

### Create policy body (service-aligned)

```json
{
  "keyword": "restricted phrase",
  "severity": "low|medium|high",
  "is_active": true
}
```

---

## 14. Anomaly APIs

- `GET /anomaly/events?unacked={bool}`
- `POST /anomaly/events/{id}/acknowledge`
- `GET /anomaly/rules`

Used for security/compliance event monitoring and rule introspection.

---

## 15. Pagination and Filtering Conventions

Master-data list endpoints support query params:

- `page` (default 1)
- `per_page` (default 20, clamped)
- `filter`
- `sort`
- `order` (`asc|desc`)

Paginated response envelope:

```json
{
	"items": [ ... ],
	"total": 153,
	"page": 1,
	"per_page": 20
}
```

---

## 16. Security/Behavioral Guarantees Exposed Through API

1. Requisition finalization is transactional (no partial stock deduction).
2. Approval actions enforce role and scope checks.
3. Reject/send-back require reason.
4. Order confirm rejects stale or changed pricing after verify.
5. Daily per-SKU purchase limit enforced before order confirmation.
6. Family summaries require explicit consent records.
7. Replay and timestamp checks are mandatory for protected APIs.

---

## 17. Notes for API Consumers

1. Prefer ISO dates (`YYYY-MM-DD`) for all date fields.
2. Treat `reason` as mandatory for reject/send-back endpoints.
3. Always call `/orders/verify` shortly before `/orders/confirm`.
4. On `429`, honor `Retry-After`.
5. On `400` during confirm indicating price/stock drift, re-fetch cart and re-verify.
