/// Inventory page — stock levels across all categories for the current site.
///
/// Reads from GET /api/v1/inventory/catalog which requires inventory:read or
/// requisitions:write. Shows on-hand quantities with a visual low-stock
/// indicator (< 10 units highlighted in amber, 0 in red).
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::auth::{use_auth, ApiClient};
use crate::components::card::Card;

// ── DTO ───────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct StockItem {
    id: String,
    sku: String,
    name: String,
    unit: String,
    unit_price_cents: i64,
    on_hand: i32,
    controlled: bool,
    category: String,
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn stock_badge(on_hand: i32) -> Html {
    let (bg, fg, label) = if on_hand == 0 {
        ("#fee2e2", "#991b1b", "Out of stock")
    } else if on_hand < 10 {
        ("#fef3c7", "#92400e", "Low stock")
    } else {
        ("#d1fae5", "#065f46", "In stock")
    };
    html! {
        <span style={format!(
            "background:{bg};color:{fg};padding:0.15rem 0.45rem;\
             border-radius:3px;font-size:0.75rem;font-weight:600;"
        )}>
            {label}
        </span>
    }
}

fn fmt_price(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}

// ── component ─────────────────────────────────────────────────────────────────

#[function_component(InventoryPage)]
pub fn inventory() -> Html {
    let auth = use_auth();
    let items = use_state(|| Option::<Result<Vec<StockItem>, String>>::None);
    let search = use_state(|| String::new());
    let filter_low = use_state(|| false);

    {
        let auth = auth.clone();
        let items = items.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let (Some(token), Some(key)) =
                    (auth.api_token.clone(), auth.signing_key.clone())
                {
                    let client = ApiClient { api_token: token, signing_key: key };
                    match client.get("/api/v1/inventory/catalog").await {
                        Ok(r) if r.ok() => match r.json::<Vec<StockItem>>().await {
                            Ok(v) => items.set(Some(Ok(v))),
                            Err(e) => items.set(Some(Err(e.to_string()))),
                        },
                        Ok(r) => items.set(Some(Err(format!("HTTP {}", r.status())))),
                        Err(e) => items.set(Some(Err(e))),
                    }
                }
            });
            || ()
        });
    }

    let on_search = {
        let search = search.clone();
        Callback::from(move |e: InputEvent| {
            let el: web_sys::HtmlInputElement = e.target_unchecked_into();
            search.set(el.value());
        })
    };

    let toggle_low = {
        let filter_low = filter_low.clone();
        Callback::from(move |_: MouseEvent| filter_low.set(!*filter_low))
    };

    html! {
        <>
            <div class="page-header">
                <h1>{"Inventory"}</h1>
                <p>{"Stock levels and availability across the current site."}</p>
            </div>

            <Card>
                // Toolbar
                <div style="display:flex;gap:0.75rem;align-items:center;\
                            margin-bottom:1rem;flex-wrap:wrap;">
                    <input
                        class="input"
                        type="search"
                        placeholder="Search by name or SKU…"
                        value={(*search).clone()}
                        oninput={on_search}
                        style="flex:1;min-width:200px;"
                    />
                    <button
                        class={if *filter_low { "button" } else { "button button--ghost" }}
                        onclick={toggle_low}
                    >
                        {"⚠ Low stock only"}
                    </button>
                </div>

                // Data
                { match (*items).clone() {
                    None => html! {
                        <div style="text-align:center;padding:3rem;color:#9ca3af;">
                            <div style="font-size:1.5rem;margin-bottom:0.5rem;">{"⏳"}</div>
                            {"Loading stock levels…"}
                        </div>
                    },
                    Some(Err(ref e)) => html! {
                        <div style="padding:1rem;background:#fee2e2;border-radius:4px;\
                                    color:#991b1b;font-size:0.9rem;">
                            <strong>{"Error loading inventory: "}</strong>{e}
                        </div>
                    },
                    Some(Ok(ref all_items)) => {
                        let search_lc = (*search).to_lowercase();
                        let visible: Vec<&StockItem> = all_items.iter()
                            .filter(|it| {
                                let name_match = it.name.to_lowercase().contains(&search_lc)
                                    || it.sku.to_lowercase().contains(&search_lc);
                                let low_match = !*filter_low || it.on_hand < 10;
                                name_match && low_match
                            })
                            .collect();

                        let total_items = all_items.len();
                        let low_count = all_items.iter().filter(|it| it.on_hand < 10).count();
                        let out_count  = all_items.iter().filter(|it| it.on_hand == 0).count();

                        html! {
                            <>
                                // Summary strip
                                <div style="display:flex;gap:1.5rem;margin-bottom:1rem;\
                                            font-size:0.85rem;flex-wrap:wrap;">
                                    <span style="color:#6b7280;">
                                        <strong style="color:#111827;">{total_items}</strong>
                                        {" SKUs total"}
                                    </span>
                                    <span style="color:#92400e;">
                                        <strong>{low_count}</strong>{" low-stock"}
                                    </span>
                                    <span style="color:#991b1b;">
                                        <strong>{out_count}</strong>{" out-of-stock"}
                                    </span>
                                </div>

                                if visible.is_empty() {
                                    <div style="text-align:center;padding:2rem;color:#9ca3af;">
                                        {"No items match the current filter."}
                                    </div>
                                } else {
                                    <div style="overflow-x:auto;">
                                    <table class="table" style="width:100%;">
                                        <thead><tr>
                                            <th>{"SKU"}</th>
                                            <th>{"Name"}</th>
                                            <th>{"Category"}</th>
                                            <th style="text-align:right;">{"Unit price"}</th>
                                            <th style="text-align:right;">{"On hand"}</th>
                                            <th>{"Status"}</th>
                                        </tr></thead>
                                        <tbody>
                                        { for visible.iter().map(|it| html! { <tr>
                                            <td>
                                                <code style="font-size:0.8rem;">{&it.sku}</code>
                                                if it.controlled {
                                                    <span style="margin-left:0.35rem;\
                                                        background:#fde68a;color:#78350f;\
                                                        font-size:0.65rem;padding:0.1rem 0.3rem;\
                                                        border-radius:2px;font-weight:700;">
                                                        {"CTRL"}
                                                    </span>
                                                }
                                            </td>
                                            <td style="font-size:0.9rem;">{&it.name}</td>
                                            <td style="font-size:0.85rem;color:#6b7280;">{&it.category}</td>
                                            <td style="text-align:right;font-size:0.9rem;font-family:monospace;">
                                                {fmt_price(it.unit_price_cents)}
                                                <span style="font-size:0.75rem;color:#9ca3af;margin-left:0.25rem;">
                                                    {"/"}{&it.unit}
                                                </span>
                                            </td>
                                            <td style="text-align:right;">
                                                <strong style={
                                                    if it.on_hand == 0 { "color:#ef4444;" }
                                                    else if it.on_hand < 10 { "color:#f59e0b;" }
                                                    else { "color:#111827;" }
                                                }>
                                                    {it.on_hand}
                                                </strong>
                                            </td>
                                            <td>{stock_badge(it.on_hand)}</td>
                                        </tr> })}
                                        </tbody>
                                    </table>
                                    </div>
                                }
                            </>
                        }
                    }
                }}
            </Card>
        </>
    }
}
