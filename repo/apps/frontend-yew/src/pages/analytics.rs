/// Analytics dashboard — aggregated stats from worker-computed analytics tables.
/// Admin-only. Shows live operational counts + daily/weekly/monthly trend tables.
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::card::Card;

// ── DTOs ──────────────────────────────────────────────────────────────────
#[derive(Clone, PartialEq, Deserialize, Debug)]
struct StatRow {
    stat_key: String,
    stat_value: f64,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct DailyStats {
    stat_date: String,
    stats: Vec<StatRow>,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct WeeklyStats {
    week_start: String,
    stats: Vec<StatRow>,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct MonthlyStats {
    year_month: String,
    stats: Vec<StatRow>,
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
    recent_daily: Vec<DailyStats>,
    recent_weekly: Vec<WeeklyStats>,
    recent_monthly: Vec<MonthlyStats>,
    live: LiveCounts,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct Institution {
    id: String,
    name: String,
    code: String,
}

fn fmt_stat(key: &str) -> &str {
    match key {
        "requisition_count" => "Requisitions",
        "order_count" => "Orders",
        "supply_spend_cents" => "Supply Spend",
        "approved_requisitions" => "Approved",
        "wellness_sessions" => "Wellness",
        _ => key,
    }
}

fn fmt_value(key: &str, value: f64) -> String {
    if key == "supply_spend_cents" {
        format!("${:.2}", value / 100.0)
    } else {
        format!("{:.0}", value)
    }
}

fn stat_get(stats: &[StatRow], key: &str) -> String {
    stats.iter()
        .find(|s| s.stat_key == key)
        .map(|s| fmt_value(key, s.stat_value))
        .unwrap_or_else(|| "—".into())
}

#[function_component(AnalyticsPage)]
pub fn analytics() -> Html {
    let auth = use_auth();
    let institutions = use_state(|| Vec::<Institution>::new());
    let selected_iid = use_state(|| String::new());
    let dashboard = use_state(|| Option::<Result<DashboardView, String>>::None);

    // Load institutions on mount
    {
        let auth = auth.clone();
        let institutions = institutions.clone();
        let selected_iid = selected_iid.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                    let client = ApiClient { api_token: token, signing_key: key };
                    if let Ok(r) = client.get("/api/v1/master-data/institutions").await {
                        if let Ok(v) = r.json::<Vec<Institution>>().await {
                            if let Some(first) = v.first() {
                                selected_iid.set(first.id.clone());
                            }
                            institutions.set(v);
                        }
                    }
                }
            });
            || ()
        });
    }

    // Reload dashboard when institution selection changes
    {
        let auth = auth.clone();
        let dashboard = dashboard.clone();
        let iid = (*selected_iid).clone();
        use_effect_with(iid.clone(), move |iid| {
            let iid = iid.clone();
            if !iid.is_empty() {
                let dashboard = dashboard.clone();
                dashboard.set(None);
                spawn_local(async move {
                    if let (Some(token), Some(key)) = (auth.api_token.clone(), auth.signing_key.clone()) {
                        let client = ApiClient { api_token: token, signing_key: key };
                        let url = format!("/api/v1/analytics/dashboard?institution_id={}", iid);
                        match client.get(&url).await {
                            Ok(r) if r.ok() => match r.json::<DashboardView>().await {
                                Ok(v) => dashboard.set(Some(Ok(v))),
                                Err(e) => dashboard.set(Some(Err(e.to_string()))),
                            },
                            Ok(r) => {
                                let msg = r.text().await.unwrap_or_default();
                                dashboard.set(Some(Err(format!("HTTP {}: {}", r.status(), msg))));
                            },
                            Err(e) => dashboard.set(Some(Err(e.to_string()))),
                        }
                    }
                });
            }
            || ()
        });
    }

    let on_iid_change = {
        let selected_iid = selected_iid.clone();
        Callback::from(move |e: Event| {
            let el: web_sys::HtmlInputElement = e.target_unchecked_into();
            selected_iid.set(el.value());
        })
    };

    let iid_val = (*selected_iid).clone();

    html! {
        <>
            <div class="page-header">
                <h1>{"Analytics"}</h1>
                <p>{"Spend, throughput, and resident-care KPIs. Worker jobs refresh every 24 h."}</p>
            </div>

            // Institution selector
            <Card>
                <div style="display:flex;align-items:center;gap:1rem;">
                    <label><strong>{"Institution:"}</strong></label>
                    <select class="input" onchange={on_iid_change}>
                        { for (*institutions).iter().map(|i| {
                            let sel = i.id == iid_val;
                            html! { <option value={i.id.clone()} selected={sel}>
                                {format!("{} ({})", i.name, i.code)}
                            </option> }
                        })}
                    </select>
                </div>
            </Card>

            { match (*dashboard).clone() {
                None if iid_val.is_empty() => html! {},
                None => html! { <Card><div style="text-align:center;padding:2rem;color:#888;">{"Loading…"}</div></Card> },
                Some(Err(ref e)) => html! { <Card><div style="color:red;padding:1rem;">{e}</div></Card> },
                Some(Ok(ref dash)) => html! {
                    <>
                        // Live counts banner
                        <Card>
                            <h2 style="margin:0 0 1rem;">{"Live Counts"}</h2>
                            <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:1rem;">
                                { kpi_tile("Pending Requisitions", dash.live.pending_requisitions.to_string(), "#f59e0b") }
                                { kpi_tile("Open Orders", dash.live.open_orders.to_string(), "#3b82f6") }
                                { kpi_tile("Wellness (week)", dash.live.wellness_sessions_this_week.to_string(), "#10b981") }
                                { kpi_tile("Moderation Queue", dash.live.moderation_queue_pending.to_string(), "#ef4444") }
                                { kpi_tile("Anomalies (unacked)", dash.live.anomaly_events_unacked.to_string(), "#8b5cf6") }
                            </div>
                        </Card>

                        // Daily trends
                        if !dash.recent_daily.is_empty() {
                            <Card>
                                <h2 style="margin:0 0 1rem;">{"Daily Stats — Last 7 Days"}</h2>
                                <div style="overflow-x:auto;">
                                <table class="table" style="width:100%;">
                                    <thead><tr>
                                        <th>{"Date"}</th><th>{"Requisitions"}</th>
                                        <th>{"Orders"}</th><th>{"Supply Spend"}</th><th>{"Approved"}</th>
                                    </tr></thead>
                                    <tbody>
                                    { for dash.recent_daily.iter().map(|d| html! {
                                        <tr>
                                            <td>{&d.stat_date}</td>
                                            <td>{stat_get(&d.stats, "requisition_count")}</td>
                                            <td>{stat_get(&d.stats, "order_count")}</td>
                                            <td>{stat_get(&d.stats, "supply_spend_cents")}</td>
                                            <td>{stat_get(&d.stats, "approved_requisitions")}</td>
                                        </tr>
                                    })}
                                    </tbody>
                                </table>
                                </div>
                            </Card>
                        }

                        // Weekly trends
                        if !dash.recent_weekly.is_empty() {
                            <Card>
                                <h2 style="margin:0 0 1rem;">{"Weekly Stats — Last 4 Weeks"}</h2>
                                <div style="overflow-x:auto;">
                                <table class="table" style="width:100%;">
                                    <thead><tr>
                                        <th>{"Week Starting"}</th><th>{"Requisitions"}</th>
                                        <th>{"Orders"}</th><th>{"Wellness"}</th><th>{"Supply Spend"}</th>
                                    </tr></thead>
                                    <tbody>
                                    { for dash.recent_weekly.iter().map(|w| html! {
                                        <tr>
                                            <td>{&w.week_start}</td>
                                            <td>{stat_get(&w.stats, "requisition_count")}</td>
                                            <td>{stat_get(&w.stats, "order_count")}</td>
                                            <td>{stat_get(&w.stats, "wellness_sessions")}</td>
                                            <td>{stat_get(&w.stats, "supply_spend_cents")}</td>
                                        </tr>
                                    })}
                                    </tbody>
                                </table>
                                </div>
                            </Card>
                        }

                        // Monthly trends
                        if !dash.recent_monthly.is_empty() {
                            <Card>
                                <h2 style="margin:0 0 1rem;">{"Monthly Stats — Last 3 Months"}</h2>
                                <div style="overflow-x:auto;">
                                <table class="table" style="width:100%;">
                                    <thead><tr>
                                        <th>{"Month"}</th>
                                        { for ["requisition_count","order_count","supply_spend_cents","approved_requisitions"]
                                            .iter().map(|k| html!{ <th>{fmt_stat(k)}</th> }) }
                                    </tr></thead>
                                    <tbody>
                                    { for dash.recent_monthly.iter().map(|m| html! {
                                        <tr>
                                            <td>{&m.year_month}</td>
                                            <td>{stat_get(&m.stats, "requisition_count")}</td>
                                            <td>{stat_get(&m.stats, "order_count")}</td>
                                            <td>{stat_get(&m.stats, "supply_spend_cents")}</td>
                                            <td>{stat_get(&m.stats, "approved_requisitions")}</td>
                                        </tr>
                                    })}
                                    </tbody>
                                </table>
                                </div>
                            </Card>
                        }

                        if dash.recent_daily.is_empty() && dash.recent_weekly.is_empty() && dash.recent_monthly.is_empty() {
                            <Card>
                                <p style="color:#888;text-align:center;padding:2rem;">
                                    {"No analytics data computed yet. The daily worker job runs every 24 hours."}
                                </p>
                            </Card>
                        }
                    </>
                },
            }}
        </>
    }
}

fn kpi_tile(label: &str, value: String, color: &str) -> Html {
    html! {
        <div style={format!(
            "border:2px solid {color};border-radius:8px;padding:1rem;text-align:center;"
        )}>
            <div style={format!("font-size:1.8rem;font-weight:bold;color:{color};")}>{value}</div>
            <div style="font-size:0.8rem;color:#666;margin-top:0.25rem;">{label}</div>
        </div>
    }
}
