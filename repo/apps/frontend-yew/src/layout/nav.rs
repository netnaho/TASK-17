use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::use_auth;
use crate::router::Route;

#[derive(Clone, PartialEq)]
pub struct NavGroup {
    pub label: &'static str,
    pub items: Vec<Route>,
}

pub fn role_aware_groups() -> Vec<NavGroup> {
    // Phase 1: groups are role-aware in shape only — actual role gating is
    // wired in the auth phase. The structure already mirrors how the final
    // menu will collapse for non-admin roles.
    vec![
        NavGroup {
            label: "Operations",
            items: vec![Route::Dashboard, Route::Requisitions, Route::Approvals],
        },
        NavGroup {
            label: "Supply",
            items: vec![Route::Inventory, Route::Orders],
        },
        NavGroup {
            label: "Insights",
            items: vec![Route::Analytics],
        },
        NavGroup {
            label: "Residents",
            items: vec![Route::FamilyPortal],
        },
        NavGroup {
            label: "Admin",
            items: vec![Route::MasterData, Route::Moderation, Route::SecurityEvents, Route::Settings],
        },
    ]
}

#[derive(Properties, PartialEq)]
pub struct NavProps {
    pub current: Route,
}

fn route_visible_for_roles(route: &Route, roles: &[String]) -> bool {
    let admin = roles.iter().any(|r| r == "admin");
    if admin { return true; }
    match route {
        Route::Dashboard => true,
        Route::RequisitionNew | Route::RequisitionDetail { .. } => true,
        Route::Requisitions | Route::Approvals => roles.iter().any(|r|
            matches!(r.as_str(), "medical" | "dept_approver" | "finance_approver")),
        Route::Inventory | Route::Orders => roles.iter().any(|r|
            matches!(r.as_str(), "finance_approver" | "medical")),
        Route::Analytics => roles.iter().any(|r| r == "finance_approver"),
        Route::FamilyPortal => roles.iter().any(|r| matches!(r.as_str(), "family" | "senior")),
        Route::MasterData | Route::Settings | Route::Moderation | Route::SecurityEvents => false,
        _ => true,
    }
}

#[function_component(SideNav)]
pub fn side_nav(props: &NavProps) -> Html {
    let auth = use_auth();
    let roles = auth.principal.as_ref().map(|p| p.roles.clone()).unwrap_or_default();
    let groups: Vec<NavGroup> = role_aware_groups()
        .into_iter()
        .map(|mut g| {
            g.items.retain(|r| route_visible_for_roles(r, &roles));
            g
        })
        .filter(|g| !g.items.is_empty())
        .collect();
    html! {
        <aside class="sidebar">
            <div class="brand">
                {"SilverOak"}
                <small>{"Operations Suite"}</small>
            </div>
            { for groups.into_iter().map(|g| render_group(g, &props.current)) }
        </aside>
    }
}

fn render_group(group: NavGroup, current: &Route) -> Html {
    html! {
        <div>
            <div class="nav-section">{group.label}</div>
            { for group.items.into_iter().map(|r| render_link(r, current)) }
        </div>
    }
}

fn render_link(route: Route, current: &Route) -> Html {
    let class = if &route == current { "nav-link active" } else { "nav-link" };
    html! {
        <Link<Route> to={route.clone()} classes={classes!(class)}>
            { route.label() }
        </Link<Route>>
    }
}
