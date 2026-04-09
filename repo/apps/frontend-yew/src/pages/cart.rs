/// Cart page: browse products, manage cart lines, apply coupon, pick delivery.
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::card::Card;
use crate::router::Route;

// ── DTOs ──────────────────────────────────────────────────────────────────
#[derive(Clone, PartialEq, Deserialize, Debug)]
struct Product {
    id: String,
    sku: String,
    name: String,
    store_key: String,
    base_price_cents: i64,
    stock: i32,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct DeliveryMethod {
    id: String,
    key: String,
    label: String,
    fee_cents: i64,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct CartLineView {
    product_id: String,
    sku: String,
    name: String,
    quantity: i32,
    #[allow(dead_code)]
    store_key: String,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct PricedLine {
    product_id: String,
    sku: String,
    unit_price_cents: i64,
    line_total_cents: i64,
    member_price_applied: bool,
    threshold_discount_applied: bool,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct BundleWarning {
    label: String,
    warning_message: String,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
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

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct CartView {
    id: String,
    lines: Vec<CartLineView>,
    coupon_code: Option<String>,
    delivery_method_id: Option<String>,
    has_cross_store: bool,
    pricing: Option<PricingSummary>,
}

#[derive(Serialize)]
struct SetLineBody { product_id: String, quantity: i32 }

fn cents(c: i64) -> String { format!("${:.2}", c as f64 / 100.0) }

// ── helpers that fire and reload cart ────────────────────────────────────

async fn fetch_cart(token: String, key: String) -> Option<CartView> {
    let client = ApiClient { api_token: token, signing_key: key };
    if let Ok(r) = client.get("/api/v1/orders/cart").await {
        if r.ok() { return r.json::<CartView>().await.ok(); }
    }
    None
}

// ── Component ─────────────────────────────────────────────────────────────
#[function_component(CartPage)]
pub fn cart_page() -> Html {
    let auth     = use_auth();
    let products = use_state(Vec::<Product>::new);
    let methods  = use_state(Vec::<DeliveryMethod>::new);
    let cart     = use_state(|| Option::<CartView>::None);
    let error    = use_state(|| Option::<String>::None);
    let coupon_input = use_state(String::new);

    // qty_inputs: product_id → desired quantity (for the add-to-cart inputs)
    let qty_inputs: UseStateHandle<std::collections::HashMap<String, i32>> =
        use_state(std::collections::HashMap::new);

    // ── initial load ──────────────────────────────────────────────────────
    {
        let products = products.clone();
        let methods  = methods.clone();
        let cart     = cart.clone();
        let error    = error.clone();
        let auth     = auth.clone();
        use_effect_with((), move |_| {
            let token = auth.api_token.clone();
            let key   = auth.signing_key.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token.clone(), signing_key: key.clone() };
                    if let Ok(r) = client.get("/api/v1/orders/products").await {
                        if let Ok(v) = r.json::<Vec<Product>>().await { products.set(v); }
                    }
                    let client2 = ApiClient { api_token: token.clone(), signing_key: key.clone() };
                    if let Ok(r) = client2.get("/api/v1/orders/delivery-methods").await {
                        if let Ok(v) = r.json::<Vec<DeliveryMethod>>().await { methods.set(v); }
                    }
                    match fetch_cart(token, key).await {
                        Some(c) => cart.set(Some(c)),
                        None => error.set(Some("failed to load cart".into())),
                    }
                }
            });
            || ()
        });
    }

    // ── action: add/update a cart line ────────────────────────────────────
    let add_to_cart = {
        let auth       = auth.clone();
        let cart       = cart.clone();
        let qty_inputs = qty_inputs.clone();
        move |product_id: String| {
            let token      = auth.api_token.clone();
            let key        = auth.signing_key.clone();
            let cart       = cart.clone();
            let qty        = *qty_inputs.get(&product_id).unwrap_or(&1);
            let pid        = product_id;
            Callback::from(move |_: MouseEvent| {
                let token = token.clone();
                let key   = key.clone();
                let cart  = cart.clone();
                let pid   = pid.clone();
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (token, key) {
                        let client = ApiClient { api_token: token.clone(), signing_key: key.clone() };
                        let body = SetLineBody { product_id: pid, quantity: qty };
                        let _ = client.post_json("/api/v1/orders/cart/lines", &body).await;
                        if let Some(c) = fetch_cart(token, key).await { cart.set(Some(c)); }
                    }
                });
            })
        }
    };

    // ── action: remove a cart line ────────────────────────────────────────
    let remove_line = {
        let auth = auth.clone();
        let cart = cart.clone();
        move |product_id: String| {
            let token = auth.api_token.clone();
            let key   = auth.signing_key.clone();
            let cart  = cart.clone();
            let pid   = product_id;
            Callback::from(move |_: MouseEvent| {
                let token = token.clone();
                let key   = key.clone();
                let cart  = cart.clone();
                let pid   = pid.clone();
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (token, key) {
                        let client = ApiClient { api_token: token.clone(), signing_key: key.clone() };
                        let body = SetLineBody { product_id: pid, quantity: 0 };
                        let _ = client.post_json("/api/v1/orders/cart/lines", &body).await;
                        if let Some(c) = fetch_cart(token, key).await { cart.set(Some(c)); }
                    }
                });
            })
        }
    };

    // ── action: apply coupon ──────────────────────────────────────────────
    let apply_coupon_cb = {
        let auth         = auth.clone();
        let cart         = cart.clone();
        let coupon_input = coupon_input.clone();
        Callback::from(move |_: MouseEvent| {
            let token = auth.api_token.clone();
            let key   = auth.signing_key.clone();
            let cart  = cart.clone();
            let code  = (*coupon_input).clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token.clone(), signing_key: key.clone() };
                    let body = serde_json::json!({ "code": code });
                    let _ = client.post_json("/api/v1/orders/cart/coupon", &body).await;
                    if let Some(c) = fetch_cart(token, key).await { cart.set(Some(c)); }
                }
            });
        })
    };

    // ── action: remove coupon ─────────────────────────────────────────────
    let remove_coupon_cb = {
        let auth = auth.clone();
        let cart = cart.clone();
        Callback::from(move |_: MouseEvent| {
            let token = auth.api_token.clone();
            let key   = auth.signing_key.clone();
            let cart  = cart.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token.clone(), signing_key: key.clone() };
                    let _ = client.post_json("/api/v1/orders/cart/coupon-remove",
                        &serde_json::json!({})).await;
                    if let Some(c) = fetch_cart(token, key).await { cart.set(Some(c)); }
                }
            });
        })
    };

    // ── action: set delivery method (POST) ───────────────────────────────
    let set_delivery = {
        let auth = auth.clone();
        let cart = cart.clone();
        move |dm_id: String| {
            let token = auth.api_token.clone();
            let key   = auth.signing_key.clone();
            let cart  = cart.clone();
            Callback::from(move |_: MouseEvent| {
                let token = token.clone();
                let key   = key.clone();
                let cart  = cart.clone();
                let dm_id = dm_id.clone();
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (token, key) {
                        let client = ApiClient { api_token: token.clone(), signing_key: key.clone() };
                        let body = serde_json::json!({ "delivery_method_id": dm_id });
                        let _ = client.post_json("/api/v1/orders/cart/delivery", &body).await;
                        if let Some(c) = fetch_cart(token, key).await { cart.set(Some(c)); }
                    }
                });
            })
        }
    };

    // ── derive rendering data ─────────────────────────────────────────────
    let cart_data   = (*cart).clone();
    let cart_lines  = cart_data.as_ref().map(|c| c.lines.clone()).unwrap_or_default();
    let pricing     = cart_data.as_ref().and_then(|c| c.pricing.clone());
    let has_cross   = cart_data.as_ref().map(|c| c.has_cross_store).unwrap_or(false);
    let sel_dm_id   = cart_data.as_ref().and_then(|c| c.delivery_method_id.clone());
    let cart_coupon = cart_data.as_ref().and_then(|c| c.coupon_code.clone());
    let has_items   = !cart_lines.is_empty();

    // group product store keys (deduped, sorted)
    let mut store_keys: Vec<String> = (*products).iter().map(|p| p.store_key.clone()).collect();
    store_keys.sort();
    store_keys.dedup();

    html! {
        <>
            <div class="page-header">
                <h1>{"Shop & Cart"}</h1>
                <p>{"Browse consumable items, manage your cart, then check out."}</p>
            </div>

            if let Some(e) = (*error).clone() {
                <div class="state error">{ e }</div>
            }

            if has_cross {
                <div style="background:#fff3cd;padding:10px;margin-bottom:12px;border-radius:6px;">
                    {"Your cart contains items from multiple stores. Each will be fulfilled separately."}
                </div>
            }

            // ── product catalog ──────────────────────────────────────────
            { for store_keys.iter().map(|store| {
                let store_products: Vec<Product> = (*products).iter()
                    .filter(|p| &p.store_key == store).cloned().collect();
                let store_label = store.clone();
                html! {
                    <Card title={AttrValue::from(format!("Store: {store_label}"))}>
                        <table class="table">
                            <thead><tr>
                                <th>{"Item"}</th><th>{"Base price"}</th>
                                <th>{"Stock"}</th><th>{"Qty"}</th><th></th>
                            </tr></thead>
                            <tbody>
                            { for store_products.into_iter().map(|p| {
                                                let pid = p.id.clone();
                                let pid2 = pid.clone();
                                let pid3 = pid.clone();
                                let qi2 = qty_inputs.clone();
                                let qty_val = *qty_inputs.get(&pid).unwrap_or(&1);
                                let add_cb = add_to_cart(pid2);
                                html! {
                                    <tr>
                                        <td>{ &p.name }<br/><small class="muted">{ &p.sku }</small></td>
                                        <td>{ cents(p.base_price_cents) }</td>
                                        <td>{ p.stock }</td>
                                        <td>
                                            <input
                                                type="number" min="1" max="5"
                                                style="width:60px"
                                                value={qty_val.to_string()}
                                                oninput={Callback::from(move |e: InputEvent| {
                                                    let el: web_sys::HtmlInputElement =
                                                        e.target_unchecked_into();
                                                    if let Ok(v) = el.value().parse::<i32>() {
                                                        let mut m = (*qi2).clone();
                                                        m.insert(pid3.clone(), v);
                                                        qi2.set(m);
                                                    }
                                                })}
                                            />
                                        </td>
                                        <td>
                                            <button class="button" onclick={add_cb}>{"Add"}</button>
                                        </td>
                                    </tr>
                                }
                            })}
                            </tbody>
                        </table>
                    </Card>
                }
            })}

            // ── cart lines ────────────────────────────────────────────────
            <Card title={AttrValue::from("Your cart")}>
                if cart_lines.is_empty() {
                    <p class="muted">{"Cart is empty — add items above."}</p>
                } else {
                    <table class="table">
                        <thead><tr>
                            <th>{"Item"}</th><th>{"Qty"}</th>
                            if pricing.is_some() {
                                <>
                                    <th>{"Unit price"}</th>
                                    <th>{"Line total"}</th>
                                    <th>{"Pricing"}</th>
                                </>
                            }
                            <th></th>
                        </tr></thead>
                        <tbody>
                        { for cart_lines.iter().map(|l| {
                            let pid = l.product_id.clone();
                            let priced = pricing.as_ref().and_then(|pr|
                                pr.lines.iter().find(|pl| pl.product_id == pid).cloned()
                            );
                            let rem_cb = remove_line(pid);
                            html! {
                                <tr>
                                    <td>{ &l.name }<br/><small class="muted">{ &l.sku }</small></td>
                                    <td>{ l.quantity }</td>
                                    { match priced {
                                        Some(pr) => html! {
                                            <>
                                                <td>{ cents(pr.unit_price_cents) }</td>
                                                <td>{ cents(pr.line_total_cents) }</td>
                                                <td>
                                                    { if pr.member_price_applied {
                                                        html!{<span class="badge" style="background:#d1ecf1;padding:2px 6px;border-radius:4px;font-size:0.8em;">{"member"}</span>}
                                                    } else { html!{} } }
                                                    { if pr.threshold_discount_applied {
                                                        html!{<span class="badge" style="background:#d4edda;padding:2px 6px;border-radius:4px;font-size:0.8em;">{"bulk"}</span>}
                                                    } else { html!{} } }
                                                </td>
                                            </>
                                        },
                                        None => html! { <></> },
                                    } }
                                    <td>
                                        <button class="button secondary" onclick={rem_cb}>
                                            {"Remove"}
                                        </button>
                                    </td>
                                </tr>
                            }
                        })}
                        </tbody>
                    </table>

                    // Pricing summary
                    if let Some(ref pr) = pricing {
                        <div style="margin-top:16px;text-align:right;line-height:1.8;">
                            <div>{"Subtotal: "}{ cents(pr.subtotal_cents) }</div>
                            if pr.delivery_fee_cents > 0 {
                                <div>{"Delivery: "}{ cents(pr.delivery_fee_cents) }</div>
                            }
                            if pr.coupon_applied {
                                <div style="color:green;">
                                    {"Coupon "}
                                    { pr.coupon_code.as_deref().map(|c| format!("({c})")).unwrap_or_default() }
                                    {": -"}{ cents(pr.coupon_discount_cents) }
                                </div>
                            }
                            if let Some(ref reason) = pr.coupon_blocked_reason {
                                <div style="color:#856404;">
                                    { format!("Coupon not applied: {reason}") }
                                </div>
                            }
                            <div style="font-weight:bold;margin-top:4px;">
                                {"Total: "}{ cents(pr.total_cents) }
                            </div>
                        </div>

                        { for pr.bundle_warnings.iter().map(|w| html! {
                            <div style="background:#fff3cd;padding:8px;margin-top:8px;border-radius:4px;">
                                <strong>{ &w.label }{": "}</strong>{ &w.warning_message }
                            </div>
                        })}
                        { for pr.purchase_limit_warnings.iter().map(|w| html! {
                            <div style="background:#f8d7da;padding:8px;margin-top:8px;border-radius:4px;">
                                {"Daily limit: "}{ w }
                            </div>
                        })}
                    }
                }
            </Card>

            // ── coupon ────────────────────────────────────────────────────
            <Card title={AttrValue::from("Coupon")}>
                if let Some(ref code) = cart_coupon {
                    <div style="display:flex;align-items:center;gap:12px;">
                        <p style="margin:0;">{"Applied: "}<strong>{ code }</strong></p>
                        <button class="button secondary" onclick={remove_coupon_cb}>{"Remove"}</button>
                    </div>
                } else {
                    <div style="display:flex;gap:8px;align-items:center;">
                        <input
                            type="text"
                            placeholder="Coupon code (e.g. WELCOME10)"
                            style="flex:1"
                            value={(*coupon_input).clone()}
                            oninput={{
                                let ci = coupon_input.clone();
                                Callback::from(move |e: InputEvent| {
                                    let el: web_sys::HtmlInputElement = e.target_unchecked_into();
                                    ci.set(el.value());
                                })
                            }}
                        />
                        <button class="button" onclick={apply_coupon_cb}>{"Apply"}</button>
                    </div>
                }
            </Card>

            // ── delivery method ───────────────────────────────────────────
            <Card title={AttrValue::from("Delivery method")}>
                { for (*methods).iter().map(|m| {
                    let dm_id   = m.id.clone();
                    let is_sel  = sel_dm_id.as_deref() == Some(&dm_id);
                    let onclick = set_delivery(dm_id);
                    let fee_str = if m.fee_cents == 0 {
                        "Free".to_string()
                    } else {
                        cents(m.fee_cents)
                    };
                    html! {
                        <label style="display:flex;align-items:center;gap:12px;margin-bottom:10px;cursor:pointer;">
                            <input
                                type="radio"
                                name="delivery"
                                checked={is_sel}
                                onclick={onclick}
                            />
                            { format!("{} — {}", m.label, fee_str) }
                        </label>
                    }
                })}
            </Card>

            // ── proceed to checkout ───────────────────────────────────────
            if has_items {
                <div style="text-align:right;margin-top:16px;">
                    <Link<Route> to={Route::Checkout}>
                        <button class="button">{"Proceed to checkout →"}</button>
                    </Link<Route>>
                </div>
            }
        </>
    }
}
