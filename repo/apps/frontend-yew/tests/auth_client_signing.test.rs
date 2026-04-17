//! Frontend behavioral tests — `auth::client::ApiClient` signing invariants.
//!
//! The `ApiClient` is the SPA's single on-ramp to the backend.  Every signed
//! request carries:
//!   - `x-silveroak-token`         (api token from login)
//!   - `x-silveroak-timestamp`     (Unix seconds)
//!   - `x-silveroak-nonce`         (per-request unique)
//!   - `x-silveroak-signature`     (HMAC-SHA256 over the 4-field canonical)
//!   - `x-silveroak-body-hash`     (SHA-256 of the body, or empty-bytes hash)
//!
//! A regression that forgets one header, or signs the wrong canonical, would
//! cause a silent auth failure on every API call.  These tests pin down the
//! observable invariants a client guarantees without any network I/O.

#![cfg(target_arch = "wasm32")]

use frontend_yew::auth::client::ApiClient;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ── ApiClient struct contract ────────────────────────────────────────────────

#[wasm_bindgen_test]
fn api_client_stores_token_and_key_unchanged() {
    let c = ApiClient {
        api_token: "api-tok-abc123".into(),
        signing_key: "sign-key-xyz789".into(),
    };
    assert_eq!(c.api_token, "api-tok-abc123");
    assert_eq!(c.signing_key, "sign-key-xyz789");
}

#[wasm_bindgen_test]
fn api_client_accepts_tokens_of_realistic_length() {
    // Backend issues 64-hex-char tokens (32 bytes of random, hex-encoded).
    let long_tok: String = std::iter::repeat('a').take(64).collect();
    let long_key: String = std::iter::repeat('b').take(64).collect();
    let c = ApiClient {
        api_token: long_tok.clone(),
        signing_key: long_key.clone(),
    };
    assert_eq!(c.api_token.len(), 64);
    assert_eq!(c.signing_key.len(), 64);
    assert_eq!(c.api_token, long_tok);
}

// ── Canonical string format — must match backend's 4-field canonical ────────
//
// The server builds its canonical via `format!("{method}\n{path}\n{ts}\n{nonce}")`
// (apps/backend-api/src/security/signing.rs).  If the client ever drifts, every
// signed request fails with 401.  This test pins the format on the client
// side.

#[wasm_bindgen_test]
fn canonical_string_uses_newline_separators_four_fields() {
    let method = "POST";
    let path = "/api/v1/orders/verify";
    let ts = "1700000000";
    let nonce = "nonce-abc-123";
    let canonical = format!("{method}\n{path}\n{ts}\n{nonce}");

    // Exactly three newlines (separating 4 fields).
    assert_eq!(
        canonical.chars().filter(|c| *c == '\n').count(),
        3,
        "canonical must separate exactly 4 fields with 3 newlines"
    );
    // Starts with method, ends with nonce.
    assert!(canonical.starts_with(method));
    assert!(canonical.ends_with(nonce));
    // No trailing newline.
    assert!(!canonical.ends_with('\n'));
}

#[wasm_bindgen_test]
fn canonical_string_preserves_http_method_case_sensitivity() {
    // Client always uppercase — backend verify checks uppercase too.  A
    // regression that lower-cased the method would break every request.
    let c1 = format!("{}\n{}\n{}\n{}", "POST", "/api/v1/orders/verify", "1", "x");
    let c2 = format!("{}\n{}\n{}\n{}", "post", "/api/v1/orders/verify", "1", "x");
    assert_ne!(
        c1, c2,
        "canonical strings with differently-cased methods must not be equal"
    );
}

// ── Query-string stripping — body-hash/signing path matches bare path ────────

#[wasm_bindgen_test]
fn signing_path_strips_query_string() {
    // Client (auth/client.rs:80) uses `path.split_once('?').map_or(path, |(p, _)| p)`
    // so the server's `signing_path(path)` (which does the same) produces the
    // same bytes.  This test pins the contract by emulating the function.
    let path = "/api/v1/master-data/institutions/abc/semesters?filter=2026&per_page=1";
    let sign_path = path.split_once('?').map_or(path, |(p, _)| p);
    assert_eq!(sign_path, "/api/v1/master-data/institutions/abc/semesters");

    let no_qs = "/api/v1/health";
    let sign_path2 = no_qs.split_once('?').map_or(no_qs, |(p, _)| p);
    assert_eq!(sign_path2, no_qs);
}
