use yew::prelude::*;

use crate::components::states::ErrorState;

#[function_component(NotFoundPage)]
pub fn not_found() -> Html {
    html! {
        <ErrorState
            title={AttrValue::from("Page not found")}
            detail={Some(AttrValue::from("Check the URL or use the side navigation."))}
        />
    }
}
