//! Frontend unit tests — router `Route` enum labels and parameterised paths.
//!
//! Imports the real `frontend_yew::router::Route` type and verifies the
//! nav-label mapping (used by the sidebar and breadcrumb components) plus
//! round-trip construction of the dynamic routes (`RequisitionDetail`,
//! `OrderDetail`).  Together with `app_render.test.rs`, this covers the app
//! shell + routing layer.

#![cfg(target_arch = "wasm32")]

use frontend_yew::router::Route;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn dashboard_route_label_is_dashboard() {
    assert_eq!(Route::Dashboard.label(), "Dashboard");
}

#[wasm_bindgen_test]
fn login_route_label_is_sign_in() {
    assert_eq!(Route::Login.label(), "Sign in");
}

#[wasm_bindgen_test]
fn requisitions_route_label_is_requisitions() {
    assert_eq!(Route::Requisitions.label(), "Requisitions");
}

#[wasm_bindgen_test]
fn cart_route_label_is_cart() {
    assert_eq!(Route::Cart.label(), "Cart");
}

#[wasm_bindgen_test]
fn master_data_route_label_matches_sidebar() {
    assert_eq!(Route::MasterData.label(), "Master data");
}

#[wasm_bindgen_test]
fn not_found_route_label_is_not_found() {
    assert_eq!(Route::NotFound.label(), "Not found");
}

#[wasm_bindgen_test]
fn requisition_detail_preserves_id_parameter() {
    let r = Route::RequisitionDetail { id: "abc-123".to_string() };
    // Label is the static category name; id is preserved in the struct field.
    assert_eq!(r.label(), "Requisition");
    if let Route::RequisitionDetail { id } = r {
        assert_eq!(id, "abc-123");
    } else {
        panic!("route variant destructure failed");
    }
}

#[wasm_bindgen_test]
fn order_detail_preserves_id_parameter() {
    let r = Route::OrderDetail { id: "order-xyz".to_string() };
    assert_eq!(r.label(), "Order");
    if let Route::OrderDetail { id } = r {
        assert_eq!(id, "order-xyz");
    } else {
        panic!("route variant destructure failed");
    }
}

#[wasm_bindgen_test]
fn all_static_routes_produce_non_empty_labels() {
    let routes = [
        Route::Dashboard,
        Route::Login,
        Route::Requisitions,
        Route::RequisitionNew,
        Route::Approvals,
        Route::Inventory,
        Route::Orders,
        Route::Cart,
        Route::Checkout,
        Route::MasterData,
        Route::Analytics,
        Route::FamilyPortal,
        Route::Moderation,
        Route::SecurityEvents,
        Route::Settings,
        Route::NotFound,
    ];
    for r in routes {
        assert!(
            !r.label().is_empty(),
            "label must be non-empty for every route variant"
        );
    }
}
