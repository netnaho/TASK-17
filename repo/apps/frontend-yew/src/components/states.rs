use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MessageProps {
    pub title: AttrValue,
    #[prop_or_default]
    pub detail: Option<AttrValue>,
}

#[function_component(LoadingState)]
pub fn loading(props: &MessageProps) -> Html {
    html! {
        <div class="state">
            <strong>{ &props.title }</strong>
            { if let Some(d) = &props.detail { html!{ <p>{ d }</p> } } else { Html::default() } }
        </div>
    }
}

#[function_component(EmptyState)]
pub fn empty(props: &MessageProps) -> Html {
    html! {
        <div class="state">
            <strong>{ &props.title }</strong>
            { if let Some(d) = &props.detail { html!{ <p>{ d }</p> } } else { Html::default() } }
        </div>
    }
}

#[function_component(ErrorState)]
pub fn error(props: &MessageProps) -> Html {
    html! {
        <div class="state error">
            <strong>{ &props.title }</strong>
            { if let Some(d) = &props.detail { html!{ <p>{ d }</p> } } else { Html::default() } }
        </div>
    }
}
