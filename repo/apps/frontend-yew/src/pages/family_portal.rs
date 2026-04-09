/// Family portal — consent-gated views of resident supply usage and wellness summaries.
///
/// Guardians see only what they have explicitly consented to sharing.
/// No internal notes, no admin fields, no raw identifiers are exposed.
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::card::Card;

// ── DTOs ──────────────────────────────────────────────────────────────────
#[derive(Clone, PartialEq, Deserialize, Debug)]
struct ResidentView {
    student_id: String,
    first_name: String,
    last_name: String,
    class_code: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct ConsentView {
    data_category: String,
    consented: bool,
    consented_at: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct FamilyGroupView {
    id: String,
    label: String,
    institution_id: String,
    residents: Vec<ResidentView>,
    consent: Vec<ConsentView>,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct SupplySummaryItem {
    resident_first_name: String,
    resident_last_name: String,
    period_month: String,
    category: String,
    approved_count: i64,
    total_quantity: i64,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct WellnessSummaryItem {
    resident_first_name: String,
    resident_last_name: String,
    activity_type: String,
    activity_date: String,
    duration_minutes: i32,
    is_personal_best: bool,
}

#[derive(Clone, PartialEq, Serialize)]
struct ConsentInput {
    data_category: String,
    consented: bool,
}

// ── active view for a group ────────────────────────────────────────────────
#[derive(Clone, PartialEq, Debug)]
enum GroupView {
    Overview,
    SupplySummary,
    WellnessSummary,
}

// ── component ──────────────────────────────────────────────────────────────
#[function_component(FamilyPortalPage)]
pub fn family_portal() -> Html {
    let auth = use_auth();
    let groups = use_state(|| Option::<Result<Vec<FamilyGroupView>, String>>::None);
    let selected_group = use_state(|| Option::<FamilyGroupView>::None);
    let group_view = use_state(|| GroupView::Overview);
    let supply_data = use_state(|| Option::<Result<Vec<SupplySummaryItem>, String>>::None);
    let wellness_data = use_state(|| Option::<Result<Vec<WellnessSummaryItem>, String>>::None);
    let consent_msg = use_state(|| Option::<String>::None);

    // Load family groups on mount
    {
        let auth = auth.clone();
        let groups = groups.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.get("/api/v1/family/groups").await {
                        Ok(r) if r.ok() => match r.json::<Vec<FamilyGroupView>>().await {
                            Ok(v) => groups.set(Some(Ok(v))),
                            Err(e) => groups.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => groups.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => groups.set(Some(Err(e.to_string()))),
                    }
                }
            });
            || ()
        });
    }

    let load_supply = {
        let auth = auth.clone();
        let supply_data = supply_data.clone();
        let selected_group = selected_group.clone();
        let group_view = group_view.clone();
        Callback::from(move |_: MouseEvent| {
            let group_id = (*selected_group).as_ref().map(|g| g.id.clone());
            let Some(gid) = group_id else { return; };
            let auth = auth.clone();
            let supply_data = supply_data.clone();
            let group_view = group_view.clone();
            group_view.set(GroupView::SupplySummary);
            supply_data.set(None);
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    let url = format!("/api/v1/family/groups/{}/supply-summary", gid);
                    match client.get(&url).await {
                        Ok(r) if r.ok() => match r.json::<Vec<SupplySummaryItem>>().await {
                            Ok(v) => supply_data.set(Some(Ok(v))),
                            Err(e) => supply_data.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => {
                            let msg = r.text().await.unwrap_or_default();
                            supply_data.set(Some(Err(format!("HTTP {}: {}", r.status(), msg))));
                        },
                        Err(e) => supply_data.set(Some(Err(e))),
                    }
                }
            });
        })
    };

    let load_wellness = {
        let auth = auth.clone();
        let wellness_data = wellness_data.clone();
        let selected_group = selected_group.clone();
        let group_view = group_view.clone();
        Callback::from(move |_: MouseEvent| {
            let group_id = (*selected_group).as_ref().map(|g| g.id.clone());
            let Some(gid) = group_id else { return; };
            let auth = auth.clone();
            let wellness_data = wellness_data.clone();
            let group_view = group_view.clone();
            group_view.set(GroupView::WellnessSummary);
            wellness_data.set(None);
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    let url = format!("/api/v1/family/groups/{}/wellness-summary", gid);
                    match client.get(&url).await {
                        Ok(r) if r.ok() => match r.json::<Vec<WellnessSummaryItem>>().await {
                            Ok(v) => wellness_data.set(Some(Ok(v))),
                            Err(e) => wellness_data.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => {
                            let msg = r.text().await.unwrap_or_default();
                            wellness_data.set(Some(Err(format!("HTTP {}: {}", r.status(), msg))));
                        },
                        Err(e) => wellness_data.set(Some(Err(e))),
                    }
                }
            });
        })
    };

    let make_consent_toggle = {
        let auth = auth.clone();
        let selected_group = selected_group.clone();
        let consent_msg = consent_msg.clone();
        let groups = groups.clone();
        move |category: String, currently_consented: bool| {
            let auth = auth.clone();
            let selected_group = selected_group.clone();
            let consent_msg = consent_msg.clone();
            let groups = groups.clone();
            Callback::from(move |_: MouseEvent| {
                let group_id = (*selected_group).as_ref().map(|g| g.id.clone());
                let Some(gid) = group_id else { return; };
                let auth = auth.clone();
                let category = category.clone();
                let consent_msg = consent_msg.clone();
                let groups = groups.clone();
                let new_consented = !currently_consented;
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        let url = format!("/api/v1/family/groups/{}/consent", gid);
                        let body = ConsentInput { data_category: category.clone(), consented: new_consented };
                        match client.post_json(&url, &body).await {
                            Ok(r) if r.ok() => {
                                consent_msg.set(Some(format!(
                                    "{} consent for '{}' {}.",
                                    if new_consented { "Granted" } else { "Revoked" },
                                    category,
                                    if new_consented { "successfully" } else { "successfully" }
                                )));
                                // Reload groups to update consent badges
                                if let Ok(r2) = client.get("/api/v1/family/groups").await {
                                    if let Ok(v) = r2.json::<Vec<FamilyGroupView>>().await {
                                        groups.set(Some(Ok(v)));
                                    }
                                }
                            }
                            Ok(r) => consent_msg.set(Some(format!("Error: HTTP {}", r.status()))),
                            Err(e) => consent_msg.set(Some(format!("Error: {e}"))),
                        }
                    }
                });
            })
        }
    };

    html! {
        <>
            <div class="page-header">
                <h1>{"Family Portal"}</h1>
                <p>{"Consent-gated summaries of resident activity. Only approved data is visible."}</p>
            </div>

            <div style="display:grid;grid-template-columns:280px 1fr;gap:1rem;align-items:start;">
                // Left: group list
                <div>
                    <Card>
                        <h3 style="margin-top:0;">{"My Family Groups"}</h3>
                        { match (*groups).clone() {
                            None => html! { <div style="color:#888;">{"Loading…"}</div> },
                            Some(Err(ref e)) => html! { <div style="color:red;">{e}</div> },
                            Some(Ok(ref glist)) if glist.is_empty() => html! {
                                <div style="color:#888;font-size:0.9rem;">
                                    {"No family groups assigned. Ask an administrator to add you."}
                                </div>
                            },
                            Some(Ok(ref glist)) => html! {
                                <ul style="list-style:none;padding:0;margin:0;">
                                { for glist.iter().map(|g| {
                                    let is_selected = (*selected_group).as_ref().map(|s| s.id == g.id).unwrap_or(false);
                                    let group_clone = g.clone();
                                    let selected_group = selected_group.clone();
                                    let group_view = group_view.clone();
                                    html! {
                                        <li
                                            onclick={Callback::from(move |_| {
                                                selected_group.set(Some(group_clone.clone()));
                                                group_view.set(GroupView::Overview);
                                            })}
                                            style={format!(
                                                "padding:0.75rem;cursor:pointer;border-radius:4px;margin-bottom:0.25rem;background:{};",
                                                if is_selected { "#e8f4fd" } else { "transparent" }
                                            )}
                                        >
                                            <strong>{&g.label}</strong>
                                            <div style="font-size:0.8rem;color:#666;margin-top:0.2rem;">
                                                {format!("{} resident(s)", g.residents.len())}
                                            </div>
                                        </li>
                                    }
                                })}
                                </ul>
                            },
                        }}
                    </Card>
                </div>

                // Right: group detail
                <div>
                { if let Some(ref group) = *selected_group {
                    html! {
                        <>
                            <Card>
                                <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem;">
                                    <h2 style="margin:0;">{&group.label}</h2>
                                    <div style="display:flex;gap:0.5rem;">
                                        <button class="button button--secondary" onclick={load_supply.clone()}>
                                            {"Supply Summary"}
                                        </button>
                                        <button class="button button--secondary" onclick={load_wellness.clone()}>
                                            {"Wellness Summary"}
                                        </button>
                                    </div>
                                </div>

                                // Consent management
                                <div style="margin-bottom:1.5rem;">
                                    <h4 style="margin:0 0 0.5rem;">{"Data Sharing Consent"}</h4>
                                    <p style="font-size:0.85rem;color:#666;margin:0 0 0.75rem;">
                                        {"You control which data categories are visible. Toggle consent below."}
                                    </p>
                                    <div style="display:flex;gap:1rem;flex-wrap:wrap;">
                                    { for [
                                        ("supply_usage", "Supply Usage"),
                                        ("wellness_summary", "Wellness Summary"),
                                        ("activity_log", "Activity Log"),
                                    ].iter().map(|(cat, label)| {
                                        let consented = group.consent.iter()
                                            .find(|c| c.data_category == *cat)
                                            .map(|c| c.consented)
                                            .unwrap_or(false);
                                        let toggle = make_consent_toggle(cat.to_string(), consented);
                                        html! {
                                            <div style="display:flex;align-items:center;gap:0.5rem;">
                                                <button
                                                    class={if consented { "button" } else { "button button--ghost" }}
                                                    onclick={toggle}
                                                    style="font-size:0.8rem;"
                                                >
                                                    {if consented { format!("✓ {label}") } else { format!("○ {label}") }}
                                                </button>
                                            </div>
                                        }
                                    })}
                                    </div>
                                    if let Some(ref msg) = *consent_msg {
                                        <div style="margin-top:0.5rem;font-size:0.85rem;color:#059669;">{msg}</div>
                                    }
                                </div>

                                // Residents list
                                <h4 style="margin:0 0 0.5rem;">{"Residents"}</h4>
                                if group.residents.is_empty() {
                                    <p style="color:#888;font-size:0.9rem;">{"No residents linked to this group."}</p>
                                } else {
                                    <ul style="list-style:none;padding:0;margin:0;">
                                    { for group.residents.iter().map(|r| html! {
                                        <li style="padding:0.4rem 0;border-bottom:1px solid #f0f0f0;">
                                            <strong>{format!("{} {}", r.first_name, r.last_name)}</strong>
                                            if let Some(ref cc) = r.class_code {
                                                <span style="margin-left:0.5rem;font-size:0.85rem;color:#666;">
                                                    {format!("Class: {cc}")}
                                                </span>
                                            }
                                        </li>
                                    })}
                                    </ul>
                                }
                            </Card>

                            // Supply or wellness detail view
                            { match *group_view {
                                GroupView::Overview => html! {},
                                GroupView::SupplySummary => html! {
                                    <Card>
                                        <h3 style="margin-top:0;">{"Supply Usage (last 6 months)"}</h3>
                                        { match (*supply_data).clone() {
                                            None => html! { <div style="color:#888;">{"Loading…"}</div> },
                                            Some(Err(ref e)) => html! {
                                                <div style="color:red;">{e}
                                                    <p style="color:#666;font-size:0.9rem;">
                                                        {"(Enable 'Supply Usage' consent above to view this data.)"}
                                                    </p>
                                                </div>
                                            },
                                            Some(Ok(ref items)) if items.is_empty() => html! {
                                                <p style="color:#888;">{"No approved supply usage in the past 6 months."}</p>
                                            },
                                            Some(Ok(ref items)) => html! {
                                                <div style="overflow-x:auto;">
                                                <table class="table" style="width:100%;">
                                                    <thead><tr>
                                                        <th>{"Resident"}</th><th>{"Month"}</th>
                                                        <th>{"Category"}</th><th>{"Approved Reqs"}</th>
                                                    </tr></thead>
                                                    <tbody>
                                                    { for items.iter().map(|i| html! {
                                                        <tr>
                                                            <td>{format!("{} {}", i.resident_first_name, i.resident_last_name)}</td>
                                                            <td>{&i.period_month}</td>
                                                            <td>{&i.category}</td>
                                                            <td>{i.approved_count}</td>
                                                        </tr>
                                                    })}
                                                    </tbody>
                                                </table>
                                                </div>
                                            },
                                        }}
                                    </Card>
                                },
                                GroupView::WellnessSummary => html! {
                                    <Card>
                                        <h3 style="margin-top:0;">{"Wellness Activities (last 30 days)"}</h3>
                                        { match (*wellness_data).clone() {
                                            None => html! { <div style="color:#888;">{"Loading…"}</div> },
                                            Some(Err(ref e)) => html! {
                                                <div style="color:red;">{e}
                                                    <p style="color:#666;font-size:0.9rem;">
                                                        {"(Enable 'Wellness Summary' consent above to view this data.)"}
                                                    </p>
                                                </div>
                                            },
                                            Some(Ok(ref items)) if items.is_empty() => html! {
                                                <p style="color:#888;">{"No wellness activities logged in the past 30 days."}</p>
                                            },
                                            Some(Ok(ref items)) => html! {
                                                <div style="overflow-x:auto;">
                                                <table class="table" style="width:100%;">
                                                    <thead><tr>
                                                        <th>{"Resident"}</th><th>{"Date"}</th>
                                                        <th>{"Activity"}</th><th>{"Duration"}</th><th>{"PB?"}</th>
                                                    </tr></thead>
                                                    <tbody>
                                                    { for items.iter().map(|i| html! {
                                                        <tr>
                                                            <td>{format!("{} {}", i.resident_first_name, i.resident_last_name)}</td>
                                                            <td>{&i.activity_date}</td>
                                                            <td style="text-transform:capitalize;">{&i.activity_type}</td>
                                                            <td>{format!("{} min", i.duration_minutes)}</td>
                                                            <td>{if i.is_personal_best { "🏆" } else { "" }}</td>
                                                        </tr>
                                                    })}
                                                    </tbody>
                                                </table>
                                                </div>
                                            },
                                        }}
                                    </Card>
                                },
                            }}
                        </>
                    }
                } else {
                    html! {
                        <Card>
                            <p style="color:#888;text-align:center;padding:2rem;">
                                {"Select a family group on the left to view resident summaries."}
                            </p>
                        </Card>
                    }
                }}
                </div>
            </div>
        </>
    }
}
