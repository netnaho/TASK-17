use gloo::net::http::Request;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use super::nav::SideNav;
use crate::auth::{use_auth, AuthAction};
use crate::router::Route;

#[derive(Properties, PartialEq)]
pub struct ShellProps {
    pub current: Route,
    pub children: Children,
}

#[function_component(Shell)]
pub fn shell(props: &ShellProps) -> Html {
    let auth = use_auth();
    let email = auth.principal.as_ref().map(|p| p.email.clone()).unwrap_or_default();

    let on_logout = {
        let auth = auth.clone();
        Callback::from(move |_| {
            let auth = auth.clone();
            spawn_local(async move {
                let _ = Request::post("/api/v1/auth/logout").send().await;
                auth.dispatch(AuthAction::SignedOut);
            });
        })
    };

    html! {
        <div class="app-shell">
            <SideNav current={props.current.clone()} />
            <div class="main">
                <header class="topbar">
                    <h1>{ props.current.label() }</h1>
                    <div style="display:flex;align-items:center;gap:16px;">
                        <span class="muted">{ email }</span>
                        <button class="button secondary" onclick={on_logout}>{"Sign out"}</button>
                    </div>
                </header>
                <main class="content">
                    { for props.children.iter() }
                </main>
            </div>
        </div>
    }
}
