# API_tests/

End-to-end HTTP tests that run against the live `backend-api` service
(typically the Dockerized stack).  Each phase script is self-contained —
it logs in, exercises a feature area, and exits non-zero on any failure.

## Prerequisites

- `curl`, `jq`, `openssl` must be in `$PATH`
- The stack must be running and reachable (default: `http://localhost:8080`)
- Seed data must be present: `admin@silveroak.local` and
  `medical@silveroak.local` with password `ChangeMeNow!2025`, at least one
  institution in the database

## Running

```bash
# All phases
for f in API_tests/phase*.sh; do bash "$f"; done

# Single phase
bash API_tests/phase6.sh

# Custom base URL
API_BASE=http://staging.internal:8080 bash API_tests/phase6.sh
```

## Auth model

All authenticated requests use HMAC-SHA256 signed tokens.

### Login

`POST /api/v1/auth/login` returns:
```json
{ "api_token": "…", "signing_key": "…", "token_expires_at": "…" }
```

Pass `api_token` in the `x-silveroak-token` header on every signed request.
Tokens expire after 15 minutes; re-login to obtain a fresh token and key.

### Canonical string

The client computes:
```
CANONICAL = METHOD + "\n" + PATH + "\n" + TIMESTAMP + "\n" + NONCE
```

| Field       | Value |
|-------------|-------|
| `METHOD`    | Upper-case HTTP verb (`GET`, `POST`, …) |
| `PATH`      | Bare resource path **without query string or fragment** — `?…` is stripped before signing |
| `TIMESTAMP` | Unix seconds (integer string, e.g. `1700000000`) |
| `NONCE`     | Per-request unique string (UUID or `n-RANDOM-RANDOM`) |

**Query strings are never signed.** A request to
`/api/v1/analytics/dashboard?institution_id=abc` signs the path
`/api/v1/analytics/dashboard`.  This matches the server-side verification which
uses `req.uri().path()` (Actix strips the query component automatically).

The bash `sign()` helper in each test script enforces this with:
```bash
local sign_path="${path%%\?*}"   # strip query string for canonical
```

### Signature and request headers

```
SIGNATURE = HMAC-SHA256(signing_key, CANONICAL)   # hex-encoded
```

Required headers on every signed request:
```
x-silveroak-token:     <api_token>
x-silveroak-timestamp: <TIMESTAMP>
x-silveroak-nonce:     <NONCE>
x-silveroak-signature: <SIGNATURE>
```

### Replay protection

The server rejects any request whose `(signature)` has been seen before within
the replay window (±60 seconds of server time).  The timestamp + nonce pair
ensures every legitimate request produces a unique signature.

### Backward compatibility note

The server also accepts the legacy canonical form where the query string IS
included in PATH.  This fallback will be removed in a future release once all
clients have been updated.  New clients must use the query-stripped form.

## Phase 6 — Compliance & Observability (`phase6.sh`)

Covers the Family portal, Analytics, Moderation, and Anomaly detection layers.

| Test | Area       | What is checked |
|------|------------|-----------------|
| 1    | Family     | Create family group (admin) |
| 2    | Family     | Non-member cannot view group detail (`GET /family/groups/{id}`) |
| 3    | Family     | Add resident to group (`POST /family/groups/{id}/residents`) |
| 4    | Family     | Supply summary blocked without `supply_usage` consent |
| 5    | Family     | Grant `supply_usage` consent |
| 6    | Family     | Supply summary accessible after consent |
| 7    | Family     | Wellness summary blocked without `wellness_summary` consent |
| 8    | Family     | Grant `wellness_summary` consent |
| 9    | Family     | Wellness summary accessible after consent |
| 10   | Family     | Log wellness activity (`POST /family/wellness?institution_id=`) |
| 11   | Family     | Response never exposes `notes_encrypted` field |
| 12   | Analytics  | Dashboard returns `live` counts (`GET /analytics/dashboard`) |
| 13   | Analytics  | Daily stats (`GET /analytics/daily`) |
| 14   | Analytics  | Weekly stats (`GET /analytics/weekly`) |
| 15   | Analytics  | Monthly stats (`GET /analytics/monthly`) |
| 16   | Analytics  | Non-admin receives 403 |
| 17   | Moderation | List queue (`GET /moderation/queue`) |
| 18   | Moderation | Non-admin receives 403 |
| 19   | Moderation | List keyword policies (`GET /moderation/policies`) |
| 20   | Moderation | Create keyword policy (`POST /moderation/policies`) |
| 21   | Moderation | Delete keyword policy (`POST /moderation/policies/{id}/delete`) |
| 22   | Moderation | Review queue item (`POST /moderation/queue/{id}/review`) |
| 23   | Anomaly    | List events (`GET /anomaly/events`) |
| 24   | Anomaly    | Non-admin receives 403 |
| 25   | Anomaly    | List detection rules (`GET /anomaly/rules`) |
| 26   | Anomaly    | Acknowledge event (`POST /anomaly/events/{id}/acknowledge`) |
| 27   | Anomaly    | `unacked=true` filter returns only unacknowledged events |
| 28   | Family     | Admin can list all groups (`GET /family/groups`) |
| 29   | Family     | Add member to group via user_id (`POST /family/groups/{id}/members`) |
| 30   | Family     | Activity-log consent gate (`GET /family/groups/{id}/activity-log`) |

### Key routes under test

```
POST   /api/v1/family/groups
GET    /api/v1/family/groups
GET    /api/v1/family/groups/{id}
POST   /api/v1/family/groups/{id}/members
POST   /api/v1/family/groups/{id}/residents
POST   /api/v1/family/groups/{id}/consent
GET    /api/v1/family/groups/{id}/supply-summary
GET    /api/v1/family/groups/{id}/wellness-summary
GET    /api/v1/family/groups/{id}/activity-log
POST   /api/v1/family/wellness?institution_id={uuid}

GET    /api/v1/analytics/dashboard?institution_id={uuid}
GET    /api/v1/analytics/daily?institution_id={uuid}&date={YYYY-MM-DD}
GET    /api/v1/analytics/weekly?institution_id={uuid}&week_start={YYYY-MM-DD}
GET    /api/v1/analytics/monthly?institution_id={uuid}&year_month={YYYY-MM}

GET    /api/v1/moderation/queue?status={pending|reviewed}
POST   /api/v1/moderation/queue/{id}/review
GET    /api/v1/moderation/policies
POST   /api/v1/moderation/policies
POST   /api/v1/moderation/policies/{id}/delete

GET    /api/v1/anomaly/events?unacked={true|false}
POST   /api/v1/anomaly/events/{id}/acknowledge
GET    /api/v1/anomaly/rules
```
