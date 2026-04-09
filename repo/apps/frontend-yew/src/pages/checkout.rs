/// Checkout page: verify → inspect pricing summary → confirm order.
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::card::Card;
use crate::router::Route;

#[derive(Clone, PartialEq, Deserialize)]
struct PricedLine {
    sku: String,
    name: String,
    quantity: i32,
    unit_price_cents: i64,
    line_total_cents: i64,
    member_price_applied: bool,
    threshold_discount_applied: bool,
}

#[derive(Clone, PartialEq, Deserialize)]
struct BundleWarning {
    label: String,
    warning_message: String,
}

#[derive(Clone, PartialEq, Deserialize)]
struct PricingSummary {
    lines: Vec<PricedLine>,
    subtotal_cents: i64,
    delivery_fee_cents: i64,
    coupon_discount_cents: i64,
    total_cents: i64,
    coupon_applied: bool,
    coupon_code: Option<String>,
    coupon_blocked_reason: Option<String>,
    bundle_warnings: Vec<BundleWarning>,
    purchase_limit_warnings: Vec<String>,
}

fn cents(c: i64) -> String { format!("${:.2}", c as f64 / 100.0) }

#[derive(Clone, PartialEq)]
enum CheckoutStep {
    Idle,
    Verifying,
    Verified(PricingSummary),
    Confirming,
    Done(String),  // order ref_code
    Error(String),
}

#[function_component(CheckoutPage)]
pub fn checkout_page() -> Html {
    let auth = use_auth();
    let step = use_state(|| CheckoutStep::Idle);
    let navigator = use_navigator().unwrap();

    let verify_cb = {
        let auth = auth.clone();
        let step = step.clone();
        Callback::from(move |_: MouseEvent| {
            let token = auth.api_token.clone();
            let key   = auth.signing_key.clone();
            let step  = step.clone();
            step.set(CheckoutStep::Verifying);
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.post_json("/api/v1/orders/verify", &serde_json::json!({})).await {
                        Ok(r) if r.ok() => match r.json::<PricingSummary>().await {
                            Ok(summary) => step.set(CheckoutStep::Verified(summary)),
                            Err(e) => step.set(CheckoutStep::Error(e.to_string())),
                        },
                        Ok(r) => {
                            let status = r.status();
                            let body = r.text().await.unwrap_or_default();
                            step.set(CheckoutStep::Error(format!("HTTP {status}: {body}")));
                        }
                        Err(e) => step.set(CheckoutStep::Error(e.to_string())),
                    }
                }
            });
        })
    };

    let confirm_cb = {
        let auth = auth.clone();
        let step = step.clone();
        let nav  = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            let token = auth.api_token.clone();
            let key   = auth.signing_key.clone();
            let step  = step.clone();
            let nav   = nav.clone();
            step.set(CheckoutStep::Confirming);
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.post_json("/api/v1/orders/confirm", &serde_json::json!({})).await {
                        Ok(r) if r.ok() => {
                            // Parse just the id for the redirect
                            let body = r.text().await.unwrap_or_default();
                            let id: Option<String> = serde_json::from_str::<serde_json::Value>(&body)
                                .ok()
                                .and_then(|v| v["id"].as_str().map(|s| s.to_string()));
                            if let Some(id) = id {
                                nav.push(&Route::OrderDetail { id });
                            } else {
                                step.set(CheckoutStep::Error("order confirmed but could not parse id".into()));
                            }
                        }
                        Ok(r) => {
                            let status = r.status();
                            let body = r.text().await.unwrap_or_default();
                            step.set(CheckoutStep::Error(format!("HTTP {status}: {body}")));
                        }
                        Err(e) => step.set(CheckoutStep::Error(e.to_string())),
                    }
                }
            });
        })
    };

    let body = match (*step).clone() {
        CheckoutStep::Idle => html! {
            <Card>
                <p>{"Click Verify to re-check prices and stock before placing your order."}</p>
                <p class="muted">{"The price lock expires in 10 minutes."}</p>
                <button class="button" onclick={verify_cb}>{"Verify order"}</button>
            </Card>
        },
        CheckoutStep::Verifying => html! {
            <Card><div class="state">{"Verifying…"}</div></Card>
        },
        CheckoutStep::Error(e) => html! {
            <Card>
                <div class="state error">{ e }</div>
                <div style="margin-top:12px;">
                    <Link<Route> to={Route::Cart}>
                        <button class="button secondary">{"Back to cart"}</button>
                    </Link<Route>>
                    {" "}
                    <button class="button" onclick={verify_cb}>{"Retry verify"}</button>
                </div>
            </Card>
        },
        CheckoutStep::Verified(summary) => html! {
            <>
                // Warnings
                { for summary.bundle_warnings.iter().map(|w| html! {
                    <div style="background:#fff3cd;padding:10px;margin-bottom:8px;border-radius:6px;">
                        <strong>{ &w.label }{": "}</strong>{ &w.warning_message }
                    </div>
                })}
                { for summary.purchase_limit_warnings.iter().map(|w| html! {
                    <div style="background:#f8d7da;padding:10px;margin-bottom:8px;border-radius:6px;">
                        { w }
                    </div>
                })}

                <Card title={AttrValue::from("Order summary (price locked)")}>
                    <table class="table">
                        <thead><tr>
                            <th>{"Item"}</th><th>{"Qty"}</th>
                            <th>{"Unit"}</th><th>{"Total"}</th><th>{"Pricing"}</th>
                        </tr></thead>
                        <tbody>
                        { for summary.lines.iter().map(|l| html! {
                            <tr>
                                <td>{ &l.name }<br/><small class="muted">{ &l.sku }</small></td>
                                <td>{ l.quantity }</td>
                                <td>{ cents(l.unit_price_cents) }</td>
                                <td>{ cents(l.line_total_cents) }</td>
                                <td>
                                    { if l.member_price_applied { html!{ <span class="badge">{"member"}</span> } } else { html!{} } }
                                    { if l.threshold_discount_applied { html!{ <span class="badge">{"bulk"}</span> } } else { html!{} } }
                                </td>
                            </tr>
                        })}
                        </tbody>
                    </table>

                    <div style="text-align:right;margin-top:12px;">
                        <p>{"Subtotal: "}{ cents(summary.subtotal_cents) }</p>
                        if summary.delivery_fee_cents > 0 {
                            <p>{"Delivery: "}{ cents(summary.delivery_fee_cents) }</p>
                        }
                        if summary.coupon_applied {
                            <p style="color:green;">
                                {"Coupon ("}{summary.coupon_code.as_deref().unwrap_or("")}{"): -"}
                                { cents(summary.coupon_discount_cents) }
                            </p>
                        }
                        if let Some(reason) = &summary.coupon_blocked_reason {
                            <p style="color:orange;">{ format!("Coupon not applied: {reason}") }</p>
                        }
                        <p style="font-size:1.2em;"><strong>{"Total: "}{ cents(summary.total_cents) }</strong></p>
                    </div>
                </Card>

                <div style="display:flex;gap:12px;margin-top:16px;justify-content:flex-end;">
                    <Link<Route> to={Route::Cart}>
                        <button class="button secondary">{"Edit cart"}</button>
                    </Link<Route>>
                    <button class="button" onclick={confirm_cb}>{"Confirm order"}</button>
                </div>
            </>
        },
        CheckoutStep::Confirming => html! {
            <Card><div class="state">{"Placing order…"}</div></Card>
        },
        CheckoutStep::Done(_) => html! {
            <Card><div class="state">{"Order placed! Redirecting…"}</div></Card>
        },
    };

    html! {
        <>
            <div class="page-header">
                <h1>{"Checkout"}</h1>
                <p>{"Review your order before confirming."}</p>
            </div>
            { body }
        </>
    }
}
