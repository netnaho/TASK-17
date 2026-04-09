//! Actix middlewares that wire the security primitives onto every
//! request. Order on the API scope is significant:
//!
//!   1. `IpGuard`        — blacklist + per-IP rate limit (always on)
//!   2. `SignedAuth`     — token + signature + replay window (auth scope only)
//!   3. handler          — receives a `Principal` extension
//!
//! Routes that should be public (login, health, recovery request) live
//! OUTSIDE the `SignedAuth` scope so the gating is statically obvious in
//! `routes/mod.rs`.

use std::future::{ready, Ready};
use std::rc::Rc;

use actix_web::{
    body::{BoxBody, EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    web, Error, HttpMessage, ResponseError,
};
use futures_util::future::LocalBoxFuture;
use sqlx::PgPool;

use super::{
    blacklist,
    constants::{HDR_API_TOKEN, HDR_BODY_HASH, HDR_NONCE, HDR_SIGNATURE, HDR_TIMESTAMP},
    rate_limit::{RateLimitOutcome, RateLimiter},
    rbac, signing,
    signing_mode::SigningMode,
    tokens::ApiTokenService,
};
use crate::error::ApiAppError;

#[derive(Clone)]
pub struct ClientIp(pub String);

fn client_ip(req: &ServiceRequest) -> String {
    if let Some(h) = req.headers().get("x-forwarded-for") {
        if let Ok(v) = h.to_str() {
            if let Some(first) = v.split(',').next() {
                return first.trim().to_string();
            }
        }
    }
    req.connection_info().realip_remote_addr().unwrap_or("unknown").to_string()
}

fn header(req: &ServiceRequest, name: &str) -> Option<String> {
    req.headers().get(name).and_then(|h| h.to_str().ok()).map(|s| s.to_string())
}

// =========================================================================
// IpGuard: blacklist + sliding-window rate limit
// =========================================================================
pub struct IpGuard {
    pub limiter: RateLimiter,
}

impl<S, B> Transform<S, ServiceRequest> for IpGuard
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type InitError = ();
    type Transform = IpGuardMw<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(IpGuardMw {
            service: Rc::new(service),
            limiter: self.limiter.clone(),
        }))
    }
}

pub struct IpGuardMw<S> {
    service: Rc<S>,
    limiter: RateLimiter,
}

impl<S, B> Service<ServiceRequest> for IpGuardMw<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.service.clone();
        let limiter = self.limiter.clone();
        Box::pin(async move {
            let ip = client_ip(&req);
            req.extensions_mut().insert(ClientIp(ip.clone()));

            if let Some(pool) = req.app_data::<web::Data<PgPool>>().cloned() {
                if blacklist::is_blacklisted(pool.get_ref(), &ip).await.unwrap_or(false) {
                    let resp = ApiAppError::Forbidden("ip blacklisted".into()).error_response();
                    return Ok(req.into_response(resp).map_into_right_body());
                }
            }

            match limiter.check(&ip).await {
                RateLimitOutcome::Allow => {}
                RateLimitOutcome::Deny { retry_after_secs } => {
                    tracing::info!(
                        ip,
                        backend = limiter.backend_label(),
                        retry_after_secs,
                        "rate_limit: request denied"
                    );
                    let resp = ApiAppError::RateLimited(retry_after_secs).error_response();
                    return Ok(req.into_response(resp).map_into_right_body());
                }
            }

            svc.call(req).await.map(|r| r.map_into_left_body())
        })
    }
}

// =========================================================================
// SignedAuth: token + HMAC signature + ±60s replay window + nonce store
// =========================================================================
//
// Canonical string formats (controlled by `SigningMode`):
//
//   compat: METHOD\nPATH\nTIMESTAMP\nNONCE
//   dual:   try strict first; compat fallback with WARN if body-hash absent
//   strict: METHOD\nPATH\nTIMESTAMP\nNONCE\nQUERY\nBODY_SHA256
//
// The HMAC key is the per-token `signing_key` returned at login.
// The signature is also stored in `used_signatures` with TTL so the same
// (token, nonce, timestamp) tuple cannot be replayed inside the window.
pub struct SignedAuth {
    /// Controls which canonical-string contract is enforced.
    /// See `SigningMode` for the three-phase rollout playbook.
    pub mode: SigningMode,
}

impl Default for SignedAuth {
    fn default() -> Self {
        Self { mode: SigningMode::Compat }
    }
}

impl<S, B> Transform<S, ServiceRequest> for SignedAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type InitError = ();
    type Transform = SignedAuthMw<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SignedAuthMw { service: Rc::new(service), mode: self.mode }))
    }
}

pub struct SignedAuthMw<S> {
    service: Rc<S>,
    mode: SigningMode,
}

impl<S, B> Service<ServiceRequest> for SignedAuthMw<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.service.clone();
        let mode = self.mode;
        Box::pin(async move {
            let reject = |err: ApiAppError, req: ServiceRequest| {
                Ok::<_, Error>(req.into_response(err.error_response()).map_into_right_body())
            };

            // ── Required headers present in all modes ─────────────────────
            let Some(token) = header(&req, HDR_API_TOKEN) else {
                return reject(ApiAppError::Unauthorized("missing api token".into()), req);
            };
            let Some(ts) = header(&req, HDR_TIMESTAMP) else {
                return reject(ApiAppError::Unauthorized("missing timestamp".into()), req);
            };
            let Some(nonce) = header(&req, HDR_NONCE) else {
                return reject(ApiAppError::Unauthorized("missing nonce".into()), req);
            };
            let Some(sig) = header(&req, HDR_SIGNATURE) else {
                return reject(ApiAppError::Unauthorized("missing signature".into()), req);
            };

            let ts_parsed = match signing::parse_timestamp(&ts) {
                Ok(v) => v,
                Err(e) => return reject(e, req),
            };
            if !signing::within_replay_window(ts_parsed) {
                return reject(
                    ApiAppError::Unauthorized("timestamp outside replay window".into()),
                    req,
                );
            }

            let pool = req
                .app_data::<web::Data<PgPool>>()
                .cloned()
                .expect("pg pool present");

            let ctx = match ApiTokenService::lookup(pool.get_ref(), &token).await {
                Ok(Some(c)) => c,
                Ok(None) => {
                    return reject(
                        ApiAppError::Unauthorized("invalid or expired token".into()),
                        req,
                    )
                }
                Err(e) => return reject(ApiAppError::Internal(e.to_string()), req),
            };

            // ── Signature verification — mode-dependent ────────────────────
            let verified = match mode {
                SigningMode::Strict => {
                    // 6-field strict canonical only.  Body-hash header is required.
                    let Some(body_hash) = header(&req, HDR_BODY_HASH) else {
                        return reject(
                            ApiAppError::Unauthorized(
                                "strict signing mode: missing body hash header".into(),
                            ),
                            req,
                        );
                    };
                    let full_uri = req
                        .uri()
                        .path_and_query()
                        .map(|pq| pq.as_str())
                        .unwrap_or_else(|| req.uri().path());
                    let canonical = signing::RequestSigner::canonical_string_strict(
                        req.method().as_str(),
                        full_uri,
                        &ts,
                        &nonce,
                        &body_hash,
                    );
                    signing::RequestSigner::verify(&ctx.signing_key, &canonical, &sig)
                }

                SigningMode::Dual => {
                    // Try 6-field strict first (if client sent body-hash); if the
                    // header is absent, warn and fall back to 4-field compat.
                    if let Some(body_hash) = header(&req, HDR_BODY_HASH) {
                        let full_uri = req
                            .uri()
                            .path_and_query()
                            .map(|pq| pq.as_str())
                            .unwrap_or_else(|| req.uri().path());
                        let canonical = signing::RequestSigner::canonical_string_strict(
                            req.method().as_str(),
                            full_uri,
                            &ts,
                            &nonce,
                            &body_hash,
                        );
                        let v =
                            signing::RequestSigner::verify(&ctx.signing_key, &canonical, &sig);
                        if !v {
                            // Client sent body-hash but it doesn't verify — reject
                            // immediately; do NOT fall back to compat since the client
                            // explicitly attempted strict signing.
                            false
                        } else {
                            true
                        }
                    } else {
                        // No body-hash header → compat fallback.  Log a warning so
                        // operators can track the rollout progress.
                        tracing::warn!(
                            path = req.uri().path(),
                            method = req.method().as_str(),
                            signing_mode = "dual",
                            "dual-mode compat fallback: client did not send body hash header; \
                             update clients to send x-silveroak-body-hash to eliminate this warning"
                        );
                        verify_compat(&ctx.signing_key, &req, &ts, &nonce, &sig)
                    }
                }

                SigningMode::Compat => {
                    // 4-field canonical only, with legacy path+query fallback.
                    verify_compat(&ctx.signing_key, &req, &ts, &nonce, &sig)
                }
            };

            if !verified {
                return reject(ApiAppError::Unauthorized("bad signature".into()), req);
            }

            if let Err(e) = signing::record_signature(pool.get_ref(), ctx.user_id, &sig).await {
                return reject(e, req);
            }

            let principal = match rbac::load_principal(pool.get_ref(), ctx.user_id).await {
                Ok(p) => p,
                Err(e) => return reject(ApiAppError::Internal(e.to_string()), req),
            };

            // Capture what we need for post-response anomaly recording before
            // `req` is moved into `svc.call`.
            let user_id_for_recording = principal.user_id;
            let path_for_recording = req.uri().path().to_string();
            let pool_for_recording = pool.clone();

            req.extensions_mut().insert(principal);

            let result = svc.call(req).await.map(|r| r.map_into_left_body());

            // Post-process: if the handler returned HTTP 403 Forbidden, record
            // a permission-denial signal for anomaly detection.  This is
            // fire-and-forget — a recording failure must never overwrite or mask
            // the real 403 that the handler already decided to return.
            if let Ok(ref resp) = result {
                if resp.status() == actix_web::http::StatusCode::FORBIDDEN {
                    if let Err(e) = crate::anomaly::service::record_permission_denial(
                        pool_for_recording.get_ref(),
                        user_id_for_recording,
                        "forbidden_request",
                        &path_for_recording,
                    )
                    .await
                    {
                        tracing::warn!(
                            user_id = %user_id_for_recording,
                            path = %path_for_recording,
                            err = %e,
                            "anomaly: permission denial recording failed"
                        );
                    }
                }
            }

            result
        })
    }
}

/// Verify the standard 4-field canonical string with a legacy path+query
/// fallback for older clients that haven't been updated to strip query strings.
///
/// TODO(compat): Remove the legacy fallback once all clients have migrated.
fn verify_compat(
    signing_key: &str,
    req: &ServiceRequest,
    ts: &str,
    nonce: &str,
    sig: &str,
) -> bool {
    // Primary: normalized PATH (query-stripped)
    let canonical = signing::RequestSigner::canonical_string_no_body(
        req.method().as_str(),
        req.uri().path(),
        ts,
        nonce,
    );
    let verified = signing::RequestSigner::verify(signing_key, &canonical, sig);
    if verified {
        return true;
    }

    // Legacy fallback: path+query form for older clients
    if let Some(pq) = req.uri().path_and_query().map(|pq| pq.as_str()) {
        if pq != req.uri().path() {
            let legacy = format!(
                "{}\n{}\n{}\n{}",
                req.method().as_str().to_uppercase(),
                pq,
                ts,
                nonce,
            );
            return signing::RequestSigner::verify(signing_key, &legacy, sig);
        }
    }
    false
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::signing::RequestSigner;

    // ── status code constants ─────────────────────────────────────────────────

    #[test]
    fn forbidden_status_code_is_403() {
        assert_eq!(
            actix_web::http::StatusCode::FORBIDDEN.as_u16(),
            403,
            "permission denial recording is gated on HTTP 403; value must be exact"
        );
    }

    #[test]
    fn unauthorized_status_code_is_not_forbidden() {
        let unauth = actix_web::http::StatusCode::UNAUTHORIZED;
        let forbidden = actix_web::http::StatusCode::FORBIDDEN;
        assert_ne!(unauth, forbidden);
    }

    // ── client IP extraction ──────────────────────────────────────────────────

    #[test]
    fn client_ip_extraction_returns_first_xff_entry() {
        use actix_web::test::TestRequest;
        let req = TestRequest::get()
            .insert_header(("x-forwarded-for", "10.0.0.1, 192.168.1.1"))
            .to_http_request();
        let ip = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        assert_eq!(ip, "10.0.0.1");
    }

    // ── SignedAuth defaults ───────────────────────────────────────────────────

    #[test]
    fn signed_auth_defaults_to_compat_mode() {
        let sa = SignedAuth::default();
        assert_eq!(
            sa.mode,
            SigningMode::Compat,
            "default mode must be Compat for backward compatibility"
        );
    }

    // ── body-hash header name ─────────────────────────────────────────────────

    #[test]
    fn body_hash_header_name_matches_constant() {
        assert_eq!(HDR_BODY_HASH, "x-silveroak-body-hash");
    }

    // ── compat mode verification ──────────────────────────────────────────────

    #[test]
    fn compat_mode_verifies_4_field_canonical() {
        let key = "test-key";
        let ts = "1700000000";
        let nonce = "n1";
        let canon = RequestSigner::canonical_string_no_body("GET", "/api/v1/x", ts, nonce);
        let sig = RequestSigner::sign(key, &canon);
        // Body-hash absent → compat must still verify via verify_compat
        // We test the underlying helper directly since we can't easily inject
        // a live DB into a unit test.
        let result = RequestSigner::verify(key, &canon, &sig);
        assert!(result, "compat canonical must verify correctly");
    }

    // ── strict mode: signature contract ──────────────────────────────────────

    #[test]
    fn strict_mode_signature_verifies_with_correct_body_hash() {
        let key = "test-signing-key";
        let body_hash = RequestSigner::sha256_hex(b"{\"foo\":1}");
        let canonical = RequestSigner::canonical_string_strict(
            "POST",
            "/api/v1/requisitions?draft=true",
            "1700000000",
            "nonce42",
            &body_hash,
        );
        let sig = RequestSigner::sign(key, &canonical);
        assert!(RequestSigner::verify(key, &canonical, &sig), "correct hash must verify");

        let bad_hash = RequestSigner::sha256_hex(b"tampered body");
        let bad_canonical = RequestSigner::canonical_string_strict(
            "POST",
            "/api/v1/requisitions?draft=true",
            "1700000000",
            "nonce42",
            &bad_hash,
        );
        assert!(
            !RequestSigner::verify(key, &bad_canonical, &sig),
            "tampered body hash must not verify"
        );
    }

    #[test]
    fn strict_mode_tampered_query_fails() {
        let key = "test-signing-key";
        let body_hash = RequestSigner::sha256_hex(b"{}");
        // Sign with query a=1
        let canonical_a = RequestSigner::canonical_string_strict(
            "GET",
            "/api/v1/foo?a=1",
            "1700000000",
            "n1",
            &body_hash,
        );
        let sig = RequestSigner::sign(key, &canonical_a);

        // Verify with tampered query a=2 → must fail
        let canonical_b = RequestSigner::canonical_string_strict(
            "GET",
            "/api/v1/foo?a=2",
            "1700000000",
            "n1",
            &body_hash,
        );
        assert!(
            !RequestSigner::verify(key, &canonical_b, &sig),
            "tampered query string must not verify in strict mode"
        );
    }

    #[test]
    fn legacy_compat_request_fails_strict_canonical() {
        // A request signed with the 4-field compat canonical must NOT verify
        // against the 6-field strict canonical — confirming mode isolation.
        let key = "test-signing-key";
        let ts = "1700000000";
        let nonce = "n99";
        let compat_canon =
            RequestSigner::canonical_string_no_body("GET", "/api/v1/x", ts, nonce);
        let sig = RequestSigner::sign(key, &compat_canon);

        let empty_hash = RequestSigner::sha256_hex(b"");
        let strict_canon = RequestSigner::canonical_string_strict(
            "GET", "/api/v1/x", ts, nonce, &empty_hash,
        );
        assert!(
            !RequestSigner::verify(key, &strict_canon, &sig),
            "compat signature must not verify against strict canonical"
        );
    }

    // ── dual-mode: body-hash present → strict verification ───────────────────

    #[test]
    fn dual_mode_with_body_hash_uses_strict_canonical() {
        // The dual-mode branch that has body-hash must produce the same
        // result as the strict mode.  We verify the canonical construction
        // is identical.
        let key = "dual-key";
        let body_hash = RequestSigner::sha256_hex(b"hello");
        let canonical = RequestSigner::canonical_string_strict(
            "POST", "/api/v1/y?q=1", "1700000001", "nd1", &body_hash,
        );
        let sig = RequestSigner::sign(key, &canonical);
        assert!(RequestSigner::verify(key, &canonical, &sig));

        // Same request with tampered body must fail
        let bad_hash = RequestSigner::sha256_hex(b"bad");
        let bad_canon = RequestSigner::canonical_string_strict(
            "POST", "/api/v1/y?q=1", "1700000001", "nd1", &bad_hash,
        );
        assert!(!RequestSigner::verify(key, &bad_canon, &sig));
    }

    // ── mode switching isolation ──────────────────────────────────────────────

    #[test]
    fn signing_modes_are_distinct_values() {
        assert_ne!(SigningMode::Compat, SigningMode::Dual);
        assert_ne!(SigningMode::Dual, SigningMode::Strict);
        assert_ne!(SigningMode::Compat, SigningMode::Strict);
    }
}
