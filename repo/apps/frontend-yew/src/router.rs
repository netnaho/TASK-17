use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq, Debug)]
pub enum Route {
    #[at("/")]
    Dashboard,
    #[at("/login")]
    Login,
    #[at("/requisitions")]
    Requisitions,
    #[at("/requisitions/new")]
    RequisitionNew,
    #[at("/requisitions/:id")]
    RequisitionDetail { id: String },
    #[at("/approvals")]
    Approvals,
    #[at("/inventory")]
    Inventory,
    #[at("/orders")]
    Orders,
    #[at("/orders/cart")]
    Cart,
    #[at("/orders/checkout")]
    Checkout,
    #[at("/orders/:id")]
    OrderDetail { id: String },
    #[at("/master-data")]
    MasterData,
    #[at("/analytics")]
    Analytics,
    #[at("/family")]
    FamilyPortal,
    #[at("/moderation")]
    Moderation,
    #[at("/security")]
    SecurityEvents,
    #[at("/settings")]
    Settings,
    #[not_found]
    #[at("/404")]
    NotFound,
}

impl Route {
    pub fn label(&self) -> &'static str {
        match self {
            Route::Dashboard => "Dashboard",
            Route::Login => "Sign in",
            Route::Requisitions => "Requisitions",
            Route::RequisitionNew => "New requisition",
            Route::RequisitionDetail { .. } => "Requisition",
            Route::Approvals => "Approvals",
            Route::Inventory => "Inventory",
            Route::Orders => "Orders",
            Route::Cart => "Cart",
            Route::Checkout => "Checkout",
            Route::OrderDetail { .. } => "Order",
            Route::MasterData => "Master data",
            Route::Analytics => "Analytics",
            Route::FamilyPortal => "Family portal",
            Route::Moderation => "Moderation",
            Route::SecurityEvents => "Security",
            Route::Settings => "Settings",
            Route::NotFound => "Not found",
        }
    }
}
