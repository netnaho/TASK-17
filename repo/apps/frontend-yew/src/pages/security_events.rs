/// Security & Anomaly Events — admin view of detected anomalies.
/// Shows unacknowledged events first, allows acknowledgement.
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::{card::Card, utils::fmt_datetime};

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct AnomalyEvent {
    id: String,
    rule_key: String,
    severity: String,
    subject_type: String,
    subject_value: String,
    detail: serde_json::Value,
    detected_at: String,
    acknowledged: bool,
    acknowledged_at: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct AnomalyRule {
    rule_key: String,
    label: String,
    description: String,
    threshold_count: i32,
    window_seconds: i32,
    severity: String,
    is_active: bool,
}

fn severity_badge(s: &str) -> Html {
    let (bg, fg) = match s {
        "critical" => ("#7f1d1d", "white"),
        "high" => ("#ef4444", "white"),
        "medium" => ("#f59e0b", "white"),
        _ => ("#6b7280", "white"),
    };
    html! {
        <span style={format!("background:{bg};color:{fg};padding:0.15rem 0.4rem;border-radius:3px;font-size:0.75rem;font-weight:bold;")}>
            {s.to_uppercase()}
        </span>
    }
}

#[function_component(SecurityEventsPage)]
pub fn security_events() -> Html {
    let auth = use_auth();
    let show_all = use_state(|| false);
    let events = use_state(|| Option::<Result<Vec<AnomalyEvent>, String>>::None);
    let rules = use_state(|| Option::<Result<Vec<AnomalyRule>, String>>::None);
    let ack_msg = use_state(|| Option::<String>::None);
    let show_rules = use_state(|| false);

    let load_events = {
        let auth = auth.clone();
        let events = events.clone();
        let show_all = show_all.clone();
        Callback::from(move |_: ()| {
            let auth = auth.clone();
            let events = events.clone();
            let unacked = !*show_all;
            events.set(None);
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    let url = format!("/api/v1/anomaly/events?unacked={}", unacked);
                    match client.get(&url).await {
                        Ok(r) if r.ok() => match r.json::<Vec<AnomalyEvent>>().await {
                            Ok(v) => events.set(Some(Ok(v))),
                            Err(e) => events.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => events.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => events.set(Some(Err(e))),
                    }
                }
            });
        })
    };

    // Load rules
    {
        let auth = auth.clone();
        let rules = rules.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.get("/api/v1/anomaly/rules").await {
                        Ok(r) if r.ok() => match r.json::<Vec<AnomalyRule>>().await {
                            Ok(v) => rules.set(Some(Ok(v))),
                            Err(e) => rules.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => rules.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => rules.set(Some(Err(e))),
                    }
                }
            });
            || ()
        });
    }

    // Initial events load
    {
        let load_events = load_events.clone();
        use_effect_with((), move |_| {
            load_events.emit(());
            || ()
        });
    }

    let acknowledge = {
        let auth = auth.clone();
        let ack_msg = ack_msg.clone();
        let load_events = load_events.clone();
        move |event_id: String| {
            let auth = auth.clone();
            let ack_msg = ack_msg.clone();
            let load_events = load_events.clone();
            Callback::from(move |_: MouseEvent| {
                let auth = auth.clone();
                let event_id = event_id.clone();
                let ack_msg = ack_msg.clone();
                let load_events = load_events.clone();
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        let url = format!("/api/v1/anomaly/events/{}/acknowledge", event_id);
                        match client.post_json(&url, &serde_json::json!({})).await {
                            Ok(r) if r.ok() => {
                                ack_msg.set(Some("Event acknowledged.".into()));
                                load_events.emit(());
                            }
                            Ok(r) => ack_msg.set(Some(format!("Error: HTTP {}", r.status()))),
                            Err(e) => ack_msg.set(Some(format!("Error: {e}"))),
                        }
                    }
                });
            })
        }
    };

    let toggle_all = {
        let show_all = show_all.clone();
        let load_events = load_events.clone();
        Callback::from(move |_: MouseEvent| {
            show_all.set(!*show_all);
            load_events.emit(());
        })
    };

    html! {
        <>
            <div class="page-header">
                <h1>{"Security & Anomaly Events"}</h1>
                <p>{"Rule-based anomaly detection events. Acknowledge and investigate suspicious activity."}</p>
            </div>

            <Card>
                <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem;flex-wrap:wrap;gap:0.5rem;">
                    <div style="display:flex;gap:0.5rem;">
                        <button
                            class={if !*show_all { "button" } else { "button button--ghost" }}
                            onclick={Callback::from({let sa=show_all.clone(); let le=load_events.clone(); move |_| { sa.set(false); le.emit(()); }})}
                        >{"Unacknowledged"}</button>
                        <button
                            class={if *show_all { "button" } else { "button button--ghost" }}
                            onclick={toggle_all}
                        >{"All Events"}</button>
                    </div>
                    <button
                        class="button button--ghost"
                        onclick={Callback::from({let sr=show_rules.clone(); move |_| sr.set(!*sr)})}
                    >
                        {if *show_rules { "Hide Rules" } else { "Show Detection Rules" }}
                    </button>
                </div>

                if let Some(ref msg) = *ack_msg {
                    <div style="margin-bottom:1rem;padding:0.5rem;background:#f0fff0;border-radius:4px;font-size:0.9rem;">
                        {msg}
                    </div>
                }

                { match (*events).clone() {
                    None => html! { <div style="text-align:center;padding:2rem;color:#888;">{"Loading…"}</div> },
                    Some(Err(ref e)) => html! { <div style="color:red;padding:1rem;">{e}</div> },
                    Some(Ok(ref evts)) if evts.is_empty() => html! {
                        <div style="text-align:center;padding:2rem;color:#059669;">
                            <div style="font-size:2rem;">{"✓"}</div>
                            <div>{"No anomaly events to review."}</div>
                        </div>
                    },
                    Some(Ok(ref evts)) => html! {
                        <div style="overflow-x:auto;">
                        <table class="table" style="width:100%;">
                            <thead><tr>
                                <th>{"Severity"}</th><th>{"Rule"}</th>
                                <th>{"Subject"}</th><th>{"Detail"}</th>
                                <th>{"Detected"}</th><th>{"Status"}</th><th></th>
                            </tr></thead>
                            <tbody>
                            { for evts.iter().map(|ev| {
                                let ack_cb = acknowledge(ev.id.clone());
                                let detail_str = ev.detail.to_string();
                                html! { <tr>
                                    <td>{severity_badge(&ev.severity)}</td>
                                    <td style="font-size:0.85rem;max-width:200px;">
                                        <code style="font-size:0.75rem;">{&ev.rule_key}</code>
                                    </td>
                                    <td style="font-size:0.85rem;">
                                        <span style="color:#6b7280;">{format!("{}: ", ev.subject_type)}</span>
                                        <strong>{&ev.subject_value}</strong>
                                    </td>
                                    <td style="font-size:0.75rem;max-width:200px;overflow:hidden;text-overflow:ellipsis;color:#6b7280;">
                                        {detail_str}
                                    </td>
                                    <td style="font-size:0.8rem;white-space:nowrap;">
                                        {fmt_datetime(&ev.detected_at)}
                                    </td>
                                    <td>
                                        if ev.acknowledged {
                                            <span style="color:#059669;font-size:0.8rem;">{"Acknowledged"}</span>
                                        } else {
                                            <span style="color:#ef4444;font-size:0.8rem;font-weight:bold;">{"Pending"}</span>
                                        }
                                    </td>
                                    <td>
                                        if !ev.acknowledged {
                                            <button class="button button--sm" onclick={ack_cb}>{"Acknowledge"}</button>
                                        }
                                    </td>
                                </tr> }
                            })}
                            </tbody>
                        </table>
                        </div>
                    },
                }}

                // Detection rules panel
                if *show_rules {
                    <div style="margin-top:2rem;border-top:1px solid #eee;padding-top:1rem;">
                        <h3 style="margin-top:0;">{"Active Detection Rules"}</h3>
                        { match (*rules).clone() {
                            None | Some(Err(_)) => html! { <div style="color:#888;">{"Loading…"}</div> },
                            Some(Ok(ref rule_list)) => html! {
                                <table class="table" style="width:100%;">
                                    <thead><tr>
                                        <th>{"Rule"}</th><th>{"Description"}</th>
                                        <th>{"Threshold"}</th><th>{"Window"}</th><th>{"Severity"}</th>
                                    </tr></thead>
                                    <tbody>
                                    { for rule_list.iter().map(|r| html! {
                                        <tr>
                                            <td><code style="font-size:0.8rem;">{&r.rule_key}</code></td>
                                            <td style="font-size:0.85rem;">{&r.description}</td>
                                            <td>{format!("{} events", r.threshold_count)}</td>
                                            <td>{format!("{}s", r.window_seconds)}</td>
                                            <td>{severity_badge(&r.severity)}</td>
                                        </tr>
                                    })}
                                    </tbody>
                                </table>
                            },
                        }}
                    </div>
                }
            </Card>
        </>
    }
}
