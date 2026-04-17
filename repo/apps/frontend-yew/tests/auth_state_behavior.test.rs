//! Frontend behavioral tests — `auth::state` reducer and permission predicates.
//!
//! Goes beyond smoke-level render tests: exercises the actual state
//! transitions the SPA relies on for login/logout, role-gated nav, and
//! `has_permission` checks.  Every assertion inspects state fields or
//! observable behaviour — no "just renders without panicking" checks.
//!
//! Execution: `wasm-pack test --node apps/frontend-yew` inside the pre-baked
//! tests-runner container (see `Dockerfile.tests`).  Gated to
//! `target_arch = "wasm32"` so host `cargo check` skips this file entirely.

#![cfg(target_arch = "wasm32")]

use std::collections::HashSet;
use std::rc::Rc;

use frontend_yew::auth::state::{AuthAction, AuthState, Principal};
use wasm_bindgen_test::*;
use yew::Reducible;

wasm_bindgen_test_configure!(run_in_browser);

// ── helpers ──────────────────────────────────────────────────────────────────

fn principal_with(perms: &[&str], roles: &[&str]) -> Principal {
    Principal {
        user_id: "00000000-0000-0000-0000-000000000001".into(),
        email: "test@silveroak.local".into(),
        roles: roles.iter().map(|s| (*s).into()).collect(),
        permissions: perms.iter().map(|s| (*s).into()).collect::<HashSet<_>>(),
    }
}

fn signed_in(state: Rc<AuthState>, principal: Principal) -> Rc<AuthState> {
    state.reduce(AuthAction::SignedIn {
        principal,
        api_token: "tok-behavior-test".into(),
        signing_key: "key-behavior-test".into(),
    })
}

// ── default state ────────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn default_state_is_not_authenticated_and_not_bootstrapped() {
    let s = AuthState::default();
    assert!(!s.is_authenticated(), "default state must not count as signed in");
    assert!(!s.bootstrapped, "default state must report not-yet-bootstrapped");
    assert!(s.principal.is_none());
    assert!(s.api_token.is_none());
    assert!(s.signing_key.is_none());
}

// ── SignedIn reducer ─────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn signed_in_populates_every_credential_field() {
    let state = Rc::new(AuthState::default());
    let next = signed_in(state, principal_with(&["scope:any", "*"], &["admin"]));

    assert!(next.is_authenticated(), "SignedIn must flip is_authenticated to true");
    assert!(next.bootstrapped, "SignedIn must mark bootstrapped");
    assert_eq!(next.api_token.as_deref(), Some("tok-behavior-test"));
    assert_eq!(next.signing_key.as_deref(), Some("key-behavior-test"));

    let p = next.principal.as_ref().expect("principal set after SignedIn");
    assert_eq!(p.email, "test@silveroak.local");
    assert!(p.roles.contains(&"admin".to_string()));
    assert!(p.permissions.contains("scope:any"));
}

// ── SignedOut reducer ────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn signed_out_clears_principal_and_tokens_but_keeps_bootstrapped() {
    let state = Rc::new(AuthState::default());
    let signed = signed_in(state, principal_with(&["orders:read"], &["medical"]));
    assert!(signed.is_authenticated());

    let after = signed.reduce(AuthAction::SignedOut);

    assert!(!after.is_authenticated(), "SignedOut must clear authentication");
    assert!(after.principal.is_none(), "principal must be cleared");
    assert!(after.api_token.is_none(), "api_token must be cleared");
    assert!(after.signing_key.is_none(), "signing_key must be cleared");
    // bootstrapped stays `true` — the app is ready, just logged-out.
    assert!(after.bootstrapped, "bootstrapped must remain true after sign-out");
}

// ── has_permission predicate ─────────────────────────────────────────────────

#[wasm_bindgen_test]
fn has_permission_false_when_not_signed_in() {
    let s = AuthState::default();
    assert!(!s.has_permission("anything"), "anonymous users have no permissions");
    assert!(!s.has_permission("*"), "wildcard also denied when no principal");
}

#[wasm_bindgen_test]
fn has_permission_matches_explicit_entries() {
    let state = Rc::new(AuthState::default());
    let s = signed_in(state, principal_with(&["orders:read", "requisitions:write"], &["medical"]));

    assert!(s.has_permission("orders:read"));
    assert!(s.has_permission("requisitions:write"));
    assert!(!s.has_permission("admin:users"), "explicit permissions must not include admin");
    assert!(!s.has_permission("scope:any"));
}

#[wasm_bindgen_test]
fn wildcard_permission_grants_everything() {
    let state = Rc::new(AuthState::default());
    let s = signed_in(state, principal_with(&["*"], &["admin"]));

    assert!(s.has_permission("admin:users"));
    assert!(s.has_permission("admin:blacklist"));
    assert!(s.has_permission("totally:made-up:name"));
}

// ── role-based UI gating behavior ────────────────────────────────────────────
//
// The app shell reads `AuthState::principal.roles` to decide which nav items
// to render.  These tests capture that contract from the state layer so a
// regression (e.g. roles getting clobbered on re-bootstrap) fails here before
// it ships to users.

#[wasm_bindgen_test]
fn roles_round_trip_through_signed_in_action() {
    let state = Rc::new(AuthState::default());
    let s = signed_in(
        state,
        principal_with(&[], &["medical", "dept_approver"]),
    );
    let roles = &s.principal.as_ref().unwrap().roles;
    assert_eq!(roles.len(), 2);
    assert!(roles.contains(&"medical".to_string()));
    assert!(roles.contains(&"dept_approver".to_string()));
}

#[wasm_bindgen_test]
fn role_based_nav_decision_senior_vs_admin() {
    // Simulate the sidebar predicate used in `layout::Shell`:
    //   show_admin_menu = principal.roles.contains("admin")
    let senior = signed_in(
        Rc::new(AuthState::default()),
        principal_with(&["orders:read"], &["senior"]),
    );
    let admin = signed_in(
        Rc::new(AuthState::default()),
        principal_with(&["*", "scope:any"], &["admin"]),
    );

    let has_admin = |s: &AuthState| s.principal.as_ref()
        .map(|p| p.roles.iter().any(|r| r == "admin"))
        .unwrap_or(false);

    assert!(!has_admin(&senior), "senior must not trigger admin nav");
    assert!(has_admin(&admin), "admin role must trigger admin nav");
}

// ── reducer purity ───────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn reducer_returns_fresh_state_and_does_not_mutate_input() {
    let original = Rc::new(AuthState::default());
    let before_ptr = Rc::as_ptr(&original);
    let after = signed_in(original.clone(), principal_with(&[], &["family"]));

    // The reducer must produce a NEW Rc, not mutate the input.
    assert_ne!(before_ptr, Rc::as_ptr(&after), "reducer must return a fresh Rc");
    // Original stays default.
    assert!(!original.is_authenticated());
    // New state is signed in.
    assert!(after.is_authenticated());
}
