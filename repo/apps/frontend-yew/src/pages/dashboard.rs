/// Operations dashboard — live KPI snapshot drawn from accessible endpoints.
///
/// For admins this fetches the analytics dashboard (live counts + trends).
/// For all roles it fetches the user's own requisitions and orders to build
/// a role-appropriate at-a-glance view.
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::card::Card;
use crate::router::Route;

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct ReqSummary {
    id: String,
    status: String,
    ref_code: String,
    needed_by: String,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct OrderSummary {
    id: String,
    status: String,
    total_cents: i64,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct LiveCounts {
    pending_requisitions: i64,
    open_orders: i64,
    wellness_sessions_this_week: i64,
    moderation_queue_pending: i64,
    anomaly_events_unacked: i64,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct DashboardView {
    live_counts: LiveCounts,
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn status_badge(status: &str) -> Html {
    let (bg, fg) = match status {
        "draft"            => ("#e5e7eb", "#374151"),
        "pending_approval" => ("#fef3c7", "#92400e"),
        "approved_final"   => ("#d1fae5", "#065f46"),
        "issued"           => ("#dbeafe", "#1e40af"),
        "rejected"         => ("#fee2e2", "#991b1b"),
        "withdrawn"        => ("#f3f4f6", "#6b7280"),
        "sent_back"        => ("#ede9fe", "#5b21b6"),
        "confirmed"        => ("#dbeafe", "#1e40af"),
        "pending"          => ("#fef3c7", "#92400e"),
        _                  => ("#f3f4f6", "#374151"),
    };
    html! {
        <span style={format!(
            "background:{bg};color:{fg};padding:0.15rem 0.45rem;\
             border-radius:3px;font-size:0.75rem;font-weight:600;"
        )}>
            {status.replace('_', " ").to_uppercase()}
        </span>
    }
}

fn kpi(label: &str, value: i64, border_color: &str) -> Html {
    html! {
        <div style={format!(
            "border-left:4px solid {border_color};padding:1rem 1.25rem;\
             background:#fff;border-radius:6px;box-shadow:0 1px 3px rgba(0,0,0,.08);"
        )}>
            <div style="font-size:0.78rem;color:#6b7280;text-transform:uppercase;\
                        letter-spacing:0.05em;margin-bottom:0.3rem;">
                {label}
            </div>
            <div style="font-size:2rem;font-weight:700;color:#111827;">{value}</div>
        </div>
    }
}

// ── component ─────────────────────────────────────────────────────────────────

#[function_component(DashboardPage)]
pub fn dashboard() -> Html {
    let auth = use_auth();
    let nav = use_navigator().unwrap();

    let is_admin = auth.has_permission("*") || auth.has_permission("admin:users");
    let can_approve = auth.has_permission("requisitions:approve");
    let can_order = auth.has_permission("orders:read");

    let admin_dash  = use_state(|| Option::<Result<DashboardView, String>>::None);
    let my_reqs     = use_state(|| Option::<Result<Vec<ReqSummary>, String>>::None);
    let my_orders   = use_state(|| Option::<Result<Vec<OrderSummary>, String>>::None);
    let inbox_count = use_state(|| Option::<i64>::None);

    // Load admin dashboard (if permitted)
    {
        let auth = auth.clone();
        let admin_dash = admin_dash.clone();
        let is_admin = is_admin;
        use_effect_with((), move |_| {
            if is_admin {
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        // Fetch institutions first to get an id
                        match client.get("/api/v1/master-data/institutions").await {
                            Ok(r) if r.ok() => {
                                if let Ok(insts) = r.json::<Vec<serde_json::Value>>().await {
                                    if let Some(iid) = insts.first().and_then(|i| i["id"].as_str()) {
                                        let url = format!("/api/v1/analytics/dashboard?institution_id={iid}");
                                        if let Ok(r2) = client.get(&url).await {
                                            if r2.ok() {
                                                match r2.json::<DashboardView>().await {
                                                    Ok(v) => admin_dash.set(Some(Ok(v))),
                                                    Err(e) => admin_dash.set(Some(Err(e.to_string()))),
                                                }
                                            } else {
                                                admin_dash.set(Some(Err(format!("HTTP {}", r2.status()))));
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                });
            }
            || ()
        });
    }

    // Load user's requisitions
    {
        let auth = auth.clone();
        let my_reqs = my_reqs.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.get("/api/v1/requisitions/mine/list").await {
                        Ok(r) if r.ok() => match r.json::<Vec<ReqSummary>>().await {
                            Ok(v) => my_reqs.set(Some(Ok(v))),
                            Err(e) => my_reqs.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => my_reqs.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => my_reqs.set(Some(Err(e))),
                    }
                }
            });
            || ()
        });
    }

    // Load approver inbox count
    {
        let auth = auth.clone();
        let inbox_count = inbox_count.clone();
        let can_approve = can_approve;
        use_effect_with((), move |_| {
            if can_approve {
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        if let Ok(r) = client.get("/api/v1/requisitions/inbox").await {
                            if r.ok() {
                                if let Ok(items) = r.json::<Vec<serde_json::Value>>().await {
                                    inbox_count.set(Some(items.len() as i64));
                                }
                            }
                        }
                    }
                });
            }
            || ()
        });
    }

    // Load orders
    {
        let auth = auth.clone();
        let my_orders = my_orders.clone();
        let can_order = can_order;
        use_effect_with((), move |_| {
            if can_order {
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        match client.get("/api/v1/orders/mine").await {
                            Ok(r) if r.ok() => match r.json::<Vec<OrderSummary>>().await {
                                Ok(v) => my_orders.set(Some(Ok(v))),
                                Err(_) => my_orders.set(Some(Ok(vec![]))),
                            },
                            _ => {}
                        }
                    }
                });
            }
            || ()
        });
    }

    // ── Derived counts ────────────────────────────────────────────────────────
    let (pending_count, draft_count) = match (*my_reqs).clone() {
        Some(Ok(ref reqs)) => (
            reqs.iter().filter(|r| r.status == "pending_approval").count() as i64,
            reqs.iter().filter(|r| r.status == "draft" || r.status == "sent_back").count() as i64,
        ),
        _ => (0, 0),
    };

    let order_count = match (*my_orders).clone() {
        Some(Ok(ref orders)) => orders.len() as i64,
        _ => 0,
    };

    let user_email = auth.principal.as_ref().map(|p| p.email.as_str()).unwrap_or("—");
    let user_roles = auth.principal.as_ref()
        .map(|p| p.roles.join(", "))
        .unwrap_or_default();

    html! {
        <>
            <div class="page-header">
                <h1>{"Operations Dashboard"}</h1>
                <p style="color:#6b7280;">
                    {"Signed in as "}
                    <strong>{user_email}</strong>
                    if !user_roles.is_empty() {
                        <span style="margin-left:0.5rem;font-size:0.85rem;color:#9ca3af;">
                            {"("}{user_roles}{")"}
                        </span>
                    }
                </p>
            </div>

            // ── Admin KPI tiles (if admin + data loaded) ─────────────────────
            if is_admin {
                { match (*admin_dash).clone() {
                    None => html! {
                        <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));\
                                    gap:1rem;margin-bottom:1.5rem;">
                            { for (0..5).map(|_| html! {
                                <div style="height:80px;background:#f3f4f6;border-radius:6px;
                                            animation:pulse 1.5s infinite;"/>
                            })}
                        </div>
                    },
                    Some(Err(ref e)) => html! {
                        <div style="margin-bottom:1rem;padding:0.75rem;background:#fff3cd;
                                    border-radius:4px;font-size:0.85rem;color:#856404;">
                            {"Analytics unavailable: "}{e}
                        </div>
                    },
                    Some(Ok(ref d)) => html! {
                        <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));\
                                    gap:1rem;margin-bottom:1.5rem;">
                            {kpi("Pending Requisitions",    d.live_counts.pending_requisitions,    "#f59e0b")}
                            {kpi("Open Orders",             d.live_counts.open_orders,             "#3b82f6")}
                            {kpi("Wellness (this week)",    d.live_counts.wellness_sessions_this_week, "#10b981")}
                            {kpi("Moderation Queue",        d.live_counts.moderation_queue_pending, "#8b5cf6")}
                            {kpi("Unacked Anomalies",       d.live_counts.anomaly_events_unacked,  "#ef4444")}
                        </div>
                    },
                }}
            }

            // ── Non-admin KPI tiles ───────────────────────────────────────────
            if !is_admin {
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));\
                            gap:1rem;margin-bottom:1.5rem;">
                    {kpi("My Pending Requisitions", pending_count,            "#f59e0b")}
                    {kpi("My Drafts",               draft_count,              "#6b7280")}
                    if can_approve {
                        {kpi("Inbox (pending approval)", (*inbox_count).unwrap_or(0), "#ef4444")}
                    }
                    if can_order {
                        {kpi("My Orders", order_count, "#3b82f6")}
                    }
                </div>
            }

            // ── Quick actions ─────────────────────────────────────────────────
            <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(220px,1fr));\
                        gap:1rem;margin-bottom:1.5rem;">
                <Card>
                    <h3 style="margin:0 0 0.75rem;">{"Requisitions"}</h3>
                    <div style="display:flex;flex-direction:column;gap:0.5rem;">
                        <button class="button"
                            onclick={Callback::from({let nav=nav.clone(); move |_| nav.push(&Route::RequisitionNew)})}>
                            {"+ New Requisition"}
                        </button>
                        <button class="button button--ghost"
                            onclick={Callback::from({let nav=nav.clone(); move |_| nav.push(&Route::Requisitions)})}>
                            {"View All"}
                        </button>
                        if can_approve {
                            <button class="button button--ghost"
                                onclick={Callback::from({let nav=nav.clone(); move |_| nav.push(&Route::Approvals)})}>
                                {"Approvals Inbox"}
                            </button>
                        }
                    </div>
                </Card>

                if can_order {
                    <Card>
                        <h3 style="margin:0 0 0.75rem;">{"Orders"}</h3>
                        <div style="display:flex;flex-direction:column;gap:0.5rem;">
                            <button class="button"
                                onclick={Callback::from({let nav=nav.clone(); move |_| nav.push(&Route::Cart)})}>
                                {"Shop & Cart"}
                            </button>
                            <button class="button button--ghost"
                                onclick={Callback::from({let nav=nav.clone(); move |_| nav.push(&Route::Orders)})}>
                                {"Order History"}
                            </button>
                        </div>
                    </Card>
                }

                if is_admin {
                    <Card>
                        <h3 style="margin:0 0 0.75rem;">{"Admin"}</h3>
                        <div style="display:flex;flex-direction:column;gap:0.5rem;">
                            <button class="button button--ghost"
                                onclick={Callback::from({let nav=nav.clone(); move |_| nav.push(&Route::Analytics)})}>
                                {"Analytics"}
                            </button>
                            <button class="button button--ghost"
                                onclick={Callback::from({let nav=nav.clone(); move |_| nav.push(&Route::Moderation)})}>
                                {"Moderation Queue"}
                            </button>
                            <button class="button button--ghost"
                                onclick={Callback::from({let nav=nav.clone(); move |_| nav.push(&Route::SecurityEvents)})}>
                                {"Security Events"}
                            </button>
                        </div>
                    </Card>
                }
            </div>

            // ── Recent requisitions ───────────────────────────────────────────
            <Card>
                <h3 style="margin:0 0 0.75rem;">{"My Recent Requisitions"}</h3>
                { match (*my_reqs).clone() {
                    None => html! {
                        <div style="text-align:center;padding:1.5rem;color:#9ca3af;">
                            {"Loading…"}
                        </div>
                    },
                    Some(Err(ref e)) => html! {
                        <div style="color:#ef4444;padding:0.75rem;font-size:0.9rem;">{e}</div>
                    },
                    Some(Ok(ref reqs)) if reqs.is_empty() => html! {
                        <div style="text-align:center;padding:2rem;color:#9ca3af;">
                            <div style="font-size:2rem;margin-bottom:0.5rem;">{"📋"}</div>
                            <div>{"No requisitions yet."}</div>
                            <button class="button" style="margin-top:1rem;"
                                onclick={Callback::from({let nav=nav.clone(); move |_| nav.push(&Route::RequisitionNew)})}>
                                {"Create your first requisition"}
                            </button>
                        </div>
                    },
                    Some(Ok(ref reqs)) => html! {
                        <div style="overflow-x:auto;">
                        <table class="table" style="width:100%;">
                            <thead><tr>
                                <th>{"Ref"}</th>
                                <th>{"Status"}</th>
                                <th>{"Needed by"}</th>
                            </tr></thead>
                            <tbody>
                            { for reqs.iter().take(5).map(|r| {
                                let id = r.id.clone();
                                let nav = nav.clone();
                                html! { <tr style="cursor:pointer;"
                                    onclick={Callback::from(move |_| nav.push(&Route::RequisitionDetail { id: id.clone() }))}>
                                    <td style="font-family:monospace;font-size:0.8rem;">{&r.ref_code}</td>
                                    <td>{status_badge(&r.status)}</td>
                                    <td style="font-size:0.85rem;color:#6b7280;">
                                        {&r.needed_by[..10.min(r.needed_by.len())]}
                                    </td>
                                </tr> }
                            })}
                            </tbody>
                        </table>
                        if reqs.len() > 5 {
                            <div style="text-align:center;padding:0.5rem;">
                                <button class="button button--ghost button--sm"
                                    onclick={Callback::from({let nav=nav.clone(); move |_| nav.push(&Route::Requisitions)})}>
                                    {format!("View all {} requisitions →", reqs.len())}
                                </button>
                            </div>
                        }
                        </div>
                    },
                }}
            </Card>
        </>
    }
}
