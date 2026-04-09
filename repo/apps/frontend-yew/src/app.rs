use yew::prelude::*;
use yew_router::prelude::*;

use crate::auth::{use_auth, AuthProvider};
use crate::layout::Shell;
use crate::pages;
use crate::router::Route;

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <BrowserRouter>
            <AuthProvider>
                <Switch<Route> render={switch} />
            </AuthProvider>
        </BrowserRouter>
    }
}

fn switch(route: Route) -> Html {
    html! { <Guard route={route} /> }
}

#[derive(Properties, PartialEq)]
struct GuardProps { route: Route }

#[function_component(Guard)]
fn guard(props: &GuardProps) -> Html {
    let auth = use_auth();
    let route = props.route.clone();

    if !auth.bootstrapped {
        return html! { <div class="state">{"Loading…"}</div> };
    }

    if matches!(route, Route::Login) {
        return html! { <pages::login::LoginPage /> };
    }

    if !auth.is_authenticated() {
        return html! { <Redirect<Route> to={Route::Login}/> };
    }

    html! {
        <Shell current={route.clone()}>
            { render_page(route) }
        </Shell>
    }
}

fn render_page(route: Route) -> Html {
    match route {
        Route::Dashboard => html! { <pages::dashboard::DashboardPage /> },
        Route::Login => html! { <pages::login::LoginPage /> },
        Route::Requisitions => html! { <pages::requisitions::RequisitionsPage /> },
        Route::RequisitionNew => html! { <pages::requisition_form::RequisitionFormPage /> },
        Route::RequisitionDetail { id } => html! { <pages::requisition_detail::RequisitionDetailPage id={id} /> },
        Route::Approvals => html! { <pages::approvals::ApprovalsPage /> },
        Route::Inventory => html! { <pages::inventory::InventoryPage /> },
        Route::Orders => html! { <pages::orders::OrdersPage /> },
        Route::Cart => html! { <pages::cart::CartPage /> },
        Route::Checkout => html! { <pages::checkout::CheckoutPage /> },
        Route::OrderDetail { id } => html! { <pages::order_detail::OrderDetailPage id={id} /> },
        Route::MasterData => html! { <pages::master_data::MasterDataPage /> },
        Route::Analytics => html! { <pages::analytics::AnalyticsPage /> },
        Route::FamilyPortal => html! { <pages::family_portal::FamilyPortalPage /> },
        Route::Moderation => html! { <pages::moderation::ModerationPage /> },
        Route::SecurityEvents => html! { <pages::security_events::SecurityEventsPage /> },
        Route::Settings => html! { <pages::settings::SettingsPage /> },
        Route::NotFound => html! { <pages::not_found::NotFoundPage /> },
    }
}
