use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::{card::Card, states::{EmptyState, ErrorState, LoadingState}};
use crate::router::Route;

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct ReqDetail {
    pub id: String,
    pub ref_code: String,
    pub status: String,
    pub total_amount_cents: i64,
    pub contains_controlled: bool,
    pub needed_by: String,
}

#[function_component(RequisitionsPage)]
pub fn requisitions() -> Html {
    let auth = use_auth();
    let nav = use_navigator().unwrap();
    let items = use_state(|| Option::<Result<Vec<ReqDetail>, String>>::None);

    {
        let items = items.clone();
        let auth = auth.clone();
        use_effect_with((), move |_| {
            let token = auth.api_token.clone();
            let key = auth.signing_key.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.get("/api/v1/requisitions/mine/list").await {
                        Ok(r) if r.ok() => match r.json::<Vec<ReqDetail>>().await {
                            Ok(v) => items.set(Some(Ok(v))),
                            Err(e) => items.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => items.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => items.set(Some(Err(e.to_string()))),
                    }
                }
            });
            || ()
        });
    }

    let on_new = {
        let nav = nav.clone();
        Callback::from(move |_| nav.push(&Route::RequisitionNew))
    };

    let body = match (*items).clone() {
        None => html! { <LoadingState title={AttrValue::from("Loading requisitions…")} detail={Option::<AttrValue>::None}/> },
        Some(Err(e)) => html! { <ErrorState title={AttrValue::from("Failed to load")} detail={Some(AttrValue::from(e))}/> },
        Some(Ok(rows)) if rows.is_empty() => html! {
            <EmptyState title={AttrValue::from("No requisitions yet")}
                detail={Some(AttrValue::from("Create your first requisition to begin."))}/>
        },
        Some(Ok(rows)) => html! {
            <table class="table">
                <thead><tr><th>{"Ref"}</th><th>{"Status"}</th><th>{"Needed by"}</th><th>{"Total"}</th><th>{"Controlled"}</th></tr></thead>
                <tbody>
                { for rows.into_iter().map(|r| html!{
                    <tr>
                        <td><Link<Route> to={Route::RequisitionDetail { id: r.id.clone() }}>{ r.ref_code }</Link<Route>></td>
                        <td>{ r.status }</td>
                        <td>{ r.needed_by }</td>
                        <td>{ format!("${:.2}", r.total_amount_cents as f64 / 100.0) }</td>
                        <td>{ if r.contains_controlled { "yes" } else { "no" } }</td>
                    </tr>
                })}
                </tbody>
            </table>
        },
    };

    html! {
        <>
            <div class="page-header">
                <h1>{"Requisitions"}</h1>
                <p>{"Create, track, and manage requisitions for your department."}</p>
            </div>
            <Card>
                <div style="display:flex;justify-content:flex-end;margin-bottom:16px;">
                    <button class="button" onclick={on_new}>{"+ New requisition"}</button>
                </div>
                { body }
            </Card>
        </>
    }
}
