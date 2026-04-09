use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use super::constants::REPLAY_WINDOW;
use crate::error::ApiAppError;

type HmacSha256 = Hmac<Sha256>;

/// Canonical string the client must sign:
///   METHOD\nPATH\nTIMESTAMP\nNONCE
///
/// PATH is always the bare resource path **without query string or fragment**
/// (e.g. `/api/v1/analytics/dashboard` regardless of any `?institution_id=…`
/// suffix).  Clients must strip the query before computing the canonical string.
///
/// The HMAC signing key is issued once at login (alongside the API token)
/// and held server-side in `api_tokens.signing_key`. The bearer token is
/// stored only as a SHA-256 hash so a DB leak alone does not yield usable
/// credentials. See `API_tests/README.md` for the full auth model.
pub struct RequestSigner;

/// Return the signing path for a raw request URI, stripping any query string.
///
/// This is the single source of truth for the PATH component of the canonical
/// string: `signing_path("/api/v1/foo?bar=baz")` → `"/api/v1/foo"`.
/// Idempotent — safe to call on an already-stripped path.
pub fn signing_path(raw: &str) -> &str {
    // Everything before the first '?' is the path component.
    raw.split_once('?').map_or(raw, |(path, _)| path)
}

impl RequestSigner {
    /// Canonical string used by `SignedAuth`. The HTTP body is intentionally
    /// excluded so the middleware can authenticate without consuming the
    /// payload — body integrity rides on the TLS channel terminated at the
    /// reverse proxy.
    ///
    /// The `path` argument MAY include a query string; it is stripped
    /// internally via `signing_path` so callers do not need to pre-strip.
    pub fn canonical_string_no_body(method: &str, path: &str, timestamp: &str, nonce: &str) -> String {
        format!("{}\n{}\n{}\n{}", method.to_uppercase(), signing_path(path), timestamp, nonce)
    }

    /// **Strict-mode** canonical string (6 fields).
    ///
    /// Extends the standard 4-field form with:
    ///   5. `QUERY`      — the raw query string (without the `?` prefix), or
    ///                     an empty string if the URL has no query component.
    ///   6. `BODY_SHA256` — hex-encoded SHA-256 of the request body, as
    ///                     supplied by the client in the `x-silveroak-body-hash`
    ///                     header.  For bodyless methods (GET / HEAD / DELETE)
    ///                     the client must use the SHA-256 of an empty sequence:
    ///                     `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
    ///
    /// Layout:
    /// ```text
    /// METHOD\nPATH\nTIMESTAMP\nNONCE\nQUERY\nBODY_SHA256
    /// ```
    ///
    /// The `full_uri` argument should be the complete request URI
    /// (`path?query` or just `path`); the path and query components are split
    /// internally.
    pub fn canonical_string_strict(
        method: &str,
        full_uri: &str,
        timestamp: &str,
        nonce: &str,
        body_sha256_hex: &str,
    ) -> String {
        let (path, query) = full_uri
            .split_once('?')
            .map(|(p, q)| (p, q))
            .unwrap_or((full_uri, ""));
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method.to_uppercase(),
            path,
            timestamp,
            nonce,
            query,
            body_sha256_hex,
        )
    }

    /// Compute the SHA-256 of `body` and return it as a lowercase hex string.
    /// Used server-side to verify or re-derive the body hash for testing.
    pub fn sha256_hex(body: &[u8]) -> String {
        let hash = Sha256::digest(body);
        hex::encode(hash)
    }

    pub fn sign(signing_key: &str, canonical: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(signing_key.as_bytes()).expect("hmac key");
        mac.update(canonical.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    pub fn verify(signing_key: &str, canonical: &str, provided_hex: &str) -> bool {
        let expected = Self::sign(signing_key, canonical);
        // constant-time-ish: compare via byte equality after length check
        if expected.len() != provided_hex.len() {
            return false;
        }
        let mut diff = 0u8;
        for (a, b) in expected.as_bytes().iter().zip(provided_hex.as_bytes().iter()) {
            diff |= a ^ b;
        }
        diff == 0
    }
}

/// Parse RFC3339 *or* unix-seconds and return a UTC datetime.
pub fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, ApiAppError> {
    if let Ok(secs) = value.parse::<i64>() {
        return DateTime::<Utc>::from_timestamp(secs, 0)
            .ok_or_else(|| ApiAppError::BadRequest("invalid timestamp".into()));
    }
    DateTime::parse_from_rfc3339(value)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| ApiAppError::BadRequest("invalid timestamp".into()))
}

pub fn within_replay_window(ts: DateTime<Utc>) -> bool {
    let now = Utc::now();
    let skew = Duration::from_std(REPLAY_WINDOW).unwrap();
    ts >= now - skew && ts <= now + skew
}

/// Persist a request signature so a replay within the window is rejected.
/// Returns `Err(BadRequest("replay"))` if already seen.
pub async fn record_signature(pool: &PgPool, user_id: Uuid, signature: &str) -> Result<(), ApiAppError> {
    let expires_at = Utc::now() + Duration::from_std(REPLAY_WINDOW).unwrap() * 2;
    let res = sqlx::query(
        "INSERT INTO used_signatures (signature, user_id, expires_at) VALUES ($1, $2, $3)
         ON CONFLICT (signature) DO NOTHING",
    )
    .bind(signature)
    .bind(user_id)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|e| ApiAppError::Internal(e.to_string()))?;
    if res.rows_affected() == 0 {
        return Err(ApiAppError::BadRequest("replayed request signature".into()));
    }
    Ok(())
}

/// Best-effort cleanup of expired entries; called by the worker but exposed
/// here so tests and admin endpoints can trigger it too.
pub async fn purge_expired_signatures(pool: &PgPool) -> Result<u64> {
    let res = sqlx::query("DELETE FROM used_signatures WHERE expires_at < NOW()")
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_round_trip() {
        let canon = RequestSigner::canonical_string_no_body(
            "POST",
            "/api/v1/things",
            "1700000000",
            "nonce-123",
        );
        let sig = RequestSigner::sign("secret-key", &canon);
        assert!(RequestSigner::verify("secret-key", &canon, &sig));
        assert!(!RequestSigner::verify("other-key", &canon, &sig));
        assert!(!RequestSigner::verify("secret-key", &canon, "deadbeef"));
    }

    #[test]
    fn replay_window_bounds() {
        let now = Utc::now();
        assert!(within_replay_window(now));
        assert!(within_replay_window(now - Duration::seconds(59)));
        assert!(within_replay_window(now + Duration::seconds(59)));
        assert!(!within_replay_window(now - Duration::seconds(120)));
        assert!(!within_replay_window(now + Duration::seconds(120)));
    }

    #[test]
    fn canonical_string_uppercases_method() {
        let c1 = RequestSigner::canonical_string_no_body("get", "/path", "123", "nonce");
        let c2 = RequestSigner::canonical_string_no_body("GET", "/path", "123", "nonce");
        assert_eq!(c1, c2);
    }

    #[test]
    fn canonical_string_is_newline_separated() {
        let c = RequestSigner::canonical_string_no_body(
            "POST", "/api/v1/test", "1700000000", "nonce-xyz",
        );
        let parts: Vec<&str> = c.split('\n').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "POST");
        assert_eq!(parts[1], "/api/v1/test");
        assert_eq!(parts[2], "1700000000");
        assert_eq!(parts[3], "nonce-xyz");
    }

    #[test]
    fn different_keys_produce_different_signatures() {
        let canon =
            RequestSigner::canonical_string_no_body("GET", "/test", "123", "n");
        let sig1 = RequestSigner::sign("key-one", &canon);
        let sig2 = RequestSigner::sign("key-two", &canon);
        assert_ne!(sig1, sig2);
    }

    // ── signing_path ──────────────────────────────────────────────────────────

    #[test]
    fn signing_path_strips_query_string() {
        assert_eq!(
            signing_path("/api/v1/analytics/dashboard?institution_id=abc"),
            "/api/v1/analytics/dashboard"
        );
    }

    #[test]
    fn signing_path_strips_multi_param_query() {
        assert_eq!(
            signing_path("/api/v1/analytics/daily?institution_id=abc&date=2024-01-01"),
            "/api/v1/analytics/daily"
        );
    }

    #[test]
    fn signing_path_no_query_unchanged() {
        assert_eq!(signing_path("/api/v1/family/groups"), "/api/v1/family/groups");
    }

    #[test]
    fn signing_path_is_idempotent() {
        let p = "/api/v1/test";
        assert_eq!(signing_path(signing_path(p)), p);
    }

    // ── canonical_string_no_body with query paths ─────────────────────────────

    #[test]
    fn canonical_with_query_path_equals_canonical_with_stripped_path() {
        // A caller that passes the full path+query gets the same canonical as one
        // that pre-strips.  This is what makes the server-side verification correct
        // regardless of how the URI arrives in the middleware.
        let ts = "1700000000";
        let nonce = "n42";
        let with_query =
            RequestSigner::canonical_string_no_body("GET", "/api/v1/foo?bar=baz", ts, nonce);
        let stripped =
            RequestSigner::canonical_string_no_body("GET", "/api/v1/foo", ts, nonce);
        assert_eq!(with_query, stripped);
    }

    #[test]
    fn canonical_no_query_path_has_four_newline_parts() {
        let c = RequestSigner::canonical_string_no_body(
            "GET", "/api/v1/analytics/dashboard?institution_id=xyz", "1700000000", "nonce-abc",
        );
        let parts: Vec<&str> = c.split('\n').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "GET");
        // PATH component must never contain a query string.
        assert_eq!(parts[1], "/api/v1/analytics/dashboard");
        assert!(!parts[1].contains('?'), "signing path must not contain query string");
        assert_eq!(parts[2], "1700000000");
        assert_eq!(parts[3], "nonce-abc");
    }

    #[test]
    fn replay_protection_not_affected_by_query_stripping() {
        // Two requests to the same endpoint with different query strings but the
        // same method, timestamp and nonce WOULD produce the same canonical —
        // and therefore the same signature.  The replay window (timestamp + nonce
        // uniqueness) is the real replay guard; query params are not part of the
        // signed surface.  This test documents the intentional design.
        let c1 = RequestSigner::canonical_string_no_body(
            "GET", "/api/v1/foo?a=1", "1700000000", "n1",
        );
        let c2 = RequestSigner::canonical_string_no_body(
            "GET", "/api/v1/foo?a=2", "1700000000", "n1",
        );
        // Same canonical → same signature (query string is not signed).
        assert_eq!(c1, c2);
    }

    #[test]
    fn parse_unix_timestamp_ok() {
        let ts = parse_timestamp("1700000000").unwrap();
        assert_eq!(ts.timestamp(), 1700000000);
    }

    #[test]
    fn parse_invalid_timestamp_errors() {
        assert!(parse_timestamp("not-a-timestamp").is_err());
        assert!(parse_timestamp("").is_err());
    }

    // ── strict-mode canonical string ──────────────────────────────────────────

    #[test]
    fn strict_canonical_has_six_fields() {
        let body_hash = RequestSigner::sha256_hex(b"{}");
        let c = RequestSigner::canonical_string_strict(
            "POST", "/api/v1/requisitions", "1700000000", "nonce1", &body_hash,
        );
        let parts: Vec<&str> = c.split('\n').collect();
        assert_eq!(parts.len(), 6, "strict canonical must have 6 newline-separated fields");
    }

    #[test]
    fn strict_canonical_splits_path_and_query() {
        let body_hash = RequestSigner::sha256_hex(b"");
        let c = RequestSigner::canonical_string_strict(
            "GET",
            "/api/v1/anomaly/events?unacked=true",
            "1700000000",
            "n99",
            &body_hash,
        );
        let parts: Vec<&str> = c.split('\n').collect();
        assert_eq!(parts[1], "/api/v1/anomaly/events", "path must not contain query");
        assert_eq!(parts[4], "unacked=true", "query field must be raw query string");
    }

    #[test]
    fn strict_canonical_no_query_has_empty_query_field() {
        let body_hash = RequestSigner::sha256_hex(b"");
        let c = RequestSigner::canonical_string_strict(
            "DELETE", "/api/v1/thing/1", "1700000000", "n1", &body_hash,
        );
        let parts: Vec<&str> = c.split('\n').collect();
        assert_eq!(parts[4], "", "query field must be empty when URL has no query string");
    }

    #[test]
    fn strict_canonical_different_bodies_produce_different_canonicals() {
        let hash_a = RequestSigner::sha256_hex(b"{\"key\":\"a\"}");
        let hash_b = RequestSigner::sha256_hex(b"{\"key\":\"b\"}");
        let ca = RequestSigner::canonical_string_strict(
            "POST", "/api/v1/thing", "1700000000", "n1", &hash_a,
        );
        let cb = RequestSigner::canonical_string_strict(
            "POST", "/api/v1/thing", "1700000000", "n1", &hash_b,
        );
        assert_ne!(ca, cb, "different body hashes must produce different strict canonicals");
    }

    #[test]
    fn strict_and_compat_canonicals_differ() {
        // This confirms that strict mode is NOT backward-compatible — the two
        // canonical forms are distinct, so a client must match the server's mode.
        let body_hash = RequestSigner::sha256_hex(b"");
        let compat = RequestSigner::canonical_string_no_body("GET", "/api/v1/x", "123", "n");
        let strict = RequestSigner::canonical_string_strict("GET", "/api/v1/x", "123", "n", &body_hash);
        assert_ne!(compat, strict);
    }

    #[test]
    fn sha256_hex_empty_body_is_known_value() {
        // SHA-256 of empty bytes is a well-known constant — clients should send
        // this for GET/DELETE requests in strict mode.
        assert_eq!(
            RequestSigner::sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
    }
}
