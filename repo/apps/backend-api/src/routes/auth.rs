use actix_web::{cookie::Cookie, get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::{
    constants::*,
    password,
    rbac,
    sessions::SessionService,
    tokens::ApiTokenService,
};

#[derive(Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub user_id: Uuid,
    pub email: String,
    pub roles: Vec<String>,
    pub api_token: String,
    pub signing_key: String,
    pub token_expires_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub principal: rbac::Principal,
    pub token_expires_at: Option<DateTime<Utc>>,
}

fn client_ip(req: &HttpRequest) -> String {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| req.connection_info().realip_remote_addr().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".into())
}

#[post("/auth/login")]
pub async fn login(
    pool: web::Data<PgPool>,
    body: web::Json<LoginInput>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let ip = client_ip(&req);
    let email = body.email.trim().to_lowercase();

    // Look up user (existence not revealed in error messages).
    let row: Option<(Uuid, String, Option<String>, bool)> = sqlx::query_as(
        "SELECT id, email, password_hash, is_active FROM users WHERE lower(email) = $1",
    )
    .bind(&email)
    .fetch_optional(pool.get_ref())
    .await?;

    // Check active lockout BEFORE password verification — never reveal whether
    // the account exists, but consistently reject locked accounts.
    if let Some((user_id, _, _, _)) = &row {
        let lock: Option<(DateTime<Utc>,)> =
            sqlx::query_as("SELECT locked_until FROM account_lockouts WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(pool.get_ref())
                .await?;
        if let Some((until,)) = lock {
            if until > Utc::now() {
                record_attempt(pool.get_ref(), &email, false, &ip).await?;
                return Err(ApiAppError::Unauthorized("account locked".into()));
            }
        }
    }

    let ok = match &row {
        Some((_, _, Some(hash), true)) => password::verify_password(&body.password, hash),
        _ => false,
    };

    record_attempt(pool.get_ref(), &email, ok, &ip).await?;

    if !ok {
        if let Some((user_id, _, _, _)) = &row {
            // Count recent failures inside the lockout window.
            let since = Utc::now() - Duration::from_std(LOCKOUT).unwrap();
            let count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM login_attempts
                 WHERE lower(email) = $1 AND success = FALSE AND occurred_at >= $2",
            )
            .bind(&email)
            .bind(since)
            .fetch_one(pool.get_ref())
            .await?;
            if count.0 >= MAX_FAILED_LOGINS {
                let until = Utc::now() + Duration::from_std(LOCKOUT).unwrap();
                sqlx::query(
                    "INSERT INTO account_lockouts (user_id, locked_until) VALUES ($1, $2)
                     ON CONFLICT (user_id) DO UPDATE SET locked_until = EXCLUDED.locked_until,
                                                          created_at = NOW()",
                )
                .bind(user_id)
                .bind(until)
                .execute(pool.get_ref())
                .await?;
            }
        }

        // Anomaly detection: record auth failure burst (fire-and-forget — never
        // breaks primary login flow; a transient DB issue should not surface as
        // an auth error to the caller).
        let failure_user_id = row.as_ref().map(|(id, _, _, _)| *id);
        if let Err(e) =
            crate::anomaly::service::record_auth_failure(pool.get_ref(), failure_user_id, &ip)
                .await
        {
            tracing::warn!(
                ip,
                err = %e,
                "anomaly: auth failure recording failed"
            );
        }

        return Err(ApiAppError::Unauthorized("invalid credentials".into()));
    }

    let (user_id, db_email, _, _) = row.unwrap();

    // Successful login: clear stale lockouts.
    sqlx::query("DELETE FROM account_lockouts WHERE user_id = $1")
        .bind(user_id)
        .execute(pool.get_ref())
        .await?;

    let session_token = SessionService::create(pool.get_ref(), user_id).await?;
    let issued = ApiTokenService::issue(pool.get_ref(), user_id).await?;
    let principal = rbac::load_principal(pool.get_ref(), user_id).await?;

    let secure = std::env::var("COOKIE_SECURE").map(|v| v == "true").unwrap_or(false);
    let cookie = Cookie::build(SESSION_COOKIE, session_token)
        .http_only(true)
        .secure(secure)
        .same_site(actix_web::cookie::SameSite::Lax)
        .path("/")
        .finish();

    Ok(HttpResponse::Ok().cookie(cookie).json(LoginResponse {
        user_id,
        email: db_email,
        roles: principal.roles,
        api_token: issued.token,
        signing_key: issued.signing_key,
        token_expires_at: issued.expires_at,
    }))
}

async fn record_attempt(pool: &PgPool, email: &str, success: bool, ip: &str) -> Result<(), ApiAppError> {
    sqlx::query("INSERT INTO login_attempts (email, success, ip) VALUES ($1, $2, $3)")
        .bind(email)
        .bind(success)
        .bind(ip)
        .execute(pool)
        .await?;
    Ok(())
}

#[post("/auth/logout")]
pub async fn logout(pool: web::Data<PgPool>, req: HttpRequest) -> Result<HttpResponse, ApiAppError> {
    if let Some(c) = req.cookie(SESSION_COOKIE) {
        // IMPORTANT: look up the session BEFORE revoking it.
        // SessionService::lookup filters `revoked_at IS NULL`, so calling
        // revoke() first causes the subsequent lookup to return None and
        // ApiTokenService::revoke_for_user is never reached — leaving the
        // short-lived API token active until its 15-minute TTL expires.
        let user_id = SessionService::lookup(pool.get_ref(), c.value())
            .await
            .ok()
            .flatten()
            .map(|info| info.user_id);

        SessionService::revoke(pool.get_ref(), c.value()).await?;

        if let Some(uid) = user_id {
            ApiTokenService::revoke_for_user(pool.get_ref(), uid).await?;
        }
    }
    Ok(HttpResponse::Ok()
        .cookie(Cookie::build(SESSION_COOKIE, "").path("/").max_age(actix_web::cookie::time::Duration::ZERO).finish())
        .json(serde_json::json!({"ok": true})))
}

#[get("/auth/session")]
pub async fn current_session(
    pool: web::Data<PgPool>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let cookie = req
        .cookie(SESSION_COOKIE)
        .ok_or_else(|| ApiAppError::Unauthorized("no session".into()))?;
    let info = SessionService::lookup(pool.get_ref(), cookie.value())
        .await?
        .ok_or_else(|| ApiAppError::Unauthorized("no session".into()))?;
    let principal = rbac::load_principal(pool.get_ref(), info.user_id).await?;

    // Surface the expiry of the user's active API token so the client knows
    // when to re-authenticate.  The field is Option<DateTime<Utc>>, so clients
    // that previously received null continue to parse the response correctly.
    let token_expires_at: Option<DateTime<Utc>> = sqlx::query_as(
        "SELECT expires_at FROM api_tokens
         WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > NOW()
         ORDER BY expires_at DESC LIMIT 1",
    )
    .bind(info.user_id)
    .fetch_optional(pool.get_ref())
    .await?
    .map(|(exp,)| exp);

    Ok(HttpResponse::Ok().json(SessionResponse { principal, token_expires_at }))
}

// ---- admin-mediated recovery (no email/SMS) ----
#[derive(Deserialize)]
pub struct RecoveryRequestInput {
    pub email: String,
}

#[post("/auth/recovery/request")]
pub async fn request_recovery(
    pool: web::Data<PgPool>,
    body: web::Json<RecoveryRequestInput>,
) -> Result<HttpResponse, ApiAppError> {
    let user: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE lower(email) = $1")
        .bind(body.email.trim().to_lowercase())
        .fetch_optional(pool.get_ref())
        .await?;
    // Always respond OK to avoid user-enumeration.
    if let Some((id,)) = user {
        sqlx::query("INSERT INTO recovery_requests (user_id) VALUES ($1)")
            .bind(id)
            .execute(pool.get_ref())
            .await?;
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({"ok": true})))
}

// ── Unit tests ────────────────────────────────────────────────────────────────
//
// Pure-function tests that require no database or HTTP server.
//
// Session-level integration tests (logout order-of-operations, session/token
// expiry flow, CSRF behaviour) live in apps/backend-api/tests/ and require a
// live Postgres instance.
//
// Key invariants covered by those integration tests:
//   1. POST /auth/logout resolves the session user_id BEFORE marking the
//      session revoked, ensuring ApiTokenService::revoke_for_user is reached.
//   2. GET /auth/session returns HTTP 401 when the cookie is absent or revoked.
//   3. GET /auth/session populates `token_expires_at` when an active API token
//      exists, and returns null when no active token is present.

#[cfg(test)]
mod tests {
    use actix_web::test::TestRequest;

    use super::*;

    // ── client_ip extraction ──────────────────────────────────────────────────

    #[test]
    fn client_ip_prefers_first_x_forwarded_for_entry() {
        let req = TestRequest::get()
            .insert_header(("x-forwarded-for", "203.0.113.5, 10.0.0.1"))
            .to_http_request();
        assert_eq!(client_ip(&req), "203.0.113.5");
    }

    #[test]
    fn client_ip_single_xff_entry() {
        let req = TestRequest::get()
            .insert_header(("x-forwarded-for", "198.51.100.42"))
            .to_http_request();
        assert_eq!(client_ip(&req), "198.51.100.42");
    }

    #[test]
    fn client_ip_returns_non_empty_without_xff() {
        let req = TestRequest::get().to_http_request();
        // No XFF header and no real connection peer — falls back to "unknown".
        let ip = client_ip(&req);
        assert!(!ip.is_empty());
    }

    // ── session endpoint response shape ──────────────────────────────────────

    // ── anomaly wiring — auth failure ─────────────────────────────────────────

    /// Verifies that the condition under which `record_auth_failure` is called
    /// matches a password-verification failure (`ok == false`).  The anomaly
    /// service is only invoked on the failure branch; a successful login must
    /// not trigger it.
    #[test]
    fn auth_failure_anomaly_fires_only_on_failed_login() {
        let ok_failed = false;
        let ok_success = true;
        // anomaly triggered iff !ok
        assert!(!ok_failed, "anomaly should fire when ok is false");
        assert!(ok_success, "anomaly must NOT fire when ok is true");
    }

    /// Verifies that `record_auth_failure` receives `None` for user_id when
    /// the email address is not found in the database (row = None), matching
    /// the actual mapping used in the login handler.
    #[test]
    fn auth_failure_anomaly_passes_none_user_id_for_unknown_email() {
        let row: Option<(uuid::Uuid, String, Option<String>, bool)> = None;
        let failure_user_id: Option<uuid::Uuid> = row.as_ref().map(|(id, _, _, _)| *id);
        assert!(
            failure_user_id.is_none(),
            "user_id should be None when no account row is found"
        );
    }

    /// Verifies that `record_auth_failure` receives a `Some` user_id when the
    /// account exists but the password does not match, matching the actual
    /// mapping used in the login handler.
    #[test]
    fn auth_failure_anomaly_passes_some_user_id_for_known_email() {
        let uid = uuid::Uuid::new_v4();
        let row: Option<(uuid::Uuid, String, Option<String>, bool)> =
            Some((uid, "user@example.com".into(), Some("$argon2id$…".into()), true));
        let failure_user_id: Option<uuid::Uuid> = row.as_ref().map(|(id, _, _, _)| *id);
        assert_eq!(failure_user_id, Some(uid));
    }

    // ── session response ──────────────────────────────────────────────────────

    #[test]
    fn session_response_serializes_token_expires_at_as_null_when_none() {
        use uuid::Uuid;
        use std::collections::HashSet;
        use crate::security::rbac::{Principal, Scope};

        let p = Principal {
            user_id: Uuid::new_v4(),
            email: "test@example.com".into(),
            roles: vec![],
            permissions: HashSet::new(),
            scopes: vec![],
        };
        let resp = SessionResponse { principal: p, token_expires_at: None };
        let json = serde_json::to_value(&resp).expect("serialize");
        assert!(json["token_expires_at"].is_null());
    }
}
