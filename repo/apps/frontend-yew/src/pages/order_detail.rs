use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::{card::Card, utils::fmt_date};

#[derive(Properties, PartialEq)]
pub struct Props { pub id: String }

#[derive(Clone, PartialEq, Deserialize)]
struct OrderLineView {
    sku: String,
    name: String,
    quantity: i32,
    unit_price_cents: i64,
    line_total_cents: i64,
    member_price_applied: bool,
    threshold_discount_applied: bool,
    discount_bps: i32,
}

#[derive(Clone, PartialEq, Deserialize)]
struct AdjustmentView {
    kind: String,
    label: String,
    amount_cents: i64,
}

#[derive(Clone, PartialEq, Deserialize)]
struct OrderDetail {
    id: String,
    ref_code: String,
    status: String,
    subtotal_cents: i64,
    delivery_fee_cents: i64,
    discount_cents: i64,
    total_cents: i64,
    delivery_key: String,
    coupon_code: Option<String>,
    lines: Vec<OrderLineView>,
    adjustments: Vec<AdjustmentView>,
    confirmed_at: String,
}

fn cents(c: i64) -> String { format!("${:.2}", c as f64 / 100.0) }

#[function_component(OrderDetailPage)]
pub fn order_detail(props: &Props) -> Html {
    let auth   = use_auth();
    let detail = use_state(|| Option::<Result<OrderDetail, String>>::None);
    let id = props.id.clone();

    {
        let detail = detail.clone();
        let id     = id.clone();
        let auth   = auth.clone();
        use_effect_with(id.clone(), move |_| {
            let detail = detail.clone();
            let token  = auth.api_token.clone();
            let key    = auth.signing_key.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    let path = format!("/api/v1/orders/{id}");
                    match client.get(&path).await {
                        Ok(r) if r.ok() => match r.json::<OrderDetail>().await {
                            Ok(v) => detail.set(Some(Ok(v))),
                            Err(e) => detail.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => detail.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => detail.set(Some(Err(e.to_string()))),
                    }
                }
            });
            || ()
        });
    }

    match (*detail).clone() {
        None => html! { <div class="state">{"Loading…"}</div> },
        Some(Err(e)) => html! { <div class="state error">{ e }</div> },
        Some(Ok(o)) => html! {
            <>
                <div class="page-header">
                    <h1>{ format!("{} — {}", o.ref_code, o.status) }</h1>
                    <p>{ format!("Confirmed: {} · Delivery: {}", fmt_date(&o.confirmed_at), o.delivery_key) }</p>
                </div>
                <div class="grid-2">
                    <Card title={AttrValue::from("Items")}>
                        <table class="table">
                            <thead><tr>
                                <th>{"Item"}</th><th>{"Qty"}</th>
                                <th>{"Unit"}</th><th>{"Total"}</th><th>{"Pricing"}</th>
                            </tr></thead>
                            <tbody>
                            { for o.lines.iter().map(|l| html! {
                                <tr>
                                    <td>{ &l.name }<br/><small class="muted">{ &l.sku }</small></td>
                                    <td>{ l.quantity }</td>
                                    <td>{ cents(l.unit_price_cents) }</td>
                                    <td>{ cents(l.line_total_cents) }</td>
                                    <td>
                                        { if l.member_price_applied { html!{<span class="badge">{"member"}</span>} } else { html!{} } }
                                        { if l.threshold_discount_applied { html!{<span class="badge">{"bulk"}</span>} } else { html!{} } }
                                    </td>
                                </tr>
                            })}
                            </tbody>
                        </table>
                    </Card>

                    <Card title={AttrValue::from("Summary")}>
                        <p>{"Subtotal: "}{ cents(o.subtotal_cents) }</p>
                        <p>{"Delivery: "}{ cents(o.delivery_fee_cents) }</p>
                        if o.discount_cents > 0 {
                            <p style="color:green;">
                                {"Discount ("}{ o.coupon_code.as_deref().unwrap_or("") }{"): -"}
                                { cents(o.discount_cents) }
                            </p>
                        }
                        <p><strong>{"Total: "}{ cents(o.total_cents) }</strong></p>

                        if !o.adjustments.is_empty() {
                            <hr/>
                            <h4>{"Adjustments"}</h4>
                            { for o.adjustments.iter().map(|a| html! {
                                <p>{ format!("{}: {}", a.label, cents(a.amount_cents.abs())) }
                                   { if a.amount_cents < 0 { " (discount)" } else { "" } }
                                </p>
                            })}
                        }
                    </Card>
                </div>
            </>
        },
    }
}
