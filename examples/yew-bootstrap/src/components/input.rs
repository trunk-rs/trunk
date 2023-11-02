use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct InputProps {
    pub label: AttrValue,
    pub name: AttrValue,
    pub field_type: AttrValue,
}

#[function_component]
pub fn Input(props: &InputProps) -> Html {
    html! {
        <div class="mb-3">
            <label class="form-label">{props.label.clone()}</label>
            <input
                type={props.field_type.clone()}
                class="form-control"
                name={props.name.clone()}
            />
        </div>
    }
}
