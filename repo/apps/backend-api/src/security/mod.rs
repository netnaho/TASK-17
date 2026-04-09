//! Centralized security primitives for SilverOak.
//!
//! Reviewer map:
//!   password.rs   — Argon2 hashing + 12-char policy
//!   sessions.rs   — server-tracked session cookies
//!   tokens.rs     — short-lived (15 min) signed API tokens
//!   signing.rs    — HMAC request signature + ±60s timestamp + replay store
//!   rate_limit.rs — per-IP 120 req/min sliding window
//!   blacklist.rs  — DB-backed IP blacklist
//!   rbac.rs       — permissions & data-scope guards
//!   middleware.rs — actix middlewares wiring it all together
//!
//! All authenticated routes MUST go through `middleware::AuthScope` so the
//! checks above are statically traceable from `routes::configure`.

pub mod blacklist;
pub mod constants;
pub mod middleware;
pub mod password;
pub mod rate_limit;
pub mod rbac;
pub mod sessions;
pub mod signing;
pub mod signing_mode;
pub mod tokens;
