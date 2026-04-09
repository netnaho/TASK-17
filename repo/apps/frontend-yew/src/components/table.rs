use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DataTableProps {
    pub headers: Vec<AttrValue>,
    pub rows: Vec<Vec<AttrValue>>,
}

#[function_component(DataTable)]
pub fn data_table(props: &DataTableProps) -> Html {
    html! {
        <table class="table">
            <thead>
                <tr>
                    { for props.headers.iter().map(|h| html!{ <th>{ h }</th> }) }
                </tr>
            </thead>
            <tbody>
                { for props.rows.iter().map(|row| html!{
                    <tr>
                        { for row.iter().map(|cell| html!{ <td>{ cell }</td> }) }
                    </tr>
                }) }
            </tbody>
        </table>
    }
}
