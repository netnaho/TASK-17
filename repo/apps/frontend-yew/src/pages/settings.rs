/// Settings page — user profile, session information, and system configuration.
///
/// Shows the currently authenticated user's profile from local auth state.
/// Provides a logout action and displays effective roles and permissions.
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::{use_auth, ApiClient, AuthAction};
use crate::components::card::Card;
use crate::router::Route;

#[function_component(SettingsPage)]
pub fn settings() -> Html {
    let auth = use_auth();
    let nav = use_navigator().unwrap();
    let logout_msg = use_state(|| Option::<String>::None);
    let loading = use_state(|| false);

    let on_logout = {
        let auth = auth.clone();
        let nav = nav.clone();
        let logout_msg = logout_msg.clone();
        let loading = loading.clone();
        Callback::from(move |_: MouseEvent| {
            let auth = auth.clone();
            let nav = nav.clone();
            let logout_msg = logout_msg.clone();
            let loading = loading.clone();
            loading.set(true);
            spawn_local(async move {
                if let (Some(token), Some(key)) =
                    (auth.api_token.clone(), auth.signing_key.clone())
                {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.post_json("/api/v1/auth/logout", &serde_json::json!({})).await {
                        Ok(_) => {
                            auth.dispatch(AuthAction::SignedOut);
                            nav.push(&Route::Login);
                        }
                        Err(e) => {
                            logout_msg.set(Some(format!("Logout failed: {e}")));
                            loading.set(false);
                        }
                    }
                } else {
                    auth.dispatch(AuthAction::SignedOut);
                    nav.push(&Route::Login);
                }
            });
        })
    };

    let principal = auth.principal.as_ref();
    let email = principal.map(|p| p.email.as_str()).unwrap_or("—");
    let roles = principal.map(|p| p.roles.clone()).unwrap_or_default();
    let perms: Vec<String> = principal
        .map(|p| {
            let mut v: Vec<String> = p.permissions.iter().cloned().collect();
            v.sort();
            v
        })
        .unwrap_or_default();
    let is_admin = auth.has_permission("*");

    html! {
        <>
            <div class="page-header">
                <h1>{"Settings"}</h1>
                <p>{"User profile, session management, and system information."}</p>
            </div>

            // ── User profile ──────────────────────────────────────────────────
            <Card>
                <h2 style="margin:0 0 1rem;font-size:1.1rem;">{"Your Profile"}</h2>
                <table style="border-collapse:collapse;width:100%;max-width:500px;">
                    <tbody>
                        <tr>
                            <td style="padding:0.5rem 1rem 0.5rem 0;color:#6b7280;font-size:0.9rem;
                                        white-space:nowrap;">
                                {"Email"}
                            </td>
                            <td style="padding:0.5rem 0;font-size:0.9rem;">
                                <strong>{email}</strong>
                            </td>
                        </tr>
                        <tr>
                            <td style="padding:0.5rem 1rem 0.5rem 0;color:#6b7280;font-size:0.9rem;
                                        white-space:nowrap;vertical-align:top;">
                                {"Roles"}
                            </td>
                            <td style="padding:0.5rem 0;">
                                <div style="display:flex;gap:0.35rem;flex-wrap:wrap;">
                                    { for roles.iter().map(|r| html! {
                                        <span style="background:#dbeafe;color:#1e40af;\
                                            padding:0.15rem 0.5rem;border-radius:3px;\
                                            font-size:0.8rem;font-weight:600;">
                                            {r}
                                        </span>
                                    })}
                                    if roles.is_empty() {
                                        <span style="color:#9ca3af;font-size:0.9rem;">{"None assigned"}</span>
                                    }
                                </div>
                            </td>
                        </tr>
                    </tbody>
                </table>
            </Card>

            // ── Effective permissions ─────────────────────────────────────────
            <Card>
                <h2 style="margin:0 0 1rem;font-size:1.1rem;">{"Effective Permissions"}</h2>
                if is_admin {
                    <p style="color:#059669;font-size:0.9rem;margin:0 0 0.75rem;">
                        {"✓ Administrator — all permissions granted via wildcard ("}
                        <code>{"*"}</code>
                        {")"}
                    </p>
                }
                if perms.is_empty() {
                    <p style="color:#9ca3af;font-size:0.9rem;">{"No explicit permissions."}</p>
                } else {
                    <div style="display:flex;flex-wrap:wrap;gap:0.35rem;">
                        { for perms.iter().map(|p| html! {
                            <code style="background:#f3f4f6;padding:0.15rem 0.4rem;\
                                font-size:0.78rem;border-radius:3px;color:#374151;">
                                {p}
                            </code>
                        })}
                    </div>
                }
            </Card>

            // ── Session management ────────────────────────────────────────────
            <Card>
                <h2 style="margin:0 0 1rem;font-size:1.1rem;">{"Session"}</h2>
                <p style="font-size:0.9rem;color:#6b7280;margin:0 0 1rem;">
                    {"API tokens have a 15-minute TTL and are re-issued on each login. \
                      Signing out invalidates your session server-side."}
                </p>
                if let Some(ref msg) = *logout_msg {
                    <div style="margin-bottom:0.75rem;padding:0.5rem 0.75rem;\
                                background:#fee2e2;border-radius:4px;font-size:0.9rem;\
                                color:#991b1b;">
                        {msg}
                    </div>
                }
                <button
                    class="button button--danger"
                    onclick={on_logout}
                    disabled={*loading}
                >
                    if *loading { {"Signing out…"} } else { {"Sign out"} }
                </button>
            </Card>

            // ── System information ────────────────────────────────────────────
            <Card>
                <h2 style="margin:0 0 1rem;font-size:1.1rem;">{"System Information"}</h2>
                <table style="border-collapse:collapse;font-size:0.88rem;width:100%;max-width:500px;">
                    <tbody>
                        { for [
                            ("Stack",       "Actix-web 4 · PostgreSQL 15 · Yew 0.21 (WASM)"),
                            ("Auth",        "Argon2 · HMAC-SHA256 signed requests · Replay protection"),
                            ("Encryption",  "AES-256-GCM field-level (wellness notes)"),
                            ("Worker",      "6 background jobs (analytics, bests, consistency, anomaly sweep)"),
                        ].iter().map(|(k, v)| html! {
                            <tr>
                                <td style="padding:0.4rem 1rem 0.4rem 0;color:#6b7280;
                                            white-space:nowrap;vertical-align:top;">{k}</td>
                                <td style="padding:0.4rem 0;color:#374151;">{v}</td>
                            </tr>
                        })}
                    </tbody>
                </table>
            </Card>

            // ── Account recovery note ─────────────────────────────────────────
            <Card>
                <h2 style="margin:0 0 0.5rem;font-size:1.1rem;">{"Account Recovery"}</h2>
                <p style="font-size:0.9rem;color:#6b7280;margin:0;">
                    {"Password recovery is admin-assisted. Submit a recovery request via \
                      your institution administrator. There is no automated email/SMS flow — \
                      this is intentional for offline-first, local-network deployments."}
                </p>
            </Card>
        </>
    }
}
