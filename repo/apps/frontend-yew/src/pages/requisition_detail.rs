use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::card::Card;

#[derive(Properties, PartialEq)]
pub struct DetailProps { pub id: String }

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct Detail {
    pub id: String,
    pub ref_code: String,
    pub status: String,
    pub requester_email: String,
    pub needed_by: String,
    pub justification: String,
    pub total_amount_cents: i64,
    pub contains_controlled: bool,
    pub lines: Vec<Line>,
    pub route: Vec<Step>,
    pub audit: Vec<AuditEntry>,
}
#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct Line {
    pub item_name: String, pub sku: String, pub quantity: i32,
    pub unit_price_cents: i64, pub line_total_cents: i64, pub on_hand: i32,
    pub controlled: bool,
}
#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct Step {
    pub sequence: i32, pub label: String, pub required_role: String,
    pub decision: Option<String>, pub reason: Option<String>,
}
#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct AuditEntry {
    pub occurred_at: String, pub event_kind: String, pub comment: Option<String>,
}

fn cents_fmt(c: i64) -> String { format!("${:.2}", c as f64 / 100.0) }

#[function_component(RequisitionDetailPage)]
pub fn requisition_detail(props: &DetailProps) -> Html {
    let auth = use_auth();
    let detail = use_state(|| Option::<Result<Detail, String>>::None);
    let reason = use_state(|| String::new());
    let id = props.id.clone();

    let load = {
        let detail = detail.clone();
        let id = id.clone();
        let auth = auth.clone();
        Callback::from(move |_| {
            let detail = detail.clone();
            let id = id.clone();
            let token = auth.api_token.clone();
            let key = auth.signing_key.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (token, key) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    let path = format!("/api/v1/requisitions/{id}");
                    match client.get(&path).await {
                        Ok(r) if r.ok() => match r.json::<Detail>().await {
                            Ok(v) => detail.set(Some(Ok(v))),
                            Err(e) => detail.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => detail.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => detail.set(Some(Err(e.to_string()))),
                    }
                }
            });
        })
    };
    {
        let load = load.clone();
        use_effect_with(id.clone(), move |_| { load.emit(()); || () });
    }

    let action = {
        let auth = auth.clone();
        let id = id.clone();
        let reason = reason.clone();
        let load = load.clone();
        move |verb: &'static str| {
            let token = auth.api_token.clone();
            let key = auth.signing_key.clone();
            let id = id.clone();
            let reason = (*reason).clone();
            let load = load.clone();
            Callback::from(move |_| {
                let token = token.clone();
                let key = key.clone();
                let id = id.clone();
                let reason = reason.clone();
                let load = load.clone();
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (token, key) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        let path = format!("/api/v1/requisitions/{id}/{verb}");
                        let body = serde_json::json!({"reason": reason, "comment": reason});
                        let _ = client.post_json(&path, &body).await;
                        load.emit(());
                    }
                });
            })
        }
    };

    let content = match (*detail).clone() {
        None => html! { <div class="state">{"Loading…"}</div> },
        Some(Err(e)) => html! { <div class="state error">{ e }</div> },
        Some(Ok(d)) => html! {
            <>
                <div class="page-header">
                    <h1>{ format!("{} — {}", d.ref_code, d.status) }</h1>
                    <p>{ format!("Requester: {} · needed by {}", d.requester_email, d.needed_by) }</p>
                </div>
                <div class="grid-2">
                    <Card title={AttrValue::from("Lines")}>
                        <table class="table">
                            <thead><tr><th>{"Item"}</th><th>{"Qty"}</th><th>{"Unit"}</th><th>{"Total"}</th><th>{"On hand"}</th></tr></thead>
                            <tbody>
                            { for d.lines.iter().map(|l| html!{
                                <tr>
                                    <td>{ &l.item_name }<br/><small class="muted">{ &l.sku }{ if l.controlled { " · controlled" } else { "" } }</small></td>
                                    <td>{ l.quantity }</td>
                                    <td>{ cents_fmt(l.unit_price_cents) }</td>
                                    <td>{ cents_fmt(l.line_total_cents) }</td>
                                    <td>{ l.on_hand }</td>
                                </tr>
                            })}
                            </tbody>
                        </table>
                        <p style="margin-top:12px;"><strong>{"Total: "}</strong>{ cents_fmt(d.total_amount_cents) }</p>
                        <p class="muted">{ format!("Justification: {}", d.justification) }</p>
                    </Card>
                    <Card title={AttrValue::from("Approval route")}>
                        <ol>
                        { for d.route.iter().map(|s| html!{
                            <li>
                                <strong>{ &s.label }</strong>{" — "}{ &s.required_role }
                                { if let Some(dec) = &s.decision { html!{ <span>{ format!(" · {}", dec) }</span> } } else { html!{ <span class="muted">{" · pending"}</span> } } }
                                { if let Some(r) = &s.reason { html!{ <p class="muted">{ r }</p> } } else { Html::default() } }
                            </li>
                        })}
                        </ol>
                    </Card>
                </div>
                <Card title={AttrValue::from("Audit timeline (immutable)")}>
                    <ul>
                    { for d.audit.iter().map(|a| html!{
                        <li>
                            <code>{ &a.occurred_at }</code>{" · "}
                            <strong>{ &a.event_kind }</strong>
                            { if let Some(c) = &a.comment { html!{ <span>{ format!(" — {}", c) }</span> } } else { Html::default() } }
                        </li>
                    })}
                    </ul>
                </Card>
                <Card title={AttrValue::from("Actions")}>
                    <div class="form-field">
                        <label>{"Reason / comment"}</label>
                        <input type="text" value={(*reason).clone()} oninput={
                            let r = reason.clone();
                            Callback::from(move |e: InputEvent| {
                                let i: web_sys::HtmlInputElement = e.target_unchecked_into();
                                r.set(i.value());
                            })
                        } />
                    </div>
                    <div style="display:flex;gap:12px;flex-wrap:wrap;">
                        <button class="button" onclick={action("approve")}>{"Approve"}</button>
                        <button class="button secondary" onclick={action("send-back")}>{"Send back"}</button>
                        <button class="button secondary" onclick={action("reject")}>{"Reject"}</button>
                        <button class="button secondary" onclick={action("withdraw")}>{"Withdraw"}</button>
                    </div>
                </Card>
            </>
        },
    };
    html! { <>{ content }</> }
}
