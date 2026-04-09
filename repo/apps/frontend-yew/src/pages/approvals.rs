use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::{card::Card, states::EmptyState};
use crate::router::Route;

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct InboxItem {
    id: String,
    ref_code: String,
    requester_email: String,
    needed_by: String,
    total_amount_cents: i64,
    contains_controlled: bool,
}

#[function_component(ApprovalsPage)]
pub fn approvals() -> Html {
    let auth = use_auth();
    let items = use_state(|| Option::<Result<Vec<InboxItem>, String>>::None);
    {
        let items = items.clone();
        let auth = auth.clone();
        use_effect_with((), move |_| {
            let token = auth.api_token.clone();
            let key = auth.signing_key.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.get("/api/v1/approvals/inbox").await {
                        Ok(r) if r.ok() => match r.json::<Vec<InboxItem>>().await {
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

    let body = match (*items).clone() {
        None => html! { <div class="state">{"Loading…"}</div> },
        Some(Err(e)) => html! { <div class="state error">{ e }</div> },
        Some(Ok(rows)) if rows.is_empty() => html! {
            <EmptyState title={AttrValue::from("Nothing waiting on you")} detail={Option::<AttrValue>::None}/>
        },
        Some(Ok(rows)) => html! {
            <table class="table">
                <thead><tr><th>{"Ref"}</th><th>{"Requester"}</th><th>{"Needed by"}</th><th>{"Total"}</th><th>{"Controlled"}</th></tr></thead>
                <tbody>
                { for rows.into_iter().map(|r| html!{
                    <tr>
                        <td><Link<Route> to={Route::RequisitionDetail { id: r.id.clone() }}>{ r.ref_code }</Link<Route>></td>
                        <td>{ r.requester_email }</td>
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
                <h1>{"Approvals queue"}</h1>
                <p>{"Review and action requisitions that require your sign-off."}</p>
            </div>
            <Card>{ body }</Card>
        </>
    }
}
