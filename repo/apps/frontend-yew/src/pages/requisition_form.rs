use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::card::Card;
use crate::router::Route;

const ROUTING_CAP_CENTS: i64 = 250_000; // $2,500.00 — must match seed

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct CatalogItem {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub unit: String,
    pub unit_price_cents: i64,
    pub on_hand: i32,
    pub controlled: bool,
    pub category: String,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct SiteDept { pub site_id: String, pub department_id: String }

#[derive(Serialize)]
struct LineInput<'a> { item_id: &'a str, quantity: i32 }

#[derive(Serialize)]
struct CreateInput<'a> {
    department_id: &'a str,
    needed_by: &'a str, // ISO yyyy-mm-dd
    justification: &'a str,
    lines: Vec<LineInput<'a>>,
}

#[derive(Deserialize)]
struct CreatedReq { id: String }

fn cents_fmt(c: i64) -> String { format!("${:.2}", c as f64 / 100.0) }

#[function_component(RequisitionFormPage)]
pub fn requisition_form() -> Html {
    let auth = use_auth();
    let nav = use_navigator().unwrap();
    let catalog = use_state(|| Vec::<CatalogItem>::new());
    let load_err = use_state(|| Option::<String>::None);
    let qty: UseStateHandle<HashMap<String, i32>> = use_state(HashMap::new);
    let needed_by = use_state(|| String::new());
    let justification = use_state(|| String::new());
    let site_dept = use_state(|| Option::<SiteDept>::None);
    let submit_err = use_state(|| Option::<String>::None);

    {
        let catalog = catalog.clone();
        let load_err = load_err.clone();
        let auth = auth.clone();
        let site_dept = site_dept.clone();
        use_effect_with((), move |_| {
            let token = auth.api_token.clone();
            let key = auth.signing_key.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.get("/api/v1/inventory/catalog").await {
                        Ok(r) if r.ok() => match r.json::<Vec<CatalogItem>>().await {
                            Ok(v) => catalog.set(v),
                            Err(e) => load_err.set(Some(e.to_string())),
                        },
                        Ok(r) => load_err.set(Some(format!("HTTP {}", r.status()))),
                        Err(e) => load_err.set(Some(e.to_string())),
                    }
                    // Phase 3: also fetch /me to learn the principal's first
                    // department scope, used as site/department for create.
                    // We hard-code the demo lookup via /api/v1/me's scopes.
                    let _ = site_dept; // TODO: derive from /me; left for follow-up.
                }
            });
            || ()
        });
    }

    // Compute total + controlled flag from current quantities.
    let mut total: i64 = 0;
    let mut controlled = false;
    for item in catalog.iter() {
        let q = qty.get(&item.id).copied().unwrap_or(0);
        if q > 0 {
            total += item.unit_price_cents * q as i64;
            if item.controlled { controlled = true; }
        }
    }
    let routes_to_finance = total > ROUTING_CAP_CENTS || controlled;

    let on_qty = {
        let qty = qty.clone();
        Callback::from(move |(id, value): (String, String)| {
            let mut map = (*qty).clone();
            let v: i32 = value.parse().unwrap_or(0);
            if v <= 0 { map.remove(&id); } else { map.insert(id, v); }
            qty.set(map);
        })
    };

    let on_submit = {
        let qty = qty.clone();
        let catalog = catalog.clone();
        let needed_by = needed_by.clone();
        let justification = justification.clone();
        let auth = auth.clone();
        let nav = nav.clone();
        let submit_err = submit_err.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            submit_err.set(None);
            // Build line list from non-zero qty.
            let lines: Vec<(String, i32)> = catalog
                .iter()
                .filter_map(|i| qty.get(&i.id).copied().map(|q| (i.id.clone(), q)))
                .collect();
            if lines.is_empty() {
                submit_err.set(Some("add at least one line".into()));
                return;
            }
            if needed_by.is_empty() {
                submit_err.set(Some("needed-by date required".into()));
                return;
            }
            if justification.trim().is_empty() {
                submit_err.set(Some("justification required".into()));
                return;
            }
            // We need a site_id + department_id. For Phase 3 the API uses
            // the user's first department scope; the form posts and lets
            // the backend resolve. We send placeholders here and require
            // operators to use API directly if they need a different dept.
            // The backend rejects with a clear error if dept missing.
            let token = auth.api_token.clone();
            let key = auth.signing_key.clone();
            let needed_by = (*needed_by).clone();
            let justification = (*justification).clone();
            let nav = nav.clone();
            let submit_err = submit_err.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token.clone(), signing_key: key.clone() };
                    // Look up site/department via /api/v1/me
                    let me_resp = client.get("/api/v1/me").await;
                    let dept_id = match me_resp {
                        Ok(r) if r.ok() => {
                            let v: serde_json::Value = r.json().await.unwrap_or(serde_json::json!({}));
                            v.get("scopes").and_then(|s| s.as_array()).cloned().unwrap_or_default()
                                .into_iter()
                                .find(|s| s.get("kind").and_then(|k| k.as_str()) == Some("department"))
                                .and_then(|s| s.get("reference").and_then(|r| r.as_str()).map(|s| s.to_string()))
                                .unwrap_or_default()
                        }
                        _ => String::new(),
                    };
                    let line_inputs: Vec<LineInput> =
                        lines.iter().map(|(i, q)| LineInput { item_id: i, quantity: *q }).collect();
                    let body = CreateInput {
                        department_id: &dept_id,
                        needed_by: &needed_by,
                        justification: &justification,
                        lines: line_inputs,
                    };
                    match client.post_json("/api/v1/requisitions", &body).await {
                        Ok(r) if r.ok() => match r.json::<CreatedReq>().await {
                            Ok(c) => {
                                // Submit immediately to start approval routing.
                                let _ = client
                                    .post_json::<serde_json::Value>(
                                        &format!("/api/v1/requisitions/{}/submit", c.id),
                                        &serde_json::json!({}),
                                    )
                                    .await;
                                nav.push(&Route::RequisitionDetail { id: c.id });
                            }
                            Err(e) => submit_err.set(Some(e.to_string())),
                        },
                        Ok(r) => {
                            let status = r.status();
                            let txt = r.text().await.unwrap_or_default();
                            submit_err.set(Some(format!("HTTP {status}: {txt}")));
                        }
                        Err(e) => submit_err.set(Some(e.to_string())),
                    }
                }
            });
        })
    };

    let on_needed = {
        let needed_by = needed_by.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            needed_by.set(i.value());
        })
    };
    let on_just = {
        let justification = justification.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            justification.set(i.value());
        })
    };

    html! {
        <>
            <div class="page-header">
                <h1>{"New requisition"}</h1>
                <p>{"Pick items, enter quantities, and submit for approval."}</p>
            </div>
            <Card title={AttrValue::from("On-hand catalog")}>
                if let Some(err) = (*load_err).clone() {
                    <div class="state error">{ err }</div>
                }
                <table class="table">
                    <thead><tr>
                        <th>{"Item"}</th><th>{"Category"}</th><th>{"Unit"}</th>
                        <th>{"Unit price"}</th><th>{"On hand"}</th><th>{"Quantity"}</th><th>{"Line total"}</th>
                    </tr></thead>
                    <tbody>
                    { for catalog.iter().map(|item| {
                        let id = item.id.clone();
                        let q = qty.get(&item.id).copied().unwrap_or(0);
                        let line_total = item.unit_price_cents * q as i64;
                        let on_qty = on_qty.clone();
                        let oninput = Callback::from(move |e: InputEvent| {
                            let i: HtmlInputElement = e.target_unchecked_into();
                            on_qty.emit((id.clone(), i.value()));
                        });
                        html!{
                            <tr>
                                <td>{ &item.name }<br/><small class="muted">{ &item.sku }</small></td>
                                <td>{ &item.category }{ if item.controlled { " (controlled)" } else { "" } }</td>
                                <td>{ &item.unit }</td>
                                <td>{ cents_fmt(item.unit_price_cents) }</td>
                                <td>{ item.on_hand }</td>
                                <td><input type="number" min="0" max={item.on_hand.to_string()} value={q.to_string()} {oninput} style="width:80px;" /></td>
                                <td>{ cents_fmt(line_total) }</td>
                            </tr>
                        }
                    })}
                    </tbody>
                </table>
            </Card>

            <Card title={AttrValue::from("Summary")}>
                <div class="grid-3">
                    <div class="stat"><span class="label">{"Projected total"}</span><span class="value">{ cents_fmt(total) }</span></div>
                    <div class="stat"><span class="label">{"Controlled items"}</span><span class="value">{ if controlled { "Yes" } else { "No" } }</span></div>
                    <div class="stat"><span class="label">{"Routing"}</span><span class="value">
                        { if routes_to_finance { "Dept → Finance" } else { "Dept only" } }
                    </span></div>
                </div>
                if total > ROUTING_CAP_CENTS {
                    <p class="muted" style="margin-top:12px;">
                        { format!("Above the {} cap — finance approval required.", cents_fmt(ROUTING_CAP_CENTS)) }
                    </p>
                }
            </Card>

            <Card>
                <form onsubmit={on_submit}>
                    <div class="form-field">
                        <label>{"Needed by (date)"}</label>
                        <input type="date" required=true value={(*needed_by).clone()} oninput={on_needed} />
                    </div>
                    <div class="form-field">
                        <label>{"Justification"}</label>
                        <textarea required=true rows="3" value={(*justification).clone()} oninput={on_just}></textarea>
                    </div>
                    if let Some(err) = (*submit_err).clone() {
                        <div class="state error">{ err }</div>
                    }
                    <button class="button" type="submit">{"Submit for approval"}</button>
                </form>
            </Card>
        </>
    }
}
