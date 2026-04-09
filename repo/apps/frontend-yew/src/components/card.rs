use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CardProps {
    #[prop_or_default]
    pub title: Option<AttrValue>,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(Card)]
pub fn card(props: &CardProps) -> Html {
    html! {
        <section class="card">
            { if let Some(t) = &props.title { html!{ <h2>{ t }</h2> } } else { Html::default() } }
            { for props.children.iter() }
        </section>
    }
}
