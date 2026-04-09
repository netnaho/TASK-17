use gloo::net::http::Request;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::state::Principal;
use crate::auth::{use_auth, AuthAction};
use crate::components::card::Card;
use crate::router::Route;

#[derive(Serialize)]
struct LoginInput<'a> { email: &'a str, password: &'a str }

#[derive(Deserialize)]
struct LoginResponse {
    user_id: String,
    email: String,
    roles: Vec<String>,
    api_token: String,
    signing_key: String,
}

#[function_component(LoginPage)]
pub fn login() -> Html {
    let auth = use_auth();
    let nav = use_navigator().unwrap();
    let email = use_state(|| String::new());
    let password = use_state(|| String::new());
    let error = use_state(|| Option::<String>::None);
    let pending = use_state(|| false);

    if auth.is_authenticated() {
        nav.push(&Route::Dashboard);
    }

    let on_email = {
        let email = email.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            email.set(input.value());
        })
    };
    let on_password = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            password.set(input.value());
        })
    };

    let on_submit = {
        let email = email.clone();
        let password = password.clone();
        let error = error.clone();
        let pending = pending.clone();
        let auth = auth.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let body = LoginInput { email: &email, password: &password };
            let body = serde_json::to_string(&body).unwrap();
            let error = error.clone();
            let pending = pending.clone();
            let auth = auth.clone();
            pending.set(true);
            error.set(None);
            spawn_local(async move {
                let res = Request::post("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(body)
                    .expect("body")
                    .send()
                    .await;
                pending.set(false);
                match res {
                    Ok(r) if r.ok() => match r.json::<LoginResponse>().await {
                        Ok(payload) => {
                            let principal = Principal {
                                user_id: payload.user_id,
                                email: payload.email,
                                roles: payload.roles,
                                permissions: HashSet::new(),
                            };
                            auth.dispatch(AuthAction::SignedIn {
                                principal,
                                api_token: payload.api_token,
                                signing_key: payload.signing_key,
                            });
                        }
                        Err(e) => error.set(Some(format!("decode error: {e}"))),
                    },
                    Ok(r) => error.set(Some(format!("sign-in failed ({})", r.status()))),
                    Err(e) => error.set(Some(e.to_string())),
                }
            });
        })
    };

    html! {
        <div class="auth-shell">
            <div class="auth-card">
                <Card title={AttrValue::from("SilverOak Operations Suite")}>
                    <p class="muted">{"Sign in with your staff credentials."}</p>
                    <form onsubmit={on_submit}>
                        <div class="form-field">
                            <label for="email">{"Email"}</label>
                            <input id="email" type="email" required=true value={(*email).clone()} oninput={on_email} />
                        </div>
                        <div class="form-field">
                            <label for="password">{"Password"}</label>
                            <input id="password" type="password" required=true minlength="12"
                                value={(*password).clone()} oninput={on_password} />
                        </div>
                        if let Some(err) = (*error).clone() {
                            <div class="state error">{ err }</div>
                        }
                        <button class="button" type="submit" disabled={*pending}>
                            { if *pending { "Signing in…" } else { "Sign in" } }
                        </button>
                    </form>
                </Card>
                <p class="muted" style="margin-top:16px;text-align:center;">
                    {"Demo credentials are documented in the project README."}
                </p>
            </div>
        </div>
    }
}
