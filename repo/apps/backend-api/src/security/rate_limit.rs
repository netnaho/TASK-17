//! Pluggable per-IP sliding-window rate limiter.
//!
//! Two backends are available:
//!
//! * **Memory** (`APP__RATE_LIMIT_BACKEND=memory`) — process-local
//!   `HashMap<ip, VecDeque<Instant>>` protected by a `parking_lot::Mutex`.
//!   Fast and zero-dependency, but each API replica enforces its own window
//!   independently.  Suitable for single-container deployments (the project's
//!   offline-first brief) or for local development.
//!
//! * **Postgres** (`APP__RATE_LIMIT_BACKEND=postgres`) — timestamps are stored
//!   in the `rate_limit_log` table (migration 0008).  Every replica shares the
//!   same sliding window, making the limit globally correct under horizontal
//!   scaling.  Adds ~1 DB round-trip per request.  **This is the recommended
//!   production backend** and is set in the base `docker-compose.yml`.
//!
//! ## Postgres failure policy
//!
//! If the database is unreachable or the `rate_limit_log` table query fails:
//!
//! * The request is **allowed through** (fail-open) so that a transient DB
//!   hiccup never causes a self-inflicted denial-of-service.
//! * A structured `WARN` log is emitted with `rate_limit_db_error=true` so
//!   alerts can fire on sustained degradation.
//! * The `check()` return type is `RateLimitDecision` which callers log but
//!   never panic on.
//!
//! This policy is intentional: for a real-time operations platform a brief
//! window of un-metered traffic is less harmful than dropping all requests.
//! If a stricter policy is required, set `APP__RATE_LIMIT_BACKEND=memory` as
//! a fallback layer and layer a WAF in front of the proxy.
//!
//! ## Retry-after bounds
//!
//! `check()` always returns a `retry_after` in `[1, window_secs]` seconds.
//! This is deterministic and bounded — clients receive an accurate estimate
//! without needing to parse a `Date` header.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use parking_lot::Mutex;
use sqlx::PgPool;

use super::constants::rate_limit_per_min;

// ── in-memory backend ─────────────────────────────────────────────────────────

/// Process-local sliding-window limiter.  See module-level docs.
#[derive(Clone)]
pub struct InMemoryLimiter {
    inner: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
    window: Duration,
    limit: usize,
}

impl Default for InMemoryLimiter {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            window: Duration::from_secs(60),
            limit: rate_limit_per_min() as usize,
        }
    }
}

impl InMemoryLimiter {
    fn check(&self, ip: &str) -> RateLimitOutcome {
        let now = Instant::now();
        let mut map = self.inner.lock();
        let entry = map.entry(ip.to_string()).or_default();
        while let Some(front) = entry.front() {
            if now.duration_since(*front) > self.window {
                entry.pop_front();
            } else {
                break;
            }
        }
        if entry.len() >= self.limit {
            let oldest = *entry.front().expect("non-empty");
            let elapsed = now.duration_since(oldest);
            let retry = self
                .window
                .saturating_sub(elapsed)
                .as_secs()
                .clamp(1, self.window.as_secs());
            return RateLimitOutcome::Deny { retry_after_secs: retry };
        }
        entry.push_back(now);
        RateLimitOutcome::Allow
    }
}

// ── postgres backend ──────────────────────────────────────────────────────────

/// Distributed sliding-window limiter backed by the `rate_limit_log` table.
/// Requires migration 0008.  See module-level docs.
#[derive(Clone)]
pub struct PostgresLimiter {
    pool: PgPool,
    window_secs: i64,
    limit: i64,
}

impl PostgresLimiter {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            window_secs: 60,
            limit: rate_limit_per_min() as i64,
        }
    }

    async fn check(&self, ip: &str) -> RateLimitOutcome {
        // Step 1: Purge expired entries for this IP (best-effort).
        // On failure we continue — the count in step 2 may be slightly high,
        // which is safe (slightly more aggressive limiting).
        if let Err(e) = sqlx::query(
            "DELETE FROM rate_limit_log \
             WHERE ip = $1 AND ts < NOW() - ($2 * INTERVAL '1 second')",
        )
        .bind(ip)
        .bind(self.window_secs)
        .execute(&self.pool)
        .await
        {
            tracing::warn!(
                ip,
                rate_limit_db_error = true,
                err = %e,
                "rate_limit: purge failed; proceeding with potentially stale count"
            );
        }

        // Step 2: Count requests within the window.
        let count: i64 = match sqlx::query_scalar(
            "SELECT COUNT(*) FROM rate_limit_log WHERE ip = $1",
        )
        .bind(ip)
        .fetch_one(&self.pool)
        .await
        {
            Ok(n) => n,
            Err(e) => {
                // DB read failure → fail-open: allow the request.
                // See module-level docs for the policy rationale.
                tracing::warn!(
                    ip,
                    rate_limit_db_error = true,
                    err = %e,
                    "rate_limit: count query failed; allowing request (fail-open)"
                );
                return RateLimitOutcome::Allow;
            }
        };

        if count >= self.limit {
            // Estimate retry-after from the oldest entry in the window.
            let oldest_ts: Option<chrono::DateTime<Utc>> =
                sqlx::query_scalar("SELECT MIN(ts) FROM rate_limit_log WHERE ip = $1")
                    .bind(ip)
                    .fetch_optional(&self.pool)
                    .await
                    .ok()
                    .flatten();

            let retry = oldest_ts
                .map(|t| {
                    let window_end = t + chrono::Duration::seconds(self.window_secs);
                    let secs = (window_end - Utc::now()).num_seconds();
                    secs.clamp(1, self.window_secs) as u64
                })
                .unwrap_or(1);

            return RateLimitOutcome::Deny { retry_after_secs: retry };
        }

        // Step 3: Record this request (best-effort).
        // A failed insert means we under-count by one, which is acceptable.
        if let Err(e) = sqlx::query("INSERT INTO rate_limit_log (ip) VALUES ($1)")
            .bind(ip)
            .execute(&self.pool)
            .await
        {
            tracing::warn!(
                ip,
                rate_limit_db_error = true,
                err = %e,
                "rate_limit: insert failed; request allowed but not counted"
            );
        }

        RateLimitOutcome::Allow
    }
}

// ── outcome type ──────────────────────────────────────────────────────────────

/// Decision returned by `RateLimiter::check`.  Using a dedicated type (rather
/// than `Result<(), u64>`) makes the call-site intent explicit and allows
/// additional variants in future (e.g., `Degraded`) without breaking callers.
#[derive(Debug, PartialEq, Eq)]
pub enum RateLimitOutcome {
    Allow,
    Deny { retry_after_secs: u64 },
}

// ── unified enum ─────────────────────────────────────────────────────────────

/// Active rate-limiter backend.  Constructed once at startup from
/// `AppConfig::rate_limit_backend` and passed to `IpGuard`.
#[derive(Clone)]
pub enum RateLimiter {
    Memory(InMemoryLimiter),
    Postgres(PostgresLimiter),
}

impl Default for RateLimiter {
    /// Defaults to the in-memory backend (used when no pool is available,
    /// e.g. in tests).  Production should use `from_config` which selects
    /// `Postgres` when configured.
    fn default() -> Self {
        Self::Memory(InMemoryLimiter::default())
    }
}

impl RateLimiter {
    /// Construct the appropriate backend from a configuration string.
    /// Accepts `"postgres"` (case-insensitive) for the distributed backend;
    /// any other value uses the in-memory backend.
    pub fn from_config(backend: &str, pool: Option<PgPool>) -> Self {
        if backend.trim().eq_ignore_ascii_case("postgres") {
            if let Some(p) = pool {
                tracing::info!(rate_limit_backend = "postgres", "rate limiter initialized");
                return Self::Postgres(PostgresLimiter::new(p));
            }
            tracing::warn!(
                rate_limit_backend = "postgres",
                "rate_limit_backend=postgres requested but no pool supplied; \
                 falling back to memory — multi-replica rate limiting will NOT be shared"
            );
        }
        tracing::info!(rate_limit_backend = "memory", "rate limiter initialized");
        Self::Memory(InMemoryLimiter::default())
    }

    /// Check whether `ip` should be allowed.  Returns [`RateLimitOutcome`].
    /// Never panics, even on DB failure (see module-level docs).
    pub async fn check(&self, ip: &str) -> RateLimitOutcome {
        match self {
            Self::Memory(m) => m.check(ip),
            Self::Postgres(p) => p.check(ip).await,
        }
    }

    /// Human-readable backend label for structured logs.
    pub fn backend_label(&self) -> &'static str {
        match self {
            Self::Memory(_) => "memory",
            Self::Postgres(_) => "postgres",
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> RateLimiter {
        RateLimiter::Memory(InMemoryLimiter::default())
    }

    // ── basic allow/deny ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn allows_up_to_limit_then_denies() {
        let rl = memory();
        for _ in 0..rate_limit_per_min() {
            assert_eq!(rl.check("1.2.3.4").await, RateLimitOutcome::Allow);
        }
        assert!(matches!(rl.check("1.2.3.4").await, RateLimitOutcome::Deny { .. }));
        // different IP is unaffected
        assert_eq!(rl.check("9.9.9.9").await, RateLimitOutcome::Allow);
    }

    #[tokio::test]
    async fn multiple_ips_are_independent() {
        let rl = memory();
        for _ in 0..rate_limit_per_min() {
            rl.check("10.0.0.1").await;
        }
        assert!(matches!(rl.check("10.0.0.1").await, RateLimitOutcome::Deny { .. }));
        assert_eq!(rl.check("10.0.0.2").await, RateLimitOutcome::Allow);
    }

    #[tokio::test]
    async fn fresh_ip_is_allowed() {
        let rl = memory();
        assert_eq!(rl.check("brand-new-ip").await, RateLimitOutcome::Allow);
    }

    // ── retry-after bounds ────────────────────────────────────────────────────

    #[tokio::test]
    async fn retry_after_is_at_least_one() {
        let rl = memory();
        for _ in 0..rate_limit_per_min() {
            rl.check("ip-x").await;
        }
        match rl.check("ip-x").await {
            RateLimitOutcome::Deny { retry_after_secs } => {
                assert!(retry_after_secs >= 1, "retry-after must be ≥ 1");
            }
            _ => panic!("expected Deny"),
        }
    }

    #[tokio::test]
    async fn retry_after_is_at_most_window() {
        let rl = memory();
        for _ in 0..rate_limit_per_min() {
            rl.check("ip-y").await;
        }
        match rl.check("ip-y").await {
            RateLimitOutcome::Deny { retry_after_secs } => {
                assert!(
                    retry_after_secs <= 60,
                    "retry-after must be ≤ window (60s), got {retry_after_secs}"
                );
            }
            _ => panic!("expected Deny"),
        }
    }

    // ── from_config ───────────────────────────────────────────────────────────

    #[test]
    fn from_config_memory_without_pool() {
        let rl = RateLimiter::from_config("memory", None);
        assert!(matches!(rl, RateLimiter::Memory(_)));
        assert_eq!(rl.backend_label(), "memory");
    }

    #[test]
    fn from_config_postgres_without_pool_falls_back_to_memory() {
        let rl = RateLimiter::from_config("postgres", None);
        assert!(
            matches!(rl, RateLimiter::Memory(_)),
            "must fall back to memory when postgres is requested but pool is absent"
        );
    }

    #[test]
    fn from_config_unknown_backend_yields_memory() {
        let rl = RateLimiter::from_config("redis", None);
        assert!(matches!(rl, RateLimiter::Memory(_)));
    }

    #[test]
    fn from_config_case_insensitive() {
        let rl = RateLimiter::from_config("MEMORY", None);
        assert!(matches!(rl, RateLimiter::Memory(_)));
    }

    // ── backend label ─────────────────────────────────────────────────────────

    #[test]
    fn backend_label_memory() {
        assert_eq!(RateLimiter::default().backend_label(), "memory");
    }

    // ── outcome type ──────────────────────────────────────────────────────────

    #[test]
    fn allow_and_deny_are_not_equal() {
        assert_ne!(
            RateLimitOutcome::Allow,
            RateLimitOutcome::Deny { retry_after_secs: 5 }
        );
    }

    #[test]
    fn deny_retry_after_preserved() {
        let outcome = RateLimitOutcome::Deny { retry_after_secs: 42 };
        match outcome {
            RateLimitOutcome::Deny { retry_after_secs } => assert_eq!(retry_after_secs, 42),
            _ => panic!("expected Deny"),
        }
    }
}
