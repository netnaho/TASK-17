/// Content moderation queue — review flagged content, manage keyword policies.
/// Requires moderation:review or admin:users permission.
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::{card::Card, utils::fmt_date};

// ── DTOs ──────────────────────────────────────────────────────────────────
#[derive(Clone, PartialEq, Deserialize, Debug)]
struct ModerationQueueItem {
    id: String,
    content_type: String,
    content_snippet: String,
    matched_keywords: Vec<String>,
    severity: String,
    status: String,
    flagged_at: String,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct KeywordPolicy {
    id: String,
    keyword: String,
    severity: String,
    is_active: bool,
    created_at: String,
}

#[derive(Clone, PartialEq, Serialize)]
struct ReviewInput {
    action: String,
    note: Option<String>,
}

#[derive(Clone, PartialEq, Serialize)]
struct CreatePolicyInput {
    keyword: String,
    severity: String,
}

#[derive(Clone, PartialEq, Debug)]
enum Tab {
    Queue,
    Policies,
}

fn severity_color(s: &str) -> &str {
    match s {
        "high" => "#ef4444",
        "medium" => "#f59e0b",
        _ => "#6b7280",
    }
}

#[function_component(ModerationPage)]
pub fn moderation() -> Html {
    let auth = use_auth();
    let tab = use_state(|| Tab::Queue);
    let status_filter = use_state(|| "pending".to_string());
    let queue = use_state(|| Option::<Result<Vec<ModerationQueueItem>, String>>::None);
    let policies = use_state(|| Option::<Result<Vec<KeywordPolicy>, String>>::None);
    let action_msg = use_state(|| Option::<String>::None);
    let new_keyword = use_state(|| String::new());
    let new_severity = use_state(|| "medium".to_string());

    // Load queue
    let load_queue = {
        let auth = auth.clone();
        let queue = queue.clone();
        let status_filter = status_filter.clone();
        Callback::from(move |_: ()| {
            let auth = auth.clone();
            let queue = queue.clone();
            let status_val = (*status_filter).clone();
            queue.set(None);
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    let url = format!("/api/v1/moderation/queue?status={}", status_val);
                    match client.get(&url).await {
                        Ok(r) if r.ok() => match r.json::<Vec<ModerationQueueItem>>().await {
                            Ok(v) => queue.set(Some(Ok(v))),
                            Err(e) => queue.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => queue.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => queue.set(Some(Err(e))),
                    }
                }
            });
        })
    };

    // Load policies
    let load_policies = {
        let auth = auth.clone();
        let policies = policies.clone();
        Callback::from(move |_: ()| {
            let auth = auth.clone();
            let policies = policies.clone();
            policies.set(None);
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.get("/api/v1/moderation/policies").await {
                        Ok(r) if r.ok() => match r.json::<Vec<KeywordPolicy>>().await {
                            Ok(v) => policies.set(Some(Ok(v))),
                            Err(e) => policies.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => policies.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => policies.set(Some(Err(e))),
                    }
                }
            });
        })
    };

    // Load keyword policies once on mount.
    {
        let load_policies = load_policies.clone();
        use_effect_with((), move |_| {
            load_policies.emit(());
            || ()
        });
    }

    // Reload the moderation queue whenever the status filter changes.
    // Keying the effect on `*status_filter` means it fires on initial mount
    // (with the default "pending" value) AND on every subsequent filter change,
    // after the component has re-rendered with the new state value.
    // This eliminates the race where `on_status_change` called `load_queue`
    // immediately after `status_filter.set()`, reading the stale pre-update
    // value from the closure-captured state handle.
    {
        let load_queue = load_queue.clone();
        use_effect_with((*status_filter).clone(), move |_| {
            load_queue.emit(());
            || ()
        });
    }

    let review = {
        let auth = auth.clone();
        let action_msg = action_msg.clone();
        let load_queue = load_queue.clone();
        move |item_id: String, action: &'static str| {
            let auth = auth.clone();
            let action_msg = action_msg.clone();
            let load_queue = load_queue.clone();
            Callback::from(move |_: MouseEvent| {
                let auth = auth.clone();
                let item_id = item_id.clone();
                let action_msg = action_msg.clone();
                let load_queue = load_queue.clone();
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        let url = format!("/api/v1/moderation/queue/{}/review", item_id);
                        let body = ReviewInput { action: action.to_string(), note: None };
                        match client.post_json(&url, &body).await {
                            Ok(r) if r.ok() => {
                                action_msg.set(Some(format!("Item {action}d.")));
                                load_queue.emit(());
                            }
                            Ok(r) => action_msg.set(Some(format!("Error: HTTP {}", r.status()))),
                            Err(e) => action_msg.set(Some(format!("Error: {e}"))),
                        }
                    }
                });
            })
        }
    };

    let delete_policy = {
        let auth = auth.clone();
        let action_msg = action_msg.clone();
        let load_policies = load_policies.clone();
        move |policy_id: String| {
            let auth = auth.clone();
            let action_msg = action_msg.clone();
            let load_policies = load_policies.clone();
            Callback::from(move |_: MouseEvent| {
                let auth = auth.clone();
                let policy_id = policy_id.clone();
                let action_msg = action_msg.clone();
                let load_policies = load_policies.clone();
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        let url = format!("/api/v1/moderation/policies/{}/delete", policy_id);
                        match client.post_json(&url, &serde_json::json!({})).await {
                            Ok(_) => {
                                action_msg.set(Some("Keyword policy deleted.".into()));
                                load_policies.emit(());
                            }
                            Err(e) => action_msg.set(Some(format!("Error: {e}"))),
                        }
                    }
                });
            })
        }
    };

    let on_create_policy = {
        let auth = auth.clone();
        let new_keyword = new_keyword.clone();
        let new_severity = new_severity.clone();
        let action_msg = action_msg.clone();
        let load_policies = load_policies.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let kw = (*new_keyword).clone();
            let sev = (*new_severity).clone();
            if kw.trim().is_empty() { return; }
            let auth = auth.clone();
            let action_msg = action_msg.clone();
            let load_policies = load_policies.clone();
            let new_keyword = new_keyword.clone();
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    let body = CreatePolicyInput { keyword: kw.trim().to_string(), severity: sev };
                    match client.post_json("/api/v1/moderation/policies", &body).await {
                        Ok(r) if r.ok() => {
                            action_msg.set(Some("Keyword policy created.".into()));
                            new_keyword.set(String::new());
                            load_policies.emit(());
                        }
                        Ok(r) => action_msg.set(Some(format!("Error: HTTP {}", r.status()))),
                        Err(e) => action_msg.set(Some(format!("Error: {e}"))),
                    }
                }
            });
        })
    };

    let on_status_change = {
        let status_filter = status_filter.clone();
        // `load_queue` is NOT captured here.  Updating `status_filter` triggers a
        // re-render, which causes the `use_effect_with(status_filter)` effect above
        // to fire with the fresh value — no stale-closure race.
        Callback::from(move |e: Event| {
            let el: web_sys::HtmlInputElement = e.target_unchecked_into();
            status_filter.set(el.value());
        })
    };

    html! {
        <>
            <div class="page-header">
                <h1>{"Content Moderation"}</h1>
                <p>{"Review flagged content and manage keyword policies."}</p>
            </div>

            // Tab bar
            <Card>
                <div style="display:flex;gap:0.5rem;margin-bottom:1rem;">
                    <button
                        class={if *tab == Tab::Queue { "button" } else { "button button--ghost" }}
                        onclick={Callback::from({let tab=tab.clone(); move |_| tab.set(Tab::Queue)})}
                    >{"Moderation Queue"}</button>
                    <button
                        class={if *tab == Tab::Policies { "button" } else { "button button--ghost" }}
                        onclick={Callback::from({let tab=tab.clone(); let lp=load_policies.clone(); move |_| { tab.set(Tab::Policies); lp.emit(()); }})}
                    >{"Keyword Policies"}</button>
                </div>

                if let Some(ref msg) = *action_msg {
                    <div style="margin-bottom:1rem;padding:0.5rem;background:#f0fff0;border-radius:4px;font-size:0.9rem;">
                        {msg}
                    </div>
                }

                { match *tab {
                    Tab::Queue => html! {
                        <>
                            <div style="display:flex;align-items:center;gap:1rem;margin-bottom:1rem;">
                                <label>{"Status:"}</label>
                                <select class="input" style="width:auto;" onchange={on_status_change}>
                                    <option value="pending" selected={*status_filter == "pending"}>{"Pending"}</option>
                                    <option value="approved" selected={*status_filter == "approved"}>{"Approved"}</option>
                                    <option value="rejected" selected={*status_filter == "rejected"}>{"Rejected"}</option>
                                    <option value="escalated" selected={*status_filter == "escalated"}>{"Escalated"}</option>
                                </select>
                            </div>
                            { match (*queue).clone() {
                                None => html! { <div style="color:#888;">{"Loading…"}</div> },
                                Some(Err(ref e)) => html! { <div style="color:red;">{e}</div> },
                                Some(Ok(ref items)) if items.is_empty() => html! {
                                    <p style="color:#888;text-align:center;padding:2rem;">
                                        {format!("No {} items in the moderation queue.", *status_filter)}
                                    </p>
                                },
                                Some(Ok(ref items)) => html! {
                                    <div style="overflow-x:auto;">
                                    <table class="table" style="width:100%;">
                                        <thead><tr>
                                            <th>{"Type"}</th><th>{"Snippet"}</th>
                                            <th>{"Keywords"}</th><th>{"Severity"}</th>
                                            <th>{"Flagged"}</th><th>{"Actions"}</th>
                                        </tr></thead>
                                        <tbody>
                                        { for items.iter().map(|item| {
                                            let approve = review(item.id.clone(), "approve");
                                            let reject = review(item.id.clone(), "reject");
                                            let escalate = review(item.id.clone(), "escalate");
                                            let color = severity_color(&item.severity);
                                            html! { <tr>
                                                <td style="font-size:0.85rem;">{&item.content_type}</td>
                                                <td style="max-width:250px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:0.85rem;">
                                                    {&item.content_snippet}
                                                </td>
                                                <td>
                                                    { for item.matched_keywords.iter().map(|kw| html! {
                                                        <span style="background:#fee2e2;color:#dc2626;padding:0.1rem 0.3rem;border-radius:3px;font-size:0.75rem;margin-right:0.25rem;">
                                                            {kw}
                                                        </span>
                                                    })}
                                                </td>
                                                <td><span style={format!("color:{color};font-weight:bold;")}>{&item.severity}</span></td>
                                                <td style="font-size:0.8rem;">{fmt_date(&item.flagged_at)}</td>
                                                <td>
                                                    if item.status == "pending" {
                                                        <div style="display:flex;gap:0.25rem;">
                                                            <button class="button button--sm" onclick={approve}>{"Approve"}</button>
                                                            <button class="button button--danger button--sm" onclick={reject}>{"Reject"}</button>
                                                            <button class="button button--ghost button--sm" onclick={escalate}>{"Escalate"}</button>
                                                        </div>
                                                    }
                                                </td>
                                            </tr> }
                                        })}
                                        </tbody>
                                    </table>
                                    </div>
                                },
                            }}
                        </>
                    },
                    Tab::Policies => html! {
                        <>
                            // Create new keyword form
                            <form onsubmit={on_create_policy} style="display:flex;gap:0.5rem;margin-bottom:1rem;flex-wrap:wrap;">
                                <input
                                    class="input"
                                    placeholder="New keyword…"
                                    value={(*new_keyword).clone()}
                                    oninput={Callback::from({let nk=new_keyword.clone(); move |e: InputEvent| {
                                        let el: web_sys::HtmlInputElement = e.target_unchecked_into();
                                        nk.set(el.value());
                                    }})}
                                    style="flex:1;min-width:150px;"
                                />
                                <select class="input" style="width:auto;" onchange={Callback::from({let ns=new_severity.clone(); move |e: Event| {
                                    let el: web_sys::HtmlInputElement = e.target_unchecked_into();
                                    ns.set(el.value());
                                }})}>
                                    <option value="low">{"Low"}</option>
                                    <option value="medium" selected=true>{"Medium"}</option>
                                    <option value="high">{"High"}</option>
                                </select>
                                <button class="button" type="submit">{"Add Keyword"}</button>
                            </form>
                            { match (*policies).clone() {
                                None => html! { <div style="color:#888;">{"Loading…"}</div> },
                                Some(Err(ref e)) => html! { <div style="color:red;">{e}</div> },
                                Some(Ok(ref pols)) if pols.is_empty() => html! {
                                    <p style="color:#888;">{"No keyword policies configured."}</p>
                                },
                                Some(Ok(ref pols)) => html! {
                                    <table class="table" style="width:100%;">
                                        <thead><tr>
                                            <th>{"Keyword"}</th><th>{"Severity"}</th>
                                            <th>{"Active"}</th><th>{"Created"}</th><th></th>
                                        </tr></thead>
                                        <tbody>
                                        { for pols.iter().map(|p| {
                                            let del = delete_policy(p.id.clone());
                                            html! { <tr>
                                                <td><code>{&p.keyword}</code></td>
                                                <td style={format!("color:{};font-weight:bold;", severity_color(&p.severity))}>
                                                    {&p.severity}
                                                </td>
                                                <td>{if p.is_active { "Yes" } else { "No" }}</td>
                                                <td style="font-size:0.8rem;">{fmt_date(&p.created_at)}</td>
                                                <td>
                                                    <button class="button button--danger button--sm" onclick={del}>
                                                        {"Delete"}
                                                    </button>
                                                </td>
                                            </tr> }
                                        })}
                                        </tbody>
                                    </table>
                                },
                            }}
                        </>
                    },
                }}
            </Card>
        </>
    }
}
