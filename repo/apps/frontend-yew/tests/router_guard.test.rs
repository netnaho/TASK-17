//! Frontend behavioral tests — router guard decisions.
//!
//! The app's `Guard` component in `apps/frontend-yew/src/app.rs` makes three
//! decisions before rendering any page:
//!
//!   1. If auth has not yet bootstrapped → show a Loading state.
//!   2. Else if the route is `Login` → render the login page.
//!   3. Else if `!auth.is_authenticated()` → redirect to Login.
//!   4. Else → render the shell around the requested page.
//!
//! Any regression in this decision order lets an unauthenticated user see a
//! page momentarily, or traps an already-authenticated user on the login
//! page.  These tests reproduce the predicate in isolation and pin down the
//! expected decision for every branch.

#![cfg(target_arch = "wasm32")]

use std::collections::HashSet;
use std::rc::Rc;

use frontend_yew::auth::state::{AuthAction, AuthState, Principal};
use frontend_yew::router::Route;
use wasm_bindgen_test::*;
use yew::Reducible;

wasm_bindgen_test_configure!(run_in_browser);

/// Decision enum mirrors `Guard` branches in `app.rs` so we can assert
/// a specific outcome per (auth_state, route) pair.
#[derive(Debug, PartialEq, Eq)]
enum GuardDecision {
    Loading,
    ShowLogin,
    RedirectToLogin,
    Render(Route),
}

fn decide(auth: &AuthState, route: Route) -> GuardDecision {
    if !auth.bootstrapped {
        return GuardDecision::Loading;
    }
    if matches!(route, Route::Login) {
        return GuardDecision::ShowLogin;
    }
    if !auth.is_authenticated() {
        return GuardDecision::RedirectToLogin;
    }
    GuardDecision::Render(route)
}

fn authed(roles: &[&str]) -> Rc<AuthState> {
    let principal = Principal {
        user_id: "00000000-0000-0000-0000-000000000002".into(),
        email: "guard@silveroak.local".into(),
        roles: roles.iter().map(|s| (*s).into()).collect(),
        permissions: HashSet::new(),
    };
    Rc::new(AuthState::default()).reduce(AuthAction::SignedIn {
        principal,
        api_token: "tok".into(),
        signing_key: "key".into(),
    })
}

// ── branch 1: not bootstrapped ───────────────────────────────────────────────

#[wasm_bindgen_test]
fn unbootstrapped_returns_loading_even_for_login_route() {
    let state = AuthState::default();
    assert_eq!(decide(&state, Route::Login), GuardDecision::Loading);
    assert_eq!(decide(&state, Route::Dashboard), GuardDecision::Loading);
    assert_eq!(
        decide(&state, Route::RequisitionDetail { id: "42".into() }),
        GuardDecision::Loading,
        "loading state must win over route-level decisions"
    );
}

// ── branch 2: bootstrapped + login route ────────────────────────────────────

#[wasm_bindgen_test]
fn bootstrapped_anonymous_on_login_route_shows_login() {
    let mut state = AuthState::default();
    state.bootstrapped = true;
    assert_eq!(decide(&state, Route::Login), GuardDecision::ShowLogin);
}

#[wasm_bindgen_test]
fn bootstrapped_authenticated_on_login_route_still_shows_login() {
    // Contract: a signed-in user who navigates to /login sees the login page
    // (they can sign out from there).  No automatic redirect to Dashboard.
    let state = authed(&["admin"]);
    assert_eq!(decide(&state, Route::Login), GuardDecision::ShowLogin);
}

// ── branch 3: bootstrapped + anonymous + non-login route → redirect ──────────

#[wasm_bindgen_test]
fn bootstrapped_anonymous_non_login_redirects_to_login() {
    let mut state = AuthState::default();
    state.bootstrapped = true;

    for route in [
        Route::Dashboard,
        Route::Requisitions,
        Route::RequisitionNew,
        Route::Approvals,
        Route::Inventory,
        Route::Orders,
        Route::Cart,
        Route::Checkout,
        Route::MasterData,
        Route::Analytics,
        Route::FamilyPortal,
        Route::Moderation,
        Route::SecurityEvents,
        Route::Settings,
        Route::NotFound,
    ] {
        assert_eq!(
            decide(&state, route.clone()),
            GuardDecision::RedirectToLogin,
            "anonymous user on {route:?} must be redirected to login"
        );
    }
}

#[wasm_bindgen_test]
fn bootstrapped_anonymous_on_dynamic_routes_also_redirects() {
    let mut state = AuthState::default();
    state.bootstrapped = true;
    assert_eq!(
        decide(&state, Route::RequisitionDetail { id: "abc".into() }),
        GuardDecision::RedirectToLogin
    );
    assert_eq!(
        decide(&state, Route::OrderDetail { id: "ord-1".into() }),
        GuardDecision::RedirectToLogin
    );
}

// ── branch 4: bootstrapped + authed + non-login → render ─────────────────────

#[wasm_bindgen_test]
fn authed_user_renders_requested_route() {
    let state = authed(&["admin"]);
    assert_eq!(
        decide(&state, Route::Dashboard),
        GuardDecision::Render(Route::Dashboard)
    );
    assert_eq!(
        decide(&state, Route::Approvals),
        GuardDecision::Render(Route::Approvals)
    );
}

#[wasm_bindgen_test]
fn authed_user_parameterised_route_preserves_id() {
    let state = authed(&["medical"]);
    let decision = decide(&state, Route::RequisitionDetail { id: "req-7".into() });
    match decision {
        GuardDecision::Render(Route::RequisitionDetail { id }) => {
            assert_eq!(id, "req-7", "path parameter must be preserved through the guard");
        }
        other => panic!("expected Render(RequisitionDetail), got {other:?}"),
    }
}

// ── signed-out is treated exactly like never-signed-in ───────────────────────

#[wasm_bindgen_test]
fn signed_out_state_redirects_like_anonymous() {
    // A user who just signed out must be indistinguishable (for the guard)
    // from a fresh-session anonymous user: bootstrapped=true, principal=None.
    let after = authed(&["admin"]).reduce(AuthAction::SignedOut);
    assert!(after.bootstrapped);
    assert!(!after.is_authenticated());
    assert_eq!(
        decide(&after, Route::Dashboard),
        GuardDecision::RedirectToLogin
    );
}
