//! Security constants pulled from the Task Prompt. Centralized so a
//! reviewer can see them in one place and so tests can reference them.

use std::time::Duration;

/// Minimum password length required by policy.
pub const MIN_PASSWORD_LEN: usize = 12;

/// Number of consecutive failed logins before lockout.
pub const MAX_FAILED_LOGINS: i64 = 5;

/// Lockout duration after the threshold is hit.
pub const LOCKOUT: Duration = Duration::from_secs(15 * 60);

/// API token lifetime.
pub const API_TOKEN_TTL: Duration = Duration::from_secs(15 * 60);

/// Allowed clock skew for signed-request timestamps.
pub const REPLAY_WINDOW: Duration = Duration::from_secs(60);

/// Per-IP rate limit (sliding window of 60 seconds).
/// Overridable via RATE_LIMIT_PER_MIN env var for test environments.
pub fn rate_limit_per_min() -> u32 {
    std::env::var("RATE_LIMIT_PER_MIN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120)
}

/// Session cookie name.
pub const SESSION_COOKIE: &str = "silveroak_session";

/// Header carrying the short-lived API token.
pub const HDR_API_TOKEN: &str = "x-silveroak-token";
/// Header carrying the request timestamp (RFC3339 or unix seconds).
pub const HDR_TIMESTAMP: &str = "x-silveroak-timestamp";
/// Header carrying the HMAC signature (hex).
pub const HDR_SIGNATURE: &str = "x-silveroak-signature";
/// Header carrying the per-request nonce.
pub const HDR_NONCE: &str = "x-silveroak-nonce";
/// Header carrying the hex-encoded SHA-256 hash of the request body.
/// Required by clients when the server operates in strict-signing mode
/// (`APP__STRICT_SIGNING=true`).  For bodyless requests (GET/DELETE) the
/// client must send the SHA-256 of an empty byte sequence:
/// `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
pub const HDR_BODY_HASH: &str = "x-silveroak-body-hash";
