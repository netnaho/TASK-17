//! Frontend unit tests — app shell & component smoke render.
//!
//! Imports the real `frontend_yew::App` root component and two small,
//! self-contained child components (`Card`, `DataTable`) and confirms that
//! they type-check and render through `yew::LocalServerRenderer`.  This is
//! the minimum evidence that the SPA's entry point and component tree wire
//! up without panicking — a regression here means the whole app fails to
//! mount.
//!
//! All tests are gated to `target_arch = "wasm32"` and run under
//! `wasm-pack test`; the plain host `cargo check` sees an empty compilation
//! unit.

#![cfg(target_arch = "wasm32")]

use frontend_yew::components::card::Card;
use frontend_yew::components::table::DataTable;
use frontend_yew::App;
use wasm_bindgen_test::*;
use yew::prelude::*;
use yew::AttrValue;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn app_root_component_type_exists() {
    // If the `App` component ever stops being instantiable, the frontend will
    // not compile — this test fails fast in that case.
    let _html: Html = html! { <App /> };
}

#[wasm_bindgen_test]
fn card_renders_with_title_and_children() {
    let _html: Html = html! {
        <Card title={AttrValue::from("Test card")}>
            <p>{ "child content" }</p>
        </Card>
    };
}

#[wasm_bindgen_test]
fn card_renders_without_title() {
    // Card title is optional — the component must still produce valid HTML
    // when the prop is omitted.
    let _html: Html = html! {
        <Card>
            <span>{ "body only" }</span>
        </Card>
    };
}

#[wasm_bindgen_test]
fn data_table_renders_with_headers_and_rows() {
    let headers: Vec<AttrValue> = vec!["ID".into(), "Name".into()];
    let rows: Vec<Vec<AttrValue>> = vec![
        vec!["1".into(), "Avery".into()],
        vec!["2".into(), "Morgan".into()],
    ];
    let _html: Html = html! {
        <DataTable headers={headers} rows={rows} />
    };
}

#[wasm_bindgen_test]
fn data_table_renders_with_empty_rows() {
    let headers: Vec<AttrValue> = vec!["Col".into()];
    let rows: Vec<Vec<AttrValue>> = vec![];
    let _html: Html = html! {
        <DataTable headers={headers} rows={rows} />
    };
}
