//! Frontend unit tests — `components::utils` helpers.
//!
//! These integration tests import the real `frontend_yew::components::utils`
//! module and verify the formatting helpers that every table/list view in the
//! SPA depends on (`fmt_date`, `fmt_datetime`).
//!
//! Execution: `wasm-pack test --node apps/frontend-yew` (wired via
//! `run_tests.sh` step 4 — "frontend unit tests").  The tests are gated to
//! `target_arch = "wasm32"` so a plain host-target `cargo check` sees an empty
//! file and does not try to link the yew/web-sys dependency chain.

#![cfg(target_arch = "wasm32")]

use frontend_yew::components::utils::{fmt_date, fmt_datetime};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn fmt_date_truncates_full_iso_datetime_to_date() {
    assert_eq!(fmt_date("2024-03-15T10:30:00Z"), "2024-03-15");
}

#[wasm_bindgen_test]
fn fmt_date_preserves_exact_10_char_date() {
    assert_eq!(fmt_date("2024-03-15"), "2024-03-15");
}

#[wasm_bindgen_test]
fn fmt_date_returns_dash_for_empty_string() {
    assert_eq!(fmt_date(""), "—");
}

#[wasm_bindgen_test]
fn fmt_date_passes_through_short_values_verbatim() {
    assert_eq!(fmt_date("2024"), "2024");
    assert_eq!(fmt_date("x"), "x");
}

#[wasm_bindgen_test]
fn fmt_datetime_truncates_to_minutes() {
    assert_eq!(fmt_datetime("2024-03-15T10:30:00Z"), "2024-03-15T10:30");
}

#[wasm_bindgen_test]
fn fmt_datetime_returns_dash_for_empty_string() {
    assert_eq!(fmt_datetime(""), "—");
}

#[wasm_bindgen_test]
fn fmt_datetime_preserves_exact_16_char_input() {
    assert_eq!(fmt_datetime("2024-03-15T10:30"), "2024-03-15T10:30");
}
