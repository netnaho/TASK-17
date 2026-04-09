use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::{card::Card, states::EmptyState, utils::fmt_date};
use crate::router::Route;

#[derive(Clone, PartialEq, Deserialize)]
struct OrderSummary {
    id: String,
    ref_code: String,
    status: String,
    total_cents: i64,
    created_at: String,
}

fn cents(c: i64) -> String { format!("${:.2}", c as f64 / 100.0) }

#[function_component(OrdersPage)]
pub fn orders() -> Html {
    let auth = use_auth();
    let orders = use_state(|| Option::<Result<Vec<OrderSummary>, String>>::None);

    {
        let orders = orders.clone();
        let auth = auth.clone();
        use_effect_with((), move |_| {
            let orders = orders.clone();
            let token = auth.api_token.clone();
            let key = auth.signing_key.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.get("/api/v1/orders/mine").await {
                        Ok(r) if r.ok() => match r.json::<Vec<OrderSummary>>().await {
                            Ok(v) => orders.set(Some(Ok(v))),
                            Err(e) => orders.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => orders.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => orders.set(Some(Err(e.to_string()))),
                    }
                }
            });
            || ()
        });
    }

    let body = match (*orders).clone() {
        None => html! { <div class="state">{"Loading…"}</div> },
        Some(Err(e)) => html! { <div class="state error">{ e }</div> },
        Some(Ok(rows)) if rows.is_empty() => html! {
            <EmptyState
                title={AttrValue::from("No orders yet")}
                detail={Some(AttrValue::from("Visit the shop to place your first order."))}
            />
        },
        Some(Ok(rows)) => html! {
            <table class="table">
                <thead><tr>
                    <th>{"Ref"}</th><th>{"Status"}</th>
                    <th>{"Total"}</th><th>{"Date"}</th>
                </tr></thead>
                <tbody>
                { for rows.into_iter().map(|o| html! {
                    <tr>
                        <td>
                            <Link<Route> to={Route::OrderDetail { id: o.id.clone() }}>
                                { &o.ref_code }
                            </Link<Route>>
                        </td>
                        <td>{ &o.status }</td>
                        <td>{ cents(o.total_cents) }</td>
                        <td>{ fmt_date(&o.created_at) }</td>
                    </tr>
                })}
                </tbody>
            </table>
        },
    };

    html! {
        <>
            <div class="page-header">
                <h1>{"My orders"}</h1>
                <p>
                    <Link<Route> to={Route::Cart}>
                        <button class="button">{"Shop — go to cart"}</button>
                    </Link<Route>>
                </p>
            </div>
            <Card>{ body }</Card>
        </>
    }
}
